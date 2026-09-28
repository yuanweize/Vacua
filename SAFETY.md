# Safety Invariants and Guarantees

> **The Reclaim Safety Law**:  
> *When certainty decreases, automation must decrease.*  
> *Under no circumstances may an automated process or AI model elevate an item's safety classification.*

---

## 1. Absolute Invariants

The following invariants are implemented in code and enforced at compile-time and run-time across all crates:

1. **`PROTECTED` Invariant**: Any path matching protected system rules, user data locations, or security keys can NEVER be deleted, modified, or placed into an automated cleanup plan.
2. **`UNKNOWN` Invariant**: Any candidate with an `UNKNOWN` category or indeterminate evidence is classified as `UNKNOWN` risk and CANNOT be automatically cleaned.
3. **No Unbounded Deletion**: Wildcard deletes (`rm -rf *` style) are forbidden. Every item targeted for removal must have a verified canonical path and file identity.
4. **Plan-Executor Separation**: The filesystem scanner and risk engine CANNOT mutate the filesystem. Deletions can only be executed through an immutable `CleanupPlan` after user approval.
5. **TOCTOU Defense**: Before executing removal of any path, the executor re-evaluates:
   - Path existence.
   - Device and Inode identity (`st_dev`, `st_ino`).
   - File type and size stability.
   - Guard conditions (e.g., active processes).  
   If any check fails, the item is skipped with a logged explanation.

---

## 2. Hard-Coded Protected Zones

The following paths and categories are statically protected in `reclaim-core::invariants`:

- **System and OS Core**:
  - macOS System Volume (`/System`, `/usr`, `/bin`, `/sbin`).
  - SIP-protected directories (`/private/var/db`, `/System/Library`).
  - macOS Recovery and APFS Preboot volumes.
- **Security & Credentials**:
  - Keychains (`~/Library/Keychains`).
  - SSH keys and config (`~/.ssh`).
  - GPG keys (`~/.gnupg`).
  - Cloud provider & container credentials (`~/.aws`, `~/.kube/config`, `~/.docker/config.json`).
  - Browser password and cookie databases.
- **Authoritative User Content**:
  - `~/Documents`, `~/Desktop`.
  - Photos Libraries (`*.photoslibrary`).
  - Mail databases (`~/Library/Mail`).
  - Messages databases (`~/Library/Messages`).
  - Active Git repositories (`.git/`).
  - Active source code workspaces.
- **System Backups**:
  - Time Machine backup mount points and directories.

---

## 3. Risk Classification Definitions

Every storage candidate is evaluated into exactly one of five discrete risk tiers:

| Tier | Definition | Automated Action Permitted? | User Approval Needed? | Example |
| :--- | :--- | :--- | :--- | :--- |
| **`SAFE`** | Completely reproducible generated cache/artifact with zero user data. Producing tool is not currently running. | Yes (in user-approved plans) | Plan confirmation | Xcode DerivedData (idle), Cargo target, Homebrew download cache |
| **`REVIEW`** | Reconstructable, but removal incurs rebuild latency or network bandwidth, or may affect offline workflows. | **No** | Yes | `node_modules`, Python `venv`, Docker build cache, Simulator runtimes |
| **`CAUTION`** | Leftover or stale application data with low confidence of ownership, or potentially containing user configuration. | **No** | Explicit per-item confirmation | Old Application Support folder of deleted app |
| **`PROTECTED`** | Critical system path, user document, security key, or active database. | **NEVER** | Not selectable | `~/.ssh/id_ed25519`, `~/Library/Keychains`, SIP paths |
| **`UNKNOWN`** | Path semantics or ownership cannot be verified with high confidence. | **NEVER** | Explicit manual inspection only | Arbitrary unrecognized file in `~/Library` |

---

## 4. Reversibility and Deletion Methods

1. **Trash by Default**:
   Whenever possible, operations utilize the native macOS Trash (`FileManager.trashItem` / `NSWorkspace.shared.recycle`). Items in Trash can be inspected and restored by the user via Finder.
2. **Permanent Cache Eviction**:
   Only verified, high-volume ephemeral caches (e.g., package manager HTTP caches, compiler intermediate artifacts) where Trash operations would cause massive inode thrashing may be permanently deleted.
   - Such items must be explicitly documented as `non-restorable generated cache deletion`.
   - The UI and CLI must never deceptively claim "Undo available" for permanent cache purges.

---

## 5. Storage Accounting Honesty

- **Logical Size (`st_size`)**: The theoretical byte length of file content.
- **Allocated Size (`st_blocks * 512`)**: The true physical blocks reserved on disk.
- **Sparse Files**: Sized strictly by allocated blocks, never inflated logical length.
- **Hard Links**: Inodes are tracked (`st_dev`, `st_ino`). Hard links sharing an inode are counted only once in allocated totals.
- **APFS Clones**: When multiple files share extents via APFS clonefile, savings are flagged as `physical saving uncertain` rather than fabricating an exact deduction.
- **Purgeable Space**: System purgeable space is reported as `System Purgeable (managed by macOS)`, distinct from `User Reclaimable`.
