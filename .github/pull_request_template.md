## Summary

<!-- One or two sentences describing the change. -->

## Related Issue

<!-- Link the issue this PR closes, e.g. `Closes #123` or `Fixes #45`. -->

## Test Plan

<!-- Describe how you tested this change. List the commands you ran and their results. -->

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --workspace --all-features --all-targets -- -D warnings` passes
- [ ] `cargo test --workspace --all-features` passes
- [ ] New tests have been added for new behavior
- [ ] `cargo doc --workspace --all-features --no-deps -D warnings` passes

## Checklist

- [ ] I have read [CONTRIBUTING.md](../blob/main/CONTRIBUTING.md)
- [ ] My code follows the project's code style (`cargo fmt`)
- [ ] I have added documentation for any new public API
- [ ] I have added or updated tests for the change
- [ ] I have updated [CHANGELOG.md](../blob/main/CHANGELOG.md) under `[Unreleased]`

## Breaking Changes

<!-- If this PR contains a breaking change, describe the migration path here. -->
<!-- Otherwise, write "None" and explain why. -->

- [ ] This PR contains no breaking changes
- [ ] This PR contains breaking changes (describe migration path above)
