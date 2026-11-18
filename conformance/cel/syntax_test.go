package bridge_test

import (
	"bufio"
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"cel.dev/cel-go/cel"
	_ "cel.dev/expr/conformance/proto2"
	_ "cel.dev/expr/conformance/proto3"
	conformance "cel.dev/expr/conformance/test"
	"google.golang.org/protobuf/encoding/prototext"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/reflect/protoreflect"
)

func syntaxRoundTrip(direction, source string) (string, string, error) {
	reverse := "to-cel"
	if direction == reverse {
		reverse = "to-fel"
	}
	out, err := probe([]string{direction, "--syntax"}, source)
	if err != nil {
		return "", "", fmt.Errorf("%s: %s", direction, out)
	}
	back, err := probe([]string{reverse, "--syntax"}, string(out))
	if err != nil {
		return "", "", fmt.Errorf("%s: %s", reverse, back)
	}
	return string(out), strings.TrimSuffix(string(back), "\n"), nil
}

func TestSyntaxEditing(t *testing.T) {
	cases := []struct{ direction, expr, old, edit, want string }{
		{"to-cel", "money($amount, 'USD')", `"USD"`, `"GBP"`, `money($amount, 'GBP')`},
		{"to-fel", `[b"abc", 73u]`, "73u", "74u", `74u`},
		{"to-cel", "@current.amount ?? $defaultAmount", "defaultAmount", "baseAmount", "$baseAmount"},
		{"to-fel", "items.all(x, x.price > 3)", " > 3", " > 5", " > 5"},
		{"to-cel", "bridge_fel_decimal('abc')", `"abc"`, `"def"`, "bridge_fel_decimal('def')"},
	}
	for _, tc := range cases {
		target, _, err := syntaxRoundTrip(tc.direction, tc.expr)
		if err != nil {
			t.Fatal(err)
		}
		if !strings.Contains(target, tc.old) {
			t.Fatalf("missing operand in %q", target)
		}
		edit := strings.Replace(target, tc.old, tc.edit, 1)
		reverse := "to-cel"
		if tc.direction == reverse {
			reverse = "to-fel"
		}
		restored, err := probe([]string{reverse, "--syntax"}, edit)
		if err != nil || !strings.Contains(string(restored), tc.want) {
			t.Fatalf("%q: %s (%v)", target, restored, err)
		}
	}
	bin, err := executable()
	if err != nil {
		t.Fatal(err)
	}
	for _, s := range []string{"// cel-fel-syntax v1 from CEL\nbridge_cel_bytes('true')", "// cel-fel-syntax v2 from CEL\ntrue", "// cel-fel-syntax v1 from FEL\ntrue"} {
		cmd := exec.Command(bin, "to-cel", "--syntax")
		cmd.Stdin = strings.NewReader(s)
		var diag bytes.Buffer
		cmd.Stderr = &diag
		if out, err := cmd.Output(); err == nil || len(out) > 0 || diag.Len() == 0 {
			t.Fatalf("invalid marker accepted: %q, %s", s, out)
		}
	}
	original := "// cel-fel-syntax v1 from FEL\ntrue"
	if out, err := probe([]string{"to-fel", "--syntax", "--literal"}, original); err != nil || !strings.HasPrefix(string(out), "// cel-fel-syntax v1 from CEL") {
		t.Fatalf("literal marker: %s (%v)", out, err)
	}
}

// Parser-assigned IDs can change when the printer inserts parentheses. The
// independent CEL parser must recover every other field of the same expression.
func clearIDs(m protoreflect.Message) {
	m.Range(func(fd protoreflect.FieldDescriptor, v protoreflect.Value) bool {
		if fd.Name() == "id" {
			m.Clear(fd)
			return true
		}
		if fd.Kind() == protoreflect.MessageKind {
			if fd.IsList() {
				xs := v.List()
				for i := 0; i < xs.Len(); i++ {
					clearIDs(xs.Get(i).Message())
				}
			} else {
				clearIDs(v.Message())
			}
		}
		return true
	})
}

func testSyntaxEnv(t *testing.T) *cel.Env {
	t.Helper()
	env, err := cel.NewEnv(cel.OptionalTypes(), cel.ClearMacros(), cel.EnableIdentifierEscapeSyntax())
	if err != nil {
		t.Fatal(err)
	}
	return env
}

