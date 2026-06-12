# Error Code Reference

Updated: 2026-06-04
Roadmap item: `R-105` (`complete`)

This file defines the stable diagnostic-code ranges currently implemented in Phase 1 and the high-frequency diagnostics tooling can depend on.

## Code Families

| Range | Phase | Meaning |
| --- | --- | --- |
| `L001-L099` | lexer | Tokenization and literal scanning errors |
| `P001-P099` | parser | Syntax and feature-gate errors |
| `E001-E099` | semantic | Name resolution, typing, control flow, and trait validation errors |
| `lint(<rule>)` | lint | Lint warnings or denied lint findings |
| `midend` | midend | Internal IR/lowering errors without a stable subcode yet |
| `backend` | backend | Codegen or backend execution errors without a stable subcode yet |
| `io` | CLI | Host filesystem/process I/O errors |
| `cli` | CLI | Command-line planning or project-discovery failures |

## High-Frequency Diagnostics

The following set is the current stable Phase 1 table for high-frequency diagnostics with actionable hints.

| Code | Phase | Meaning | Expected hint/action |
| --- | --- | --- | --- |
| `L001` | lexer | unexpected character | remove the character or escape it appropriately |
| `L002` | lexer | unterminated string literal | close the string with `"` |
| `L003` | lexer | unterminated character literal | close the literal with `'` |
| `L004` | lexer | empty character literal | provide exactly one character |
| `L005` | lexer | unterminated f-string literal | close the f-string with `"` |
| `L006` | lexer | unterminated block comment | close the comment with `*/` |
| `P001` | parser | expected keyword | insert the missing keyword or fix item order |
| `P002` | parser | expected or synthesized symbol | insert the required delimiter such as `;`, `)`, or `}` |
| `P003` | parser | expected identifier | provide a valid identifier in the current grammar slot |
| `P004` | parser | experimental feature disabled | rerun with `--enable-experimental <feature>` |
| `P999` | parser | generic syntax failure | inspect nearby syntax; parser context and hint should narrow the issue |
| `E001` | semantic | undefined variable or function | declare/import the symbol or fix the name |
| `E002` | semantic | argument count mismatch | pass the expected number of arguments |
| `E003` | semantic | function return mismatch | align the returned value with the declared return type |
| `E004` | semantic | type mismatch in expression or assignment | convert or correct the incompatible type |
| `E005` | semantic | invalid field or member access | use an existing field/member on the target type |
| `E006` | semantic | invalid method call or method resolution failure | call a method that exists for the target type and signature |
| `E007` | semantic | `break` outside loop | move `break` into a loop body |
| `E008` | semantic | `continue` outside loop | move `continue` into a loop body |
| `E009` | semantic | generic or trait-bound inference failure | provide required type arguments or satisfy the bound |
| `E010` | semantic | unsatisfied generic trait bound | implement the required trait for the concrete type before calling the generic function |
| `E011` | semantic | unknown qualified module member | use an exported member from the module or import alias; inspect the candidate export list |

This satisfies the Phase 1 acceptance target of at least 20 high-frequency diagnostics with actionable remediation guidance.

## Machine-Readable JSON Diagnostics

Current CLI contract:

- `spectralang compile --json <path>`
- `spectralang check --json <path>`
- `spectralang lint --json <path>`
- `spectralang repl --json <path>`

JSON schema shape:

```json
{
  "version": 1,
  "success": false,
  "files": [
    {
      "path": "D:/Lang/SpectraLang/tests/errors/type_mismatch.spectra",
      "diagnostics": [
        {
          "severity": "error",
          "code": "E004",
          "message": "type mismatch",
          "phase": "semantic",
          "hint": "expected action here",
          "range": {
            "start": { "line": 4, "column": 12 },
            "end": { "line": 4, "column": 20 }
          },
          "related": []
        }
      ]
    }
  ]
}
```

Tooling expectations:

- `code` is stable when the compiler emitted one
- `phase` remains available even when `code` is absent
- `hint` is optional but should be consumed when present
- `related` can contain additional context-only messages without a span

## Machine-Readable SARIF Diagnostics

Current CLI contract:

- `spectralang compile --sarif <path>`
- `spectralang check --sarif <path>`
- `spectralang lint --sarif <path>`

SARIF output uses version `2.1.0` and writes one `SpectraLang` run. Each
diagnostic becomes a SARIF result:

- `ruleId` is the stable diagnostic code when present, otherwise the phase
- `level` is `error` or `warning`
- `message.text` is the compiler diagnostic message
- `locations[0].physicalLocation.artifactLocation.uri` is the source path
- `locations[0].physicalLocation.region` contains line/column data
- `properties.hint` is emitted when a fix/action hint exists
- `relatedLocations` carries additional context when present

`--json` and `--sarif` are mutually exclusive. Both formats return exit code
`65` when compilation/lint diagnostics contain errors.
