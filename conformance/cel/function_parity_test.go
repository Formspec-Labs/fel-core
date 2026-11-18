package bridge_test

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"os/exec"
	"slices"
	"strings"
	"testing"

	"cel.dev/cel-go/cel"
	"cel.dev/cel-go/common/decls"
	"cel.dev/cel-go/common/types"
	"cel.dev/cel-go/ext"
)

// This audit exercises source conversion only. A successful native case must
// produce its expected target text, pass the independent CEL checker and survive
// reimport. A refused case must exit normally with a diagnostic and empty stdout.
func checkNativeMapping(t *testing.T, env *cel.Env, direction, source, want string) {
	t.Helper()
	bin, err := executable()
	if err != nil {
		t.Fatal(err)
	}
	cmd := exec.Command(bin, direction, "--native")
	cmd.Stdin = strings.NewReader(source)
	var stderr bytes.Buffer
	cmd.Stderr = &stderr
	out, err := cmd.Output()
	if want == "" {
		var exit *exec.ExitError
		if !errors.As(err, &exit) || exit.ExitCode() != 1 || len(out) != 0 || stderr.Len() == 0 {
			t.Fatalf("%s %q: expected a native refusal, stdout=%q stderr=%q err=%v", direction, source, out, stderr.String(), err)
		}
		return
	}
	if err != nil || strings.TrimSpace(string(out)) != want {
		t.Fatalf("%s %q: want %q, got %q, stderr=%q (%v)", direction, source, want, out, stderr.String(), err)
	}
	reverse := "to-cel"
	if direction == "to-cel" {
		reverse = "to-fel"
	}
	back, err := probe([]string{reverse}, string(out))
	if err != nil {
		t.Fatalf("%s %q: %s (%v)", reverse, out, back, err)
	}
	celText := out
	if direction == "to-fel" {
		celText = back
	}
	if _, issues := env.Compile(string(celText)); issues.Err() != nil {
		t.Fatal(issues.Err())
	}
	again, err := probe([]string{direction}, string(back))
	if err != nil || !bytes.Equal(again, out) {
		t.Fatalf("canonical target changed for %q: %q / %q (%v)", source, out, again, err)
	}
}

