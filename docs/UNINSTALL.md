# Uninstalling Vacua

Vacua respects your system and does not leave background daemons, launch agents, or hidden background services.

---

## 1. Remove Binaries

### If installed via Homebrew:
```bash
brew uninstall vacua
```
To also remove the tap repository (optional):
```bash
brew untap yuanweize/tap
```

### If installed manually:
Remove the binaries from your install destination:
```bash
rm -f /usr/local/bin/vacua /usr/local/bin/vacua-intelligence
# Or if installed in ~/.local/bin:
rm -f ~/.local/bin/vacua ~/.local/bin/vacua-intelligence
```

---

## 2. Remove Shell Completions (Optional)
If you manually placed completion files in your shell directories:
```bash
rm -f /usr/local/share/zsh/site-functions/_vacua
rm -f /usr/local/share/bash-completion/completions/vacua
rm -f /usr/local/share/fish/vendor_completions.d/vacua.fish
```

---

## 3. Remove Local State and Journal (Optional)
Vacua stores its local SQLite cache database and transaction audit history in:
```text
~/.vacua/
├── index.db     # Incremental filesystem index
└── journal.db   # Audit log of past cleanup transactions
```

To purge all local state and audit records:
```bash
rm -rf ~/.vacua
```

*Note: Deleting `~/.vacua` does not touch any files in your `~/.Trash` or your active filesystem.*
