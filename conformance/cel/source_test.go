package bridge_test

import (
	"bytes"
	"os/exec"
	"strings"
	"testing"

	"cel.dev/cel-go/cel"
)

// These tests only parse, type-check and convert text. No expression is evaluated.
func TestSourceConversion(t *testing.T) {
	env, err := cel.NewEnv(
		cel.Variable("applicant.kind", cel.NullableType(cel.StringType)),
		cel.Variable("age", cel.IntType),
		cel.Variable("name", cel.StringType),
		cel.Variable("names", cel.ListType(cel.DynType)),
		cel.Variable("state", cel.NullableType(cel.StringType)),
	)
	if err != nil {
		t.Fatal(err)
	}
	medicationNames := []string{"--bind", "names:list=$medications[*].medDrugName"}
	cases := []struct {
		source, fel string
		flags       []string
	}{
		{"applicant.kind == \"nonprofit\"", "$applicant.kind = 'nonprofit'", nil},
		{"age >= 18", "$person.age >= 18", []string{"--bind", "age:int=$person.age"}},
		{"name.startsWith(\"A\")", "startsWith($person.name, 'A')", []string{"--bind", "name:string=$person.name"}},
		{"true ? {\"x\": [1, 2]} : {\"x\": [3]}", "true ? {'x': [1, 2]} : {'x': [3]}", nil},
		{"size([\"a\", \"b\"]) > 1", "length(['a', 'b']) > 1", nil},
		{"names.size()", "length($medications[*].medDrugName)", medicationNames},
		{"names.filter(n, dyn(n) != null).size()", "count($medications[*].medDrugName)", medicationNames},
		{"names.filter(n, n != null).size()", "count($medications[*].medDrugName)", medicationNames},
		{"size(names.filter(names, dyn(names) != null))", "count($medications[*].medDrugName)", medicationNames},
		{"size([1, null, 3].filter(x, null != x))", "count([1, null, 3])", nil},
		{"[1, 2].filter(x, null != dyn(x)).size()", "count([1, 2])", nil},
		{`["aspirin", null, ""].filter(x, dyn(x) != null).size()`, "count(['aspirin', null, ''])", nil},
		{"[0, false, null].filter(x, dyn(x) != null).size()", "count([0, false, null])", nil},
		{"[1, 2].filter(i, dyn(i) != null).size()", "count([1, 2])", nil},
		{"[state, name].filter(x, x != null).size()", "count([$state, $name])", []string{"--bind", "name:string=$name", "--bind", "state:string?=$state"}},
		{"[null, null].filter(i, dyn(i) != null).size()", "count([null, null])", nil},
		{"[[1, null].filter(i, i != null).size(), null].filter(i, dyn(i) != null).size()", "count([count([1, null]), null])", nil},
		{"[].filter(i, dyn(i) != null).size()", "count([])", nil},
		{"(true ? [1, null] : []).filter(i, dyn(i) != null).size()", "count(true ? [1, null] : [])", nil},
		{"\"hello\" + \" λ\"", "'hello' & ' λ'", nil},
		{"\"x\" in [\"x\", \"y\"]", "'x' in ['x', 'y']", nil},
		{"[\"first\", \"second\"][1]", "['first', 'second'][2]", nil},
		{"[[1, 2]][0][1]", "[[1, 2]][1][2]", nil},
		{"1 / 2", "floor(1 / 2)", nil},
		{"-3 / 2", "ceil((-3) / 2)", nil},
		{"1.0 / 2.0", "1 / 2", nil},
		{"0.000000059604644775390625", "0.000000059604644775390625", nil},
		{"0.5", "0.5", nil},
		{"-9223372036854775808", "-9223372036854775808", nil},
		{"9223372036854775807 + 0", "9223372036854775807 + 0", nil},
	}
	convert := func(direction string, flags []string, text string) string {
		t.Helper()
		out, err := probe(append([]string{direction}, flags...), text)
		if err != nil {
			t.Fatalf("%s %q: %s (%v)", direction, text, out, err)
		}
		return strings.TrimSuffix(string(out), "\n")
	}
	for _, tc := range cases {
		t.Run(tc.source, func(t *testing.T) {
			if _, issues := env.Compile(tc.source); issues.Err() != nil {
				t.Fatal(issues.Err())
			}
			fel := convert("to-fel", tc.flags, tc.source)
			if fel != tc.fel {
				t.Fatalf("want %q, got %q", tc.fel, fel)
			}
			back := convert("to-cel", tc.flags, fel)
			if _, issues := env.Compile(back); issues.Err() != nil {
				t.Fatal(issues.Err())
			}
			if again := convert("to-fel", tc.flags, back); again != fel {
				t.Fatalf("reverse text changed the rule: %q / %q", fel, again)
			}
		})
	}
	for _, source := range []string{"if $state = 'MD' then true else false", "'x' not in ['a', 'b']"} {
		returned := convert("to-cel", nil, source)
		if _, issues := env.Compile(returned); issues.Err() != nil {
			t.Fatal(issues.Err())
		}
		if got := convert("to-cel", nil, convert("to-fel", nil, returned)); got != returned {
			t.Fatalf("FEL-origin canonical text changed: %q / %q", returned, got)
		}
	}
}