func TestFunctionParityFEL(t *testing.T) {
	bins, err := buildTools()
	if err != nil {
		t.Fatal(err)
	}
	raw, err := exec.Command(bins.catalog).Output()
	if err != nil {
		t.Fatal(err)
	}
	var catalog struct {
		Functions []struct {
			Name     string
			Examples []struct{ Expression string }
		}
	}
	if err := json.Unmarshal(raw, &catalog); err != nil {
		t.Fatal(err)
	}
	if len(catalog.Functions) == 0 {
		t.Fatal("empty FEL catalog")
	}
	checker, err := cel.NewEnv()
	if err != nil {
		t.Fatal(err)
	}
	parser := testSyntaxEnv(t)

	// Use non-null literals to isolate dispatch from the default nullable field binding.
	type sample struct{ source, want string }
	native := map[string]sample{
		"length":     {"length('abc')", `size("abc")`},
		"count":      {"count([1,null,3])", "[1, null, 3].filter(item, dyn(item) != null).size()"},
		"startsWith": {"startsWith('abc','a')", `"abc".startsWith("a")`},
		"endsWith":   {"endsWith('abc','c')", `"abc".endsWith("c")`},
		"contains":   {"contains('abc','b')", `"abc".contains("b")`},
		"if":         {"if(true, 1, 2)", "true ? 1 : 2"},
	}
	names := make(map[string]bool)
	var shared []string
	standard := checker.Functions()
	examples, accepted := 0, 0
	for _, f := range catalog.Functions {
		if names[f.Name] || len(f.Examples) == 0 {
			t.Fatalf("duplicate name or missing catalog examples: %s", f.Name)
		}
		names[f.Name] = true
		if standard[f.Name] != nil {
			shared = append(shared, f.Name)
		}
		t.Run(f.Name, func(t *testing.T) {
			for _, e := range f.Examples {
				target, _, err := syntaxRoundTrip("to-cel", e.Expression)
				if err != nil {
					t.Fatal(err)
				}
				if _, issues := parser.Parse(target); issues.Err() != nil {
					t.Fatalf("%q: %v", target, issues.Err())
				}
				examples++
			}
			first := f.Examples[0].Expression
			if f.Name == "instance" {
				first = "instance('priorYear')"
				if _, _, err := syntaxRoundTrip("to-cel", first); err != nil {
					t.Fatal(err)
				}
			} // The catalog examples use the @ form.
			witness, ok := native[f.Name]
			if !ok {
				witness = sample{first, ""}
			} else {
				accepted++
			}
			checkNativeMapping(t, checker, "to-cel", witness.source, witness.want)
		})
	}
	if accepted != len(native) {
		t.Fatal("native witness refers to an unknown FEL function")
	}
	slices.Sort(shared)
	if !slices.Equal(shared, []string{"contains", "duration", "endsWith", "matches", "startsWith", "string"}) {
		t.Fatalf("exact public names shared with standard CEL changed: %v", shared)
	}
	t.Logf("%d FEL built-ins, %d syntax examples, %d native ordinary-call/conditional examples", len(names), examples, accepted)
	for _, tc := range []sample{
		{"length([1,null])", "size([1, null])"}, {"length(null)", ""}, {"contains(null,'x')", ""},
		{"length({'x':1})", ""}, {"if(null,1,2)", ""}, {"if(true,1,'a')", ""},
		{"floor(1 / 2)", "1 / 2"}, {"ceil(-3 / 2)", "-3 / 2"},
		{"floor(2.5)", ""}, {"ceil(2.5)", ""}, {"ceil(1 / 2)", ""},
		{"count(null)", ""}, {"count('abc')", ""}, {"matches('abc','b')", ""}, {"string(42)", ""},
		{"duration('PT1H')", ""},
	} {
		checkNativeMapping(t, checker, "to-cel", tc.source, tc.want)
	}
	checkNativeMapping(t, checker, "to-fel", "size([1, null, 3])", "length([1, null, 3])")
}

// Pick a literal for each declared CEL parameter type. Date parts with a timezone
// and string conversions use valid input text even though the audit only compiles them.
func celArgument(name string, typ *cel.Type, index int) (string, error) {
	switch typ.Kind() {
	case types.BoolKind:
		return "true", nil
	case types.BytesKind:
		return `b"abc"`, nil
	case types.DoubleKind:
		return "1.5", nil
	case types.DurationKind:
		return `duration("1h")`, nil
	case types.IntKind, types.TypeParamKind:
		return "1", nil
	case types.ListKind:
		return "[1, 2]", nil
	case types.MapKind:
		return `{"k": 1}`, nil
	case types.TimestampKind:
		return `timestamp("2024-03-02T12:34:56Z")`, nil
	case types.UintKind:
		return "1u", nil
	case types.StringKind:
		if strings.HasPrefix(name, "get") {
			return `"UTC"`, nil
		}
		switch name {
		case "bool":
			return `"true"`, nil
		case "double", "int", "uint":
			return `"1"`, nil
		case "duration":
			return `"1h"`, nil
		case "timestamp":
			return `"2024-03-02T12:34:56Z"`, nil
		}
		if index > 0 {
			return `"a"`, nil
		}
		return `"abc"`, nil
	}
	return "", fmt.Errorf("missing sample for %s parameter %v", name, typ)
}

func celCall(name string, overload *decls.OverloadDecl) (string, error) {
	var args []string
	for i, typ := range overload.ArgTypes() {
		v, err := celArgument(name, typ, i)
		if err != nil {
			return "", err
		}
		args = append(args, v)
	}
	if overload.IsMemberFunction() {
		name = args[0] + "." + name
		args = args[1:]
	}
	return name + "(" + strings.Join(args, ", ") + ")", nil
}

