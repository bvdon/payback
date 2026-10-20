# Payback

A small desktop app for tracking simple, interest-free personal loans, for Mac and Windows.

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

## Installing
Download the installer from the GitHub Releases page.
- **Mac:** open the `.dmg` and drag Payback to Applications. The first time, macOS says it can't verify the app. Open **System Settings → Privacy & Security** and click **Open Anyway**.
- **Windows:** run the `-setup.exe`. If Windows shows "Windows protected your PC", click **More info → Run anyway**.

These warnings appear because the app isn't signed with a paid developer certificate.

## Development
Requirements: Rust (stable), Node 22+.

```bash
npm install
npm run app        # desktop app with live reload
npm run browser    # UI in a normal browser at http://127.0.0.1:1422
cargo test         # core tests
npm run release    # local Mac DMG in releases/
```

To release for both platforms, push a version tag (for example `git tag v0.1.0 && git push origin v0.1.0`). GitHub Actions then builds the Mac `.dmg` and the Windows installers into a draft release.

MIT License.
