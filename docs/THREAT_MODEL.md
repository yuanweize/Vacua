# Threat Model: Project Reclaim

This document details the threat analysis and defensive mitigations implemented in Project Reclaim to ensure safe, secure, and robust storage operations.

---

## 1. Threat Vectors and Defensive Mitigations

### 1.1 Malicious Symlink Redirection
- **Threat**: An attacker or malicious installer creates a symbolic link inside a legitimate cache directory (e.g. `~/Library/Caches/evil/`) pointing to a sensitive file (`~/.ssh/id_ed25519` or `/etc/passwd`). If the scanner or cleaner recursively traverses symlinks, critical targets could be exposed or deleted.
- **Mitigation**:
  - The scanner uses `symlink_metadata` and POSIX `lstat(2)`.
  - Traversal **NEVER** follows directory symlinks by default.
  - Deletion logic validates whether a target is a symlink using `O_NOFOLLOW` / `symlink_metadata`. If a symlink is targeted, only the link itself is removed; its target is never traversed or mutated.

### 1.2 Time-of-Check to Time-of-Use (TOCTOU)
- **Threat**: A candidate file is scanned, classified as `SAFE`, and included in a plan. Before execution, a malicious or concurrent process replaces the safe file with a hard link or directory containing valuable user documents.
- **Mitigation**:
  - Cleanup plans record inode identity (`st_ino`), device ID (`st_dev`), file type, and modification timestamp (`st_mtime`).
  - Pre-execution validation verifies that all metadata matches identically.
  - On Darwin, operations verify file descriptors or perform atomicity checks via `open(..., O_NOFOLLOW)` and `unlinkat`.

### 1.3 Path Traversal Attacks
- **Threat**: Rule definitions or external inputs contain `..`, relative navigation, or encoded slashes attempting to target paths outside the allowed workspace or cache directory.
- **Mitigation**:
  - All paths are canonicalized (`fs::canonicalize`) before evaluation.
  - Path components are strictly inspected; paths containing relative `..` sequences are rejected.
  - Hard-coded protected boundaries are checked against canonical absolute paths.

### 1.4 Mount Substitution & Boundary Traversal
- **Threat**: An external NFS, SMB, or malicious DMG volume is mounted inside a scanned tree, causing the scanner to traverse network volumes or delete remote assets.
- **Mitigation**:
  - Scanner tracks the root filesystem device ID (`st_dev`).
  - By default, boundary crossing across device boundaries (`cross_mounts = false`) is forbidden.
  - Time Machine backup mounts (`/Volumes/Time Machine...`) and system volumes are explicitly blocked.

### 1.5 Hardlink Confusion
- **Threat**: An attacker hardlinks a sensitive file into a cache directory. A naive size calculator or cleaner would either double-count sizes or delete the underlying inode data.
- **Mitigation**:
  - Scanner tracks `(st_dev, st_ino)` sets to eliminate duplicate size calculations.
  - Reclaim verifies link count (`st_nlink > 1`). Candidates with multiple hardlinks are flagged for review and not automatically purged.

### 1.6 Race Condition During Deletion
- **Threat**: A user or background service creates files inside a candidate directory while deletion is underway, resulting in partial deletions or unintended deletion of freshly written data.
- **Mitigation**:
  - Whenever possible, Reclaim moves entire target directories to the native macOS Trash in a single atomic operation (`FileManager.trashItem`).
  - Re-evaluates active process guards immediately before execution.

### 1.7 Privilege Escalation Prevention
- **Threat**: Running with elevated permissions (`sudo`) might allow unintended damage to system-owned files or root directories.
- **Mitigation**:
  - Reclaim explicitly refuses to run as `root` for general user cleanup operations.
  - No privileged helper tool is included in the base architecture.
  - Adheres strictly to standard user permissions and macOS TCC sandboxing rules.

### 1.8 Malicious Rule File Ingestion
- **Threat**: A user downloads an untrusted community rule file containing malicious deletion paths or patterns designed to destroy user data.
- **Mitigation**:
  - Rules are purely declarative data (TOML format), completely incapable of executing arbitrary code, shell commands, or network requests.
  - Every rule is evaluated against the invariant protected layers: even if a rule matches `~/.ssh/**` or `~/Documents/**`, the invariant engine rejects the candidate as `PROTECTED`.
  - Versioned rule schemas with cryptographic checksums.

### 1.9 MCP Prompt Injection & Malicious LLM Instructions
- **Threat**: An LLM agent is tricked via indirect prompt injection into attempting to execute destructive deletions or exfiltrate private file paths.
- **Mitigation**:
  - The `reclaim-mcp` server exposes **READ-ONLY** tools by default (`scan_status`, `list_candidates`, `explain_candidate`, `build_cleanup_plan`).
  - MCP has zero raw `rm`, `delete`, or `shell` tool endpoints.
  - Execution requires external user confirmation and a cryptographic plan hash verification.

### 1.10 Tampered Cleanup Plan
- **Threat**: A plan file stored on disk is modified by a rogue process before execution to insert unauthorized paths.
- **Mitigation**:
  - Cleanup plans contain a SHA-256 HMAC / cryptographic hash of all canonical items, metadata fingerprints, and plan parameters.
  - If the computed hash of the plan does not match, execution is completely rejected.

### 1.11 Untrusted External Volumes
- **Threat**: Connecting an untrusted USB drive or disk image could trigger automated scans or deletions.
- **Mitigation**:
  - External volumes require explicit user opt-in (`reclaim scan --volume /Volumes/External`).
  - No background daemon automatically acts on external drive insertion.

### 1.12 Rule Supply-Chain Attack
- **Threat**: A compromised upstream dependency or malicious community PR introduces subtle bypasses to built-in rules.
- **Mitigation**:
  - All built-in rules are covered by automated unit and integration tests against synthetic directory sandboxes.
  - Invariants are hard-coded in Rust, independent of the external rule definition files.