func TestFunctionParityCEL(t *testing.T) {
	env, err := cel.NewEnv()
	if err != nil {
		t.Fatal(err)
	}
	parser := testSyntaxEnv(t)
	want := map[string]string{
		"contains_string": "contains('abc', 'a')", "starts_with_string": "startsWith('abc', 'a')",
		"ends_with_string": "endsWith('abc', 'a')",
		"size_list":        "length([1, 2])", "list_size": "length([1, 2])",
		"size_string": "length('abc')", "string_size": "length('abc')",
	}
	var names []string
	for name := range env.Functions() {
		// 'in' is a deprecated function declaration for the membership operator.
		// The remaining non-callable operator and internal names start with punctuation.
		if name != "in" && len(name) > 0 && (name[0] >= 'a' && name[0] <= 'z' || name[0] >= 'A' && name[0] <= 'Z') {
			names = append(names, name)
		}
	}
	slices.Sort(names)
	total, accepted := 0, 0
	for _, name := range names {
		for _, overload := range env.Functions()[name].OverloadDecls() {
			total++
			t.Run(overload.ID(), func(t *testing.T) {
				source, err := celCall(name, overload)
				if err != nil {
					t.Fatal(err)
				}
				checked, issues := env.Compile(source)
				if issues.Err() != nil {
					t.Fatalf("%q: %v", source, issues.Err())
				}
				tree := checked.NativeRep()
				if ids := tree.GetOverloadIDs(tree.Expr().ID()); !slices.Contains(ids, overload.ID()) {
					t.Fatalf("%q checked as %v, expected %s", source, ids, overload.ID())
				}
				if err := compareCELSource(parser, source); err != nil {
					t.Fatal(err)
				}
				target := want[overload.ID()]
				checkNativeMapping(t, env, "to-fel", source, target)
				if target != "" {
					accepted++
				}
			})
		}
	}
	if accepted != len(want) {
		t.Fatal("native witness refers to an unknown CEL overload")
	}
	t.Logf("%d standard CEL public named functions, %d verified overloads, %d native successes", len(names), total, accepted)

	macros := map[string]string{
		"has/1": `has({"k":1}.k)`, "all/2": "[1].all(x,x>0)", "exists/2": "[1].exists(x,x>0)",
		"exists_one/2": "[1].exists_one(x,x>0)", "map/2": "[1].map(x,x+1)",
		"map/3": "[1].map(x,x>0,x+1)", "filter/2": "[1].filter(x,x>0)",
	}
	for _, m := range env.Macros() {
		key := fmt.Sprintf("%s/%d", m.Function(), m.ArgCount())
		s, ok := macros[key]
		if !ok {
			t.Fatalf("missing macro witness %s", key)
		}
		if _, issues := env.Compile(s); issues.Err() != nil {
			t.Fatal(issues.Err())
		}
		if err := compareCELSource(parser, s); err != nil {
			t.Fatal(err)
		}
		checkNativeMapping(t, env, "to-fel", s, "")
		delete(macros, key)
	}
	if len(macros) != 0 {
		t.Fatalf("macro witnesses without declarations: %v", macros)
	}
	t.Logf("%d default CEL macro signatures, all standalone examples syntax-only", len(env.Macros()))
}

func TestFunctionParityExtensions(t *testing.T) {
	env, err := cel.NewEnv(ext.Strings(), ext.Math())
	if err != nil {
		t.Fatal(err)
	}
	parser := testSyntaxEnv(t)
	cases := []string{
		`"abc".substring(1)`, `"abc".substring(1,2)`,
		`"aba".replace("a","b")`, `"aba".replace("a","b",1)`, `" a ".trim()`,
		`"Straße".upperAscii()`, `"É".lowerAscii()`, `"%s".format(["a"])`,
		"math.abs(-1)", "math.abs(1u)", "math.abs(-1.5)", "math.round(2.5)", "math.ceil(1.5)", "math.floor(1.5)",
		"math.least(2,1)", "math.least([2,1])", "math.greatest(2,1)", "math.greatest([2,1])",
	}
	for _, s := range cases {
		t.Run(s, func(t *testing.T) {
			if _, issues := env.Compile(s); issues.Err() != nil {
				t.Fatal(issues.Err())
			}
			if err := compareCELSource(parser, s); err != nil {
				t.Fatal(err)
			}
			checkNativeMapping(t, env, "to-fel", s, "")
		})
	}
	t.Logf("%d optional CEL string and math examples checked", len(cases))
}
