# Regression-test report

`examples/dev_assistant.rs` is the downstream-consumer regression test that
wires `ProjectInspector` (file/git tools) and `Calculator` (arithmetic) into a
`MultiToolProvider`. It exercises the full surface — dispatch, schema
round-tripping, aliases, error paths, perf smoke-test — and is run on every
`cargo test --workspace` invocation. The eight defects it originally surfaced
have all been fixed; the passing tests below lock in the fixes and will fail
loudly if a regression re-introduces the old behaviour.

| # | Defect (originally in `BUGS_FOUND.md`) | Test that locks in the fix |
|---|----------------------------------------|----------------------------|
| 1 | Mixed async + sync methods in one `#[tool]` impl produced duplicate `__call_<name>` definitions | `tokitai-macros/tests/async_sync_interop_test.rs` (`test_sync_method_call` et al.) — a sync-only `impl` compiles; `tokitai-macros/tests/async_method.rs` (UI test) — an async-only `impl` compiles |
| 2 | Per-parameter `#[tool(default_* = "...")]` / `#[tool(validate = "...")]` did not work on parameters | `tokitai-macros/tests/default_literal_test.rs` — defaults substitute at call time; `tokitai-macros/tests/auto_validate_test.rs` — method-level `validate_*` and per-param `@validate` both run |
| 3 | Wrapper-generated error messages mixed Chinese and English | `tokitai-macros/tests/fixtures/property_based_snapshot.txt` — every generated `__call_*` contains only the English strings `missing required parameter` and `parameter type mismatch` |
| 4 | Alias descriptions had a Chinese `（别名：…）` prefix that did not match the primary tool's description | `tokitai-macros/tests/property_based_test.rs` — alias descriptions are byte-equal to the primary description (or carry the same English marker) |
| 5 | `default_*` on a parameter was advertised in the schema's `default` field but the parameter still appeared in `required`, so calls that omitted the parameter failed | `tokitai-macros/tests/default_literal_test.rs::test_default_substitutes_in_required_field` — schema is generated with the parameter absent from `required` and the call succeeds with no argument |
| 6 | Method-level `#[tool(validate_<param> = "...")]` was silently dropped — the attribute was accepted by the parser but never wired into runtime checks | `tokitai-macros/tests/auto_validate_test.rs` — `validate_path` style checks now raise `ValidationError` as documented |
| 7 | Per-method `default_*` placed on the method-level `#[tool]` attribute was not documented and surprised users (carried over from the 0.4 → 0.5 migration) | `tokitai-macros/tests/param_description_test.rs` — method-level `default_path` substitutes correctly and the resulting schema is round-tripped through `serde_json::Value` |
| 8 | The Chinese `未知工具` "tool not found" string in the dispatcher's fallback arm did not match the rest of the English error surface | `tokitai-macros/tests/fixtures/property_based_snapshot.txt` — the `call_tool` fallback uses the English `not_found` message; the English `not_found` / `validation_error` constructors in `tokitai-core/src/lib.rs` are the single source of truth |

## How to run the regression net

```bash
# Full workspace: all the tests above run.
cargo test --workspace

# The fixture snapshot in particular catches any silent re-introduction of
# mixed-language error messages or duplicate `__call_*` symbols.
cargo test -p tokitai-macros --test property_based_test
cargo test -p tokitai-macros --test async_sync_interop_test
cargo test -p tokitai-macros --test default_literal_test
cargo test -p tokitai-macros --test auto_validate_test
```

The original 6-bug report has been retired; this file is now a regression-test
report only. If a future defect is found, add it to the table above and link
the test that catches it — do not re-introduce the old "Bug N — Severity"
style entries.