func TestSyntaxFELCorpus(t *testing.T) {
	f, err := os.Open("../fel-conformance.jsonl")
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	env := testSyntaxEnv(t)
	scanner := bufio.NewScanner(f)
	count := 0
	for scanner.Scan() {
		var row struct{ Expression string }
		if err := json.Unmarshal(scanner.Bytes(), &row); err != nil {
			t.Fatal(err)
		}
		target, back, err := syntaxRoundTrip("to-cel", row.Expression)
		if err != nil {
			t.Fatalf("FEL %q: %v", row.Expression, err)
		}
		if _, issues := env.Parse(target); issues.Err() != nil {
			t.Fatalf("generated CEL %q: %v", target, issues.Err())
		}
		again, err := probe([]string{"to-cel", "--syntax"}, back)
		if err != nil || string(again) != target {
			t.Fatalf("FEL %q changed: %s (%v)", row.Expression, again, err)
		}
		count++
	}
	if err := scanner.Err(); err != nil {
		t.Fatal(err)
	}
	if count == 0 {
		t.Fatal("empty FEL corpus")
	}
	t.Logf("%d FEL source expressions", count)
}

func compareCELSource(env *cel.Env, source string) error {
	before, issues := env.Parse(source)
	if issues.Err() != nil {
		return fmt.Errorf("upstream input: %w", issues.Err())
	}
	_, back, err := syntaxRoundTrip("to-fel", source)
	if err != nil {
		return err
	}
	after, issues := env.Parse(back)
	if issues.Err() != nil {
		return fmt.Errorf("regenerated CEL %q: %w", back, issues.Err())
	}
	a, err := cel.AstToParsedExpr(before)
	if err != nil {
		return err
	}
	b, err := cel.AstToParsedExpr(after)
	if err != nil {
		return err
	}
	clearIDs(a.Expr.ProtoReflect())
	clearIDs(b.Expr.ProtoReflect())
	if !proto.Equal(a.Expr, b.Expr) {
		return fmt.Errorf("different CEL tree for %q", back)
	}
	return nil
}

func TestSyntaxCELCorpus(t *testing.T) {
	out, err := exec.Command("go", "list", "-m", "-f", "{{.Dir}}", "cel.dev/expr").Output()
	if err != nil {
		t.Fatal(err)
	}
	files, err := filepath.Glob(filepath.Join(strings.TrimSpace(string(out)), "tests/simple/testdata/*.textproto"))
	if err != nil || len(files) == 0 {
		t.Fatalf("CEL corpus: %v, %d files", err, len(files))
	}
	env := testSyntaxEnv(t)
	count, failures := 0, 0
	for _, path := range files {
		data, err := os.ReadFile(path)
		if err != nil {
			t.Fatal(err)
		}
		var file conformance.SimpleTestFile
		if err := prototext.Unmarshal(data, &file); err != nil {
			t.Fatalf("%s: %s", path, err)
		}
		for _, section := range file.GetSection() {
			for _, test := range section.GetTest() {
				count++
				source := test.GetExpr()
				if err := compareCELSource(env, source); err != nil {
					failures++
					if failures <= 18 {
						t.Logf("%s/%s/%s %q: %v", file.Name, section.Name, test.Name, source, err)
					}
				}
			}
		}
	}
	if failures > 0 {
		t.Fatalf("%d of %d CEL corpus expressions failed", failures, count)
	}
	t.Logf("%d CEL source expressions in %d files", count, len(files))
}

func TestSyntaxSpelling(t *testing.T) {
	env := testSyntaxEnv(t)
	for _, source := range []string{
		"-0x8000000000000000", "(-1).test()", "(-0.5).test()", "(-1)[0]",
		"(a && b) && (c && d)", "a && (b && c) && d && e",
		"f(')') && b // (\n && c && d", "(!a) && (b > 1) && c && d",
		"bridge_cel_bytes('literal call')", "[?x]", "ns.Msg{?`in`: x}",
	} {
		if err := compareCELSource(env, source); err != nil {
			t.Errorf("CEL %q: %v", source, err)
		}
	}
	for _, source := range []string{
		"-0", "let x = (1 in [1]) in x", strings.Repeat("1 + ", 90) + "2",
		strings.Repeat("not ", 90) + "$ready", strings.Repeat("$a ?? ", 90) + "0",
		"$rows[id = $selected].name", "($field).subfield",
	} {
		if _, _, err := syntaxRoundTrip("to-cel", source); err != nil {
			t.Errorf("FEL %q: %v", source, err)
		}
	}
	nested := strings.Repeat("f(", 50) + "x" + strings.Repeat(")", 50)
	target, _, err := syntaxRoundTrip("to-fel", nested)
	if err != nil || !strings.Contains(target, "bridge_tree(") {
		t.Fatalf("deep CEL: %s (%v)", target, err)
	}
	if err := compareCELSource(env, nested); err != nil {
		t.Error(err)
	}
	broken := strings.Replace(target, "bridge_ref(0)", "bridge_ref(999999)", 1)
	if out, err := probe([]string{"to-cel", "--syntax"}, broken); err == nil || !strings.Contains(string(out), "earlier, unused node") {
		t.Fatalf("accepted invalid syntax reference: %s (%v)", out, err)
	}
}
