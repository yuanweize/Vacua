# Privacy Policy

**Project Reclaim is 100% Local-First with Zero Telemetry.**

---

## 1. What We Collect

**Nothing.**  
By default, Project Reclaim contains zero analytics, zero telemetry, zero error reporting beacons, and zero network calls during local scanning, evaluation, and cleanup planning.

- We do **not** upload file paths, directory trees, or file contents.
- We do **not** track application names or bundle identifiers.
- We do **not** upload hardware serial numbers, MAC addresses, or IP addresses.
- All SQLite metadata indices and transaction audit logs reside strictly on your local device.

---

## 2. Network Activity Boundaries

Reclaim will only perform outbound network requests in the following explicit user-initiated scenarios:
1. **Formula / Binary Updates**: When the user explicitly runs `brew upgrade reclaim` or checks GitHub Releases for new versions.
2. **External Rule Downloads**: When the user explicitly configures a remote community rule repository.

---

## 3. Future AI & LLM Disclosures (Phase 6)

If optional cloud AI assistance is enabled by the user in future releases:
- Only sanitized, high-level category metadata (e.g. `Xcode DerivedData: 5 GB, idle 45 days`) may be sent to the model.
- Raw file paths containing personal names, usernames, or confidential project directories will be sanitized and redacted locally prior to any external request.
- On-device models (macOS Foundation Models / Apple Intelligence) will be prioritized whenever hardware permits.
