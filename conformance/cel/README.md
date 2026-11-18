# CEL and FEL function parity

The bridge preserves source for every built-in in the FEL catalog and every public named function signature in the pinned standard CEL environment that we tested. Native translation recognizes four named CEL functions and six FEL built-ins on supported inputs. Two additional FEL functions work in specific integer-division expressions. [Source-only audit](function_parity_test.go)

| Source checked | Reversible syntax | Native conversion observed |
| --- | --- | --- |
| All 75 FEL built-ins | All 133 catalog examples round-tripped. The named `instance(...)` form also passed separately from its two `@instance` examples. | Six names had an ordinary positive example. Two others had a division-specific positive example. The other 67 names had a refusal. |
| All 25 public named functions in standard CEL | All 70 declared signatures round-tripped. The independent CEL checker confirmed the selected overload for each input. | Seven signatures across four names had a positive example. The other 63 signatures had a refusal. |
| All seven macro signatures in the default CEL environment | All seven round-tripped, including both forms of `map`. | All seven standalone examples had a native refusal. The non-null `filter` followed by `size` translates to FEL `count`. |
| Selected CEL string and math extensions | All 18 examples across 12 names round-tripped and the reference checker accepted their source with the extensions enabled. | All 18 had a native refusal in the bridge's default environment. |

A signature identifies a function's argument types and whether it is written as a global call or a method. Each positive native example passed in both directions and its generated CEL passed an independent type check. The audit used parsing, type checking and conversion only. All-input equivalence, general operator coverage and arbitrary host extensions remain outside these counts. The earlier 2,344 CEL and 254 FEL fixtures provide separate evidence for preserving expression trees. [Function tests](function_parity_test.go) · [Fixture tests](syntax_test.go)

## Which calls translate natively?

| FEL | CEL | Supported inputs checked |
| --- | --- | --- |
| `contains(s, part)`, `startsWith(s, prefix)`, `endsWith(s, suffix)` | `s.contains(part)`, `s.startsWith(prefix)`, `s.endsWith(suffix)` | Non-null strings. One CEL signature per method. |
| `length(x)` | `size(x)` or `x.size()` | Strings and lists, including a list containing null. These account for four CEL signatures. The bridge exports the global spelling. CEL also defines byte and map size. |
| `count(xs)` | `xs.filter(x, dyn(x) != null).size()` | A list literal or expression of known list type, including a repeat field declared with `--bind`. Global `size` around the same filter also imports. |
| `if(c, a, b)` and `if c then a else b` | `c ? a : b` | A Boolean condition and branches of the same supported type. CEL counts its conditional as an operator. |
| `floor(1 / 2)`, `ceil(-3 / 2)` | `1 / 2`, `-3 / 2` | Proven signed integer-literal division with the matching rounding direction. Standalone `floor(2.5)` and `ceil(2.5)` were refused. |