// CEL infers nested collection types and honors literal order for nullable strings.
func TestNativeCollectionTypeRefusals(t *testing.T) {
	env, err := cel.NewEnv(cel.Variable("name", cel.StringType), cel.Variable("state", cel.NullableType(cel.StringType)))
	if err != nil {
		t.Fatal(err)
	}
	for _, source := range []string{
		"true ? [1] : ['one']", "true ? {'x': [1]} : {'x': ['one']}",
		"[name, state].filter(x, x != null).size()",
	} {
		if _, issues := env.Compile(source); issues.Err() == nil {
			t.Fatalf("CEL accepted %q", source)
		}
		if out, err := probe([]string{"to-fel", "--bind", "name:string=$name", "--bind", "state:string?=$state"}, source); err == nil {
			t.Fatalf("native import accepted %q: %s", source, out)
		}
	}
	if out, err := probe([]string{"to-cel"}, "if true then [1] else ['one']"); err == nil {
		t.Fatalf("native export accepted incompatible branches: %s", out)
	}
}

func TestSourceRefusal(t *testing.T) {
	bin, err := executable()
	if err != nil {
		t.Fatal(err)
	}
	for _, tc := range []struct{ direction, expr, reason string }{
		{"to-fel", "73u", "unsigned"},
		{"to-fel", "b'abc'", "bytes"},
		{"to-fel", "has(answers.x)", "key presence"},
		{"to-fel", "[1][1]", "out-of-bounds"}, {"to-cel", "[1][0]", "out-of-bounds"},
		{"to-fel", "0.1", "no exact"}, {"to-cel", "0.1", "exactly representable"},
		{"to-fel", "9223372036854775807 + 1", "in-range"},
		{"to-fel", "-9223372036854775808 / -1", "defined quotient"},
		{"to-cel", "1 / 3", "decimal division"},
		{"to-fel", "1.0 / 3.0", "not proved identical"},
		{"to-fel", "{1: 'x'}", "literal strings"},
		{"to-cel", "money(1, 'USD')", "money"},
		{"to-cel", "@current.amount", "no CEL binding"},
		{"to-fel", "name.startsWith('a')", "argument types"},
		{"to-fel", "[1, 2].filter(x, dyn(x) == null).size()", "no native FEL mapping"},
		{"to-fel", "[1, 2].filter(x, x != null).size()", "no native FEL mapping"},
		{"to-fel", "[1, 2].filter(x, dyn(1) != null).size()", "no native FEL mapping"},
		{"to-fel", "{'key': 1}.filter(x, dyn(x) != null).size()", "argument types"},
		{"to-fel", "true &&", ""}, {"to-cel", "if true then", ""},
	} {
		t.Run(tc.direction+"/"+tc.expr, func(t *testing.T) {
			cmd := exec.Command(bin, tc.direction, "--native")
			cmd.Stdin = strings.NewReader(tc.expr)
			var stderr bytes.Buffer
			cmd.Stderr = &stderr
			out, err := cmd.Output()
			if err == nil || len(out) != 0 || stderr.Len() == 0 || !strings.Contains(stderr.String(), tc.reason) {
				t.Fatalf("bad refusal: stdout=%q stderr=%q error=%v", out, stderr.String(), err)
			}
		})
	}
	cmd := exec.Command(bin, "to-fel", "true", "{}")
	if out, err := cmd.Output(); err == nil || len(out) != 0 {
		t.Fatalf("the text converter accepted form data: %q (%v)", out, err)
	}
}
