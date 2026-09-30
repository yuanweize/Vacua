# ADR 0008: Project Licensing Consolidation to Apache-2.0

## Status
Accepted (v0.8.0)

## Context
Prior to v0.8.0, the Vacua repository utilized a dual-licensing scheme (`MIT OR Apache-2.0`).

While dual licensing is common across generic Rust libraries, it introduces unnecessary ambiguity for an end-user macOS systems application and intelligence engine:
1. **Packaging and Downstream Complexity**: Downstream packagers (such as Homebrew formulae) required compound license declarations (`license all_of: ["MIT", "Apache-2.0"]`), complicating distribution metadata.
2. **Ambiguity in Patent Grant**: The MIT license contains no express patent license grant, whereas the Apache License 2.0 explicitly grants patent rights from contributors and includes protective patent litigation termination clauses.
3. **Distribution Consistency**: Distributing binary packages with multiple competing licenses creates redundant legal files and user-facing ambiguity.

An audit of the repository git history confirmed that all contributions are authored by the project maintainer (`Weize Yuan <iyuanweize@gmail.com>`), with no external third-party proprietary or restricted code vendored into the codebase.

## Decision

We consolidate Vacua's project licensing exclusively to the **Apache License 2.0** (`Apache-2.0`):

1. **Single Canonical License File**:
   Replace `LICENSE-MIT` and `LICENSE-APACHE` with a single canonical `LICENSE` file containing the unmodified Apache License 2.0 text.
2. **Workspace Cargo Metadata**:
   Configure `[workspace.package].license = "Apache-2.0"` across all project crates.
3. **Downstream Packaging**:
   Declare `license "Apache-2.0"` in the Homebrew formula template and downstream distribution specs.
4. **Third-Party Dependency Policy**:
   Third-party dependencies retain their own respective open-source licenses. Automated dependency audits (`cargo-deny` with `deny.toml`) strictly enforce permissive open-source licenses (`Apache-2.0`, `MIT`, `BSD`, `CC0-1.0`, `Unlicense`, `Zlib`, `Unicode-3.0`).
5. **No Artificial NOTICE**:
   In accordance with Apache-2.0 guidelines, no `NOTICE` file is created unless required to preserve factual upstream attribution.

## Consequences

- **Permissive Redistribution**: Permissive redistribution and commercial use remain fully authorized.
- **Explicit Patent License**: Users and contributors benefit from the explicit patent license grants of Section 3 of Apache-2.0.
- **Standard SPDX Identity**: Single SPDX identifier `Apache-2.0` simplifies compliance tooling.
- **Single-License Simplicity**: Eliminates dual-license wording across README, UI, and release packages.
