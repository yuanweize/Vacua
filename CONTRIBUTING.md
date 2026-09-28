# Contributing to Vacua

We welcome contributions to Vacua! Before contributing, please review our core engineering values and guidelines.

---

## Engineering Values

1. **Understand Storage Before Deleting Storage**: Every PR introducing cleanup capabilities must prioritize safety, deterministic evidence, and rebuild consequences.
2. **No Deceptive Marketing**: We do not exaggerate reclaimed space, make fake benchmark claims, or treat purgeable system space as freeable memory.
3. **Fail-Closed Safety**: Invariants (`PROTECTED` and `UNKNOWN`) are non-negotiable. No pull request may weaken these invariants.

---

## Development Workflow

1. Fork and clone the repository.
2. Ensure you have Rust 1.80+ installed on macOS.
3. Format and lint checks:
   ```bash
   cargo fmt --check
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   ```
4. Run tests:
   ```bash
   cargo test --all
   ```
5. All destructive filesystem tests must operate inside disposable temporary directories (via `tempfile::tempdir`). Never write tests against real `~/Library` paths.

---

## Adding New Declarative Rules

Community rules should be submitted as `.toml` files under `rules/<category>/` following [docs/RULE_FORMAT.md](docs/RULE_FORMAT.md).
Every rule must include:
- A unique reverse-DNS identifier (e.g., `developer.tool.cache`).
- A human-readable description and author reference.
- Deterministic process guards or minimum age guards where applicable.
- Concrete test fixture verifying that the rule matches expected paths.
