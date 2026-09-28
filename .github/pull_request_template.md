## Description
Briefly describe what changes this PR introduces and the problem it solves.

## Verification
- [ ] `cargo fmt --check` passes
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes
- [ ] `cargo test --workspace --all-features` passes
- [ ] Added or updated automated tests
- [ ] No hardcoded assumptions violating safety invariants (`PROTECTED` or `UNKNOWN`)

## Threat & Safety Impact
Explain whether this PR affects destructive logic, filesystem traversal, or rule evaluation.