Six names occur verbatim in both catalogs. The three string predicates above translate. The other three are `matches`, `string` and `duration`. For example, CEL `duration("1h")` represents a duration value while FEL `duration('PT1H')` produces a number of milliseconds. Both spellings survive a syntax round trip. [FEL duration](../../src/evaluator/builtins/dates.rs) · [CEL language definition](https://github.com/cel-expr/cel-spec/blob/v0.25.1/doc/langdef.md)

## Where the remaining FEL functions fit

Each of the other 67 FEL names appears once below. The middle column identifies related CEL facilities, including potential expressions with multiple operations. The native converter currently refuses the FEL calls in these rows. CEL facilities labeled “extension” are optional. [FEL catalog](https://github.com/Formspec-Labs/formspec/blob/main/schemas/fel-functions.schema.json) · [CEL standard declarations](https://github.com/cel-expr/cel-go/blob/v0.33.4/common/stdlib/standard.go) · [CEL extensions](https://github.com/cel-expr/cel-go/blob/v0.33.4/ext/README.md)

| FEL functions | Closest CEL facility | What a mapping would need to preserve |
| --- | --- | --- |
| `countWhere` | `filter(...).size()` | The FEL predicate binds `$` to each element. Translation must preserve its handling of nulls and errors. |
| `every`, `some` | `all`, `exists` macros | Per-element binding, empty input, nulls and errors. |
| `min`, `max`, `minWhere`, `maxWhere` | Math extension `math.least`, `math.greatest`, optionally after `filter` | FEL supports collection and scalar forms for min/max. Input types, empty lists and null elements affect the mapping. |
| `sum`, `avg`, `sumWhere`, `avgWhere` | `filter` can select rows. Standard CEL has no general-purpose source function for summing a list. | Accumulation, decimal precision and empty inputs. |
| `matches` | `s.matches(pattern)` or `matches(s, pattern)` | Pattern dialect, invalid patterns and nulls. |
| `substring`, `replace`, `trim` | String extensions with the same names | FEL substring takes a one-based start and optional length. CEL takes a zero-based start and optional exclusive end. CEL replace also accepts a replacement limit. |
| `upper`, `lower`, `format` | String extensions `upperAscii`, `lowerAscii`, `format` | Unicode case conversion versus ASCII, and differences in format strings and arguments. |
| `round`, `abs`, `power` | Math extensions `math.round` and `math.abs`. The pinned math extension exposes no general exponentiation function. | FEL supports decimal-place rounding and ties to even. CEL's math extension rounds a double to a whole value with ties away from zero. |
| `year`, `month`, `day`, `hours`, `minutes`, `seconds` | Timestamp accessors such as `getFullYear`, `getMonth`, `getDate`, `getHours`, `getMinutes`, `getSeconds` | Date versus timestamp, time zones and one-based FEL months versus zero-based CEL months. Some CEL accessors also accept durations. |
| `date`, `time`, `dateDiff`, `dateAdd`, `timeDiff`, `duration` | `timestamp`, `duration`, timestamp addition and subtraction | Date-only inputs, time-of-day values, calendar units and the result type. |
| `today`, `now` | A host-supplied clock value | Clock, date and time zone belong to the form host. |
| `coalesce`, `empty`, `present` | Explicit tests and conditionals. CEL `has` checks field presence. | An absent field, an explicitly null value and an empty answer are distinct cases. |
| `selected` | Membership `in` for lists or equality for a single choice | Choice representation, non-selection and null. |
| `isNumber`, `isString`, `isDate`, `isNull`, `typeOf` | CEL `type`, type comparisons and null comparisons | FEL decimal, date and money types and FEL's string-valued type names. |
| `number`, `string`, `boolean` | `int`, `uint`, `double`, `string`, `bool` | Accepted input types, decimal precision, string formatting and failed conversions. |
| `money`, `moneyAmount`, `moneyCurrency`, `moneyAdd`, `moneySum`, `moneySumWhere` | A host-defined monetary type and functions | Decimal amounts, currency and aggregation rules. |
| `locale`, `runtimeMeta`, `subject`, `pluralCategory`, `formatNumber`, `formatDate` | Host context and formatting facilities | Locale, response subject, metadata and the form's formatting conventions. |
| `valid`, `relevant`, `readonly`, `required`, `prev`, `next`, `parent`, `instance` | Host-supplied form and repeat state | Which field, repeat row or named data source the form means. |

### What does a form count?

The clinical intake form uses `count($medications[*].medDrugName)` to count non-null medication names. With three names `['aspirin', null, '']`, that formula gives 2. To count the three rows, use `count($medications)`. Each repeat row is an object, even when every answer is missing. To count just the one nonblank name, use `countWhere($medications[*].medDrugName, present(trim($)))`. [Clinical intake form](https://github.com/Formspec-Labs/formspec/blob/main/examples/clinical-intake/intake.definition.json) · [Formspec repeat and aggregate rules](https://github.com/Formspec-Labs/formspec/blob/main/specs/core/spec.md)

CEL `size` and fel-core `length` count every position in a list. Formspec's core function table lists the string signature for `length`. Its repeat rules explicitly support `count($medications)` for counting rows. FEL defines `count([])` as 0 and `count(null)` as null. The corrected FEL catalog agrees with the implementation and the conformance example. [Formspec core](https://github.com/Formspec-Labs/formspec/blob/main/specs/core/spec.md) · [FEL catalog](../../src/extensions/catalog/aggregate.rs) · [Conformance fixture](../../conformance/fel-conformance.jsonl) · [CEL list size](https://github.com/cel-expr/cel-spec/blob/v0.25.1/doc/langdef.md#list-operators)

The bridge exports a FEL `count` on a known list as `xs.filter(item, dyn(item) != null).size()`. The `dyn` call lets the CEL checker accept the null comparison even for lists of integers or strings. On import, `xs.filter(x, x != null).size()` also maps to `count` when the element type permits that comparison. Use the default editing mode to preserve a FEL call whose list can itself be null. [Conversion checks](source_test.go)

## Reproduce the audit

Run `go test -count=1 -v -run '^TestFunctionParity' .` from this directory. The FEL side reads the generated catalog. The CEL side enumerates the default `cel-go` environment and verifies a typed example for every named overload and a source example for every registered macro signature. The audit uses Rust `cel 0.16.0`, `cel-go v0.33.4` and the FEL catalog version `1.0`. The test reads the catalog directly from the crate’s `emit-fel-schema` command. [Test cases](function_parity_test.go) · [FEL catalog generator](../../src/bin/emit-fel-schema.rs)
