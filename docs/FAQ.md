# Frequently Asked Questions (FAQ)

### Is Vacua safe?
**Yes.** Vacua operates under a strict, multi-layered fail-closed safety model:
1. **Read-Only by Default**: Commands such as `scan`, `candidates`, `explain`, and `plan` never touch or modify the filesystem.
2. **Deterministic Classification**: Files are categorized using structured evidence vectors and rule definitions. Unrecognized files are classified as `UNKNOWN` and can never be marked as `SAFE`.
3. **Protected Path Hard Invariant**: Sensitive system paths (`/System`, `/usr/bin`), user security keys (`~/.ssh`, `~/.gnupg`, `~/Library/Keychains`), photos libraries, mail stores, and git repositories (`.git/`) are unconditionally blocked at both compile-time and runtime.
4. **TOCTOU Verification**: Before moving any item to the Trash during execution, Vacua re-reads live filesystem metadata and verifies device ID, inode, and modification time against the plan. Any discrepancy causes an immediate skip.
5. **Reversible by Default**: Approved cleanups move files to the native macOS Trash (`~/.Trash`), allowing standard Finder restore.

---

### Does Vacua use AI to delete files?
**No. Absolutely not.** 
Vacua's safety invariants and cleanup executions never depend on AI or Large Language Models. 
Apple Foundation Models / on-device LLMs are used solely as an optional natural-language query parser (converting queries like *"safely free 5 GB build cache"* into a strongly typed `StructuredIntent`). 
The resulting intent is then processed by deterministic rule engines, bounded by hard safety invariants, compiled into a SHA-256 verified plan, presented to the human user for explicit approval, and executed by the verified executor. AI has zero direct deletion authority.

---

### Does Vacua require Full Disk Access (FDA)?
**No.** Vacua runs in standard unprivileged user space. 
When Full Disk Access is not granted, Vacua gracefully inspects user-accessible paths and cleanly skips restricted system or container paths. Running `vacua doctor` will report the current FDA permission status without failing.

---

### Does Vacua send filenames or metadata to a server?
**No.** Vacua is 100% local-first:
- File scanning, APFS extent queries, and inode evaluations run entirely on your local machine.
- Apple Intelligence inference uses Apple's on-device Foundation Models with zero network egress.
- If the on-device model is unavailable, Vacua uses an offline, deterministic keyword parser.
- Zero analytics, telemetry, or remote reporting are compiled or executed.

---

### Why is reclaimed space different from logical file size?
On modern macOS APFS filesystems:
1. **Sparse Files**: Files may report a large logical size but only consume a fraction of physical disk blocks.
2. **File Clones (Copy-on-Write)**: APFS file copies share storage blocks. Deleting one clone does not free the shared physical blocks if another reference exists.
3. **Filesystem Block Allocation**: Physical disk blocks are allocated in 4 KiB clusters. Small files consume at least one full block.

Vacua uses low-level macOS system calls (`getattrlistbulk` with `ATTR_CMN_OWNED_BLOCKS` and extent descriptors) to report the true physical storage consumed by files rather than superficial byte counts.

---

### Does Vacua delete Time Machine snapshots?
**No.** Time Machine snapshots and backup stores (`/Backups.backupdb`, `/Volumes/Time Machine`, `/Volumes/.timemachine`) are hardcoded as protected locations. Vacua will never alter or purge Time Machine backups.

---

### Can I use Vacua without Apple Intelligence?
**Yes.** The Swift intelligence helper (`vacua-intelligence`) is completely optional. If Apple Intelligence is unavailable or not enabled on your machine:
- Core scanning, analysis, rule matching, candidate ranking, plan generation, and safe execution work 100% offline.
- Natural language queries automatically fall back to deterministic pattern extraction (`provider_used: deterministic-fallback`).

---

### Does Vacua support Intel Macs?
**v0.1.0 is officially verified and packaged for Apple Silicon (macOS `aarch64-apple-darwin`).**
While the Rust core can compile for `x86_64-apple-darwin`, Apple on-device Foundation Models require Apple Silicon hardware (M1/M2/M3/M4 or newer). Intel support remains on the future roadmap.

---

### How do I completely uninstall Vacua?
See [UNINSTALL.md](UNINSTALL.md) for full details. 

If installed via Homebrew:
```bash
brew uninstall vacua
```

To purge the local SQLite index and transaction audit journal:
```bash
rm -rf ~/.vacua
```
