# arb-lint

A linter and formatter for Flutter [ARB](https://github.com/google/app-resource-bundle)
localization files. It catches what `flutter gen-l10n` rejects, and what it silently
lets through.

```text
$ arb-lint tests/fixtures/bad
lib/l10n/app_ar.arb:3:3: warning[rtl-untranslated]: 'title' in RTL locale 'ar' has Latin text but no RTL letters
lib/l10n/app_ar.arb:4:3: error[unknown-placeholder]: 'welcome' uses {usr}, which the template does not have
lib/l10n/app_ar.arb:5:3: warning[plural-missing-category]: 'cartItems': plural 'n' for locale 'ar' lacks: zero, two, few, many
lib/l10n/app_en.arb:3:3: error[duplicate-key]: key 'title' is defined more than once (the last one wins)
lib/l10n/app_en.arb:13:3: error[icu-syntax]: 'broken': unclosed '{' (at character 6)
lib/l10n/app_en.arb:15:3: warning[unused-key]: 'neverUsed' is not referenced in Dart code under lib
lib/l10n/app_es.arb:1:23: error[invalid-json]: trailing comma at line 1 column 23
...
6 error(s), 13 warning(s)
```

## Install

Prebuilt binaries (Linux x86_64/aarch64, macOS x86_64/arm64, Windows x86_64) are
attached to every [GitHub release](https://github.com/AbdeldjalilChougui/arb-lint/releases), with installers:

```sh
# Linux / macOS
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/AbdeldjalilChougui/arb-lint/releases/latest/download/arb-lint-installer.sh | sh
```

```powershell
# Windows
powershell -ExecutionPolicy Bypass -c "irm https://github.com/AbdeldjalilChougui/arb-lint/releases/latest/download/arb-lint-installer.ps1 | iex"
```

Or build from crates.io:

```sh
cargo install arb-lint --locked
```

### GitHub Action

```yaml
- uses: actions/checkout@v5
- uses: AbdeldjalilChougui/arb-lint@v0.1.1
  with:
    path: .                 # Flutter project root or a directory of .arb files
    fail-on: warning        # error (default) | warning | never
```

With GitHub code scanning (SARIF):

```yaml
permissions:
  contents: read
  security-events: write
steps:
  - uses: actions/checkout@v5
  - uses: AbdeldjalilChougui/arb-lint@v0.1.1
    with:
      format: sarif
      output: arb-lint.sarif
      fail-on: never
  - uses: github/codeql-action/upload-sarif@v4
    with:
      sarif_file: arb-lint.sarif
```

Inputs: `path`, `format` (`text`/`json`/`sarif`), `output`, `fail-on`, `template`,
`require-descriptions`, `args` (extra flags), `version` (defaults to the action tag).

## Quick start

```sh
cargo install --path .
arb-lint                          # in a Flutter project (reads l10n.yaml, scans lib/ for unused keys)
arb-lint lib/l10n --template app_en.arb
arb-lint --format sarif > arb.sarif   # GitHub code scanning
arb-lint --format json
arb-lint --fix                    # canonical formatting, then lint
arb-lint --deny-warnings --require-descriptions   # strict CI mode
```

Exit status: `0` clean (or only warnings), `1` errors (or warnings with
`--deny-warnings`), `2` usage/IO problems.

## Checks

| rule | severity | what |
|---|---|---|
| `invalid-json` | error | not a JSON object (trailing commas, etc.) |
| `duplicate-key` | error | a key defined twice in one file (JSON parsers keep the last silently) |
| `invalid-message` / `invalid-metadata` | error | non-string message, malformed `@key` |
| `icu-syntax` | error | unbalanced braces, plural/select without `other`, bad plural category, duplicate case |
| `undeclared-placeholder` | error | template uses `{x}` not declared in `@key.placeholders` |
| `unused-placeholder` | warning | declared but unused placeholder |
| `unknown-placeholder` | error | translation uses a placeholder the template lacks |
| `missing-placeholder` | warning | translation drops a template placeholder |
| `missing-translation` / `extra-key` | warning | key sets differ from the template |
| `orphan-metadata` | warning | `@key` without a message |
| `missing-description` | warning | with `--require-descriptions` |
| `plural-missing-category` | warning | CLDR categories required by the locale (Arabic needs zero/one/two/few/many/other; `=0/=1/=2` count as zero/one/two) |
| `locale-mismatch` | warning | `@@locale` disagrees with the file name |
| `empty-message` | warning | empty translation |
| `unused-key` | warning | template key never referenced in Dart code |
| `rtl-untranslated` | warning | message in an RTL locale (ar, fa, he, ur…) with Latin text and no RTL letters |
| `rtl-bidi-controls` | warning | unbalanced embedding/override/isolate control characters |

`--fix` puts translations in the template's key order, places each `@key` right after
its message, puts `@@` attributes first, adds a missing `@@locale` from the file name,
and normalizes to 2-space JSON with a final newline. It never changes values and never
adds or removes messages.

## How it works

- `l10n.yaml` is read for `arb-dir` and `template-arb-file` (with Flutter's defaults).
  The template can also be guessed: `app_en.arb`, then any `en` file, then the file
  with the most metadata.
- JSON is parsed with key order preserved. A second pass with a custom serde visitor
  finds duplicate keys.
- `icu.rs` is a small recursive-descent parser for the ICU MessageFormat subset that
  gen-l10n accepts: arguments, `plural` (with `=N` and `#`), `select`, and nesting.
- Unused keys: every identifier after a `.` in non-generated `.dart` files under `lib/`
  is collected, and template keys that never appear are reported.
- SARIF 2.1.0 output includes rule metadata and file/line/column locations.

## Tests

`cargo test`: unit tests for the ICU parser, plural tables, locale detection, positions
and the formatter. `tests/fixtures/good` (a clean project, also under strict flags) and
`tests/fixtures/bad` (which triggers every rule) are checked through the CLI in
text, JSON and SARIF formats, plus a `--fix` round trip.

## Status / limitations

- Unused-key detection is a heuristic. Keys built dynamically (for example with
  `lookup(name)`) are reported as unused. Use `--no-unused` to turn it off.
- Apostrophe escaping (`use-escaping: true`) is not modelled: quotes are treated as
  literal text, as in gen-l10n's default.
- Placeholder `type`/`format` values are not validated. Only top-level keys get
  positions; errors inside a message are reported as a character offset.

## License

MIT, see [LICENSE](LICENSE).
