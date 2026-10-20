# Payback

A small desktop app for tracking simple, interest-free personal loans, for Mac, Windows and Linux.

## What it does
- Keeps any number of loans, each with a name, the amount lent and a start date.
- Records payments with an amount, a date, a type (Check, PayPal, Venmo, Cash or Other) and an optional note.
- Shows the balance remaining, recalculated after every payment, and a running balance on each payment.
- Edits and deletes payments, with undo.
- Exports a statement as PDF or CSV.
- **Shares between two computers:** export a `.payback` file and the other person imports it. The import shows exactly what will change before anything is replaced.
- Makes an automatic daily backup and keeps the last 14.

Your data stays on your computer, in one small file:
- **Mac:** `~/Library/Application Support/dev.payback.desktop/payback.json`
- **Windows:** `%APPDATA%\dev.payback.desktop\payback.json`
- **Linux:** `~/.local/share/dev.payback.desktop/payback.json`

## Installing
Download the installer for your computer from the [latest release](https://github.com/bvdon/payback/releases/latest).

| Platform | Download | Works on |
|---|---|---|
| **Mac** | `.dmg` | macOS 12 or later, Intel and Apple Silicon |
| **Windows** | `-setup.exe` or `.msi` | Windows 10 or 11 |
| **Linux** | `.deb`, `.rpm` or `.AppImage` | Ubuntu 22.04+, Debian 12+, Fedora 36+ and similar |

- **Mac:** open the `.dmg` and drag Payback to Applications. The first time, macOS says it can't verify the app. Open **System Settings → Privacy & Security** and click **Open Anyway**.
- **Windows:** run the `-setup.exe`. If Windows shows "Windows protected your PC", click **More info → Run anyway**.
- **Linux:** on Ubuntu or Debian, install the `.deb` with `sudo apt install ./Payback_*.deb`. On Fedora, install the `.rpm` with `sudo dnf install ./Payback-*.rpm`. On other distributions, make the `.AppImage` executable (`chmod +x Payback_*.AppImage`) and run it.

The Mac and Windows warnings appear because the app isn't signed with a paid developer certificate.

## Development
Requirements: Rust (stable), Node 22+.

```bash
npm install
npm run app        # desktop app with live reload
npm run browser    # UI in a normal browser at http://127.0.0.1:1422
cargo test         # core tests
npm run release    # local Mac DMG in releases/
```

To release for all three platforms, push a version tag (for example `git tag v0.1.0 && git push origin v0.1.0`). GitHub Actions then builds the Mac `.dmg`, the Windows installers and the Linux `.deb`, `.rpm` and `.AppImage` into a draft release.

MIT License.
