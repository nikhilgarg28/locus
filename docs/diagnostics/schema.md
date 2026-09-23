# Diagnostic JSON schema 1

`locus check file.lc --error-format json` writes one **one-element JSON array per diagnostic line** to stderr. Each line is independently parseable. Successful checks without warnings write no diagnostics. Normal command results remain on stdout. `--error-format=json` is equivalent; text is the default. ANSI color is never added to JSON.

Every diagnostic object has exactly these keys:

| Key | Schema 1 value |
| --- | --- |
| `schema_version` | Integer `1` |
| `severity` | `"error"` or `"warning"` |
| `code`, `message` | Strings |
| `spans` | Array of locations with `primary: bool` and `label: string`; exactly one primary |
| `notes`, `helps` | Arrays of strings |
| `suggestions` | Array of `{message, span, replacement, applicability}` |
| `claim`, `claim_after_computing` | String or null |
| `facts_considered` | Array of `{name: string or null, claim: string}` or null |
| `counterexample` | String describing a found counterexample, or null |
| `suggested_explicit_form` | A checked source expression, or null |

Locations have exactly `file`, `byte_start`, `byte_end`, `line_start`, `column_start`, `line_end`, and `column_end`. Byte offsets form a half-open UTF-8 interval. Lines and Unicode scalar columns are one-based, not terminal display widths; invalid positions use null line/column values. Suggestions use a location without the `primary` and `label` keys and an applicability of `machine_applicable` or `maybe_incorrect`. A suggestion's message is stored with the suggestion, not duplicated in `helps`.

Optional proof fields are always present. Null means unavailable, not false, unproved, or an empty list. An empty facts list means the diagnostic considered no relevant displayable facts. Bounded display/search exhaustion is stated in notes. A suggested explicit form is only populated when that concrete form was checked; general guidance remains in notes or helps. The structured fields are constructed by the compiler, independently of human-facing note wording.

Source spans are mapped back to the original entry or explicit library file before diagnostics are stably sorted by primary file name and byte offset. Ties preserve emission order. Driver-only diagnostics use an empty `<driver>` source and position 0. A source-size preflight uses the input file's name and an empty span because the oversized source is deliberately not read.

Code ranges: L0000–L0099 lexical/resource input, L0100–L0199 parser, L0200–L0299 elaborator, L0300–L0399 lowering/independent checker, L0400–L0499 driver, L0900–L0999 reserved internal errors. Former L0299 is L0300. Codes identify causes, not proof rules. `locus explain CODE` prints a cause, concrete example and stable Language-as-built paragraph citation.

Changing the fixed key set, a field's type or its meaning requires a schema-version increment and a documented migration. Adding a diagnostic code or changing human wording does not change the schema version. Consumers must tolerate unknown codes and must check `schema_version` before interpreting fields.

Corpus `.jsonl` files pin every rejected source beside its `.stderr` golden; `tests/diagnostics/driver` pins driver and generated resource cases. Explanation stdout goldens are in `tests/diagnostics/explain`. `LOCUS_BLESS=1 cargo test --test diagnostics_json` updates them. `tools/check_diagnostic_json.py` validates schema 1 using Python's standard library.
