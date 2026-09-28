# Security Policy

## Reporting Security Vulnerabilities

Project Reclaim treats security and data safety as our highest engineering priority. If you believe you have discovered a vulnerability that could lead to unauthorized deletion, privilege escalation, path traversal, or race-condition exploitation, please report it responsibly.

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, please send an encrypted or private report via GitHub Private Vulnerability Reporting or contact the security team directly.

---

## Security Principles

1. **Least Privilege**: Reclaim does not require `root` privileges. Standard scanning and cleanup operate strictly under the user's unprivileged desktop permissions.
2. **Fail-Closed Safety Invariants**: Any ambiguous, unknown, or corrupted candidate defaults to `UNKNOWN` or `PROTECTED` and cannot be deleted.
3. **Defense Against Filesystem Races**: Destructive operations use two-phase verification to defeat Time-of-Check to Time-of-Use (TOCTOU) exploits and symlink swapping.
4. **Data Isolation**: Reclaim is 100% local-first with zero telemetry. Filesystem paths, candidate filenames, and execution records never leave your local device.

For a comprehensive threat analysis, refer to [THREAT_MODEL.md](file:///Users/yuanweize/我的文档/服务器/GITHUB/reclaim/docs/THREAT_MODEL.md).
