// The reference checks invoke the same installed command that authors use.
package bridge_test

import (
	"encoding/json"
	"errors"
	"io"
	"os"
	"os/exec"
	"slices"
	"strings"
	"sync"
)

type binaries struct{ converter, catalog string }

// Cargo reports the actual paths even with a custom target directory or profile.
var buildTools = sync.OnceValues(func() (binaries, error) {
	cmd := exec.Command("cargo", "build", "--features", "cel-bridge", "--bin", "fel-convert", "--bin", "emit-fel-schema", "--message-format=json")
	cmd.Stderr = os.Stderr
	out, err := cmd.Output()
	if err != nil {
		return binaries{}, err
	}
	var bins binaries
	decoder := json.NewDecoder(strings.NewReader(string(out)))
	for {
		var artifact struct {
			Target     struct{ Name string }
			Executable string
		}
		if err := decoder.Decode(&artifact); err != nil {
			if errors.Is(err, io.EOF) {
				break
			}
			return bins, err
		}
		if artifact.Executable == "" {
			continue
		}
		switch artifact.Target.Name {
		case "fel-convert":
			bins.converter = artifact.Executable
		case "emit-fel-schema":
			bins.catalog = artifact.Executable
		}
	}
	if bins.converter == "" || bins.catalog == "" {
		return bins, errors.New("Cargo did not return the converter and catalog paths")
	}
	return bins, nil
})

func executable() (string, error) {
	bins, err := buildTools()
	return bins.converter, err
}

func probe(flags []string, source string) ([]byte, error) {
	bin, err := executable()
	if err != nil {
		return nil, err
	}
	args := slices.Clone(flags)
	if !slices.Contains(args, "--syntax") {
		args = append(args, "--native")
	}
	cmd := exec.Command(bin, append(args, "-")...)
	cmd.Stdin = strings.NewReader(source)
	return cmd.CombinedOutput()
}
