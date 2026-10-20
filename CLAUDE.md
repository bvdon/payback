# Payback — project context

## What it is
Payback is a small desktop app for tracking simple, interest-free personal loans. It's local-first: no accounts, no cloud, no subscriptions. Each person keeps their own copy of the data on their own computer.

It's built the same way as Stint, a sibling time-tracker project: Tauri 2, a Rust core, and a React + TypeScript UI bundled with esbuild. When in doubt, follow Stint's conventions.

**Targets:** macOS (`.dmg`, universal Intel + Apple Silicon), Windows (`.msi` / NSIS `.exe` installer) and, from 0.1.3, Linux (`.deb`, `.rpm`, `.AppImage`, built only by GitHub Actions).

## Status
v0.1.3 built on 2026-10-19 (0.1.1 added lender and borrower, 0.1.2 added the Mac "Check for Updates…" menu item, 0.1.3 added Linux builds). All v1 features below work:
- 26 core tests pass.
- The UI was checked in a browser against the dev server, in light and dark mode.
- The PDF statement was checked visually.
- The universal Mac DMG builds and launches (`releases/Payback-0.1.3.dmg`).
- The Windows setup `.exe` is cross-compiled from a Mac (`releases/Payback-0.1.3-windows-setup.exe`).

- **GitHub:** public repo at https://github.com/bvdon/payback, created 2026-10-19. The first CI run passed on macOS and Windows. Releases v0.1.1 and v0.1.2 are both built by Actions and published, each with `.dmg`, `-setup.exe` and `.msi`. `/releases/latest` points to v0.1.2.

**Not done yet:**
- No installer has been run on a real Windows PC or Linux desktop.
- The Tauri window hasn't been clicked through by hand. Only the browser UI and the API were exercised.

## Features (v1)
- **Multiple loans.** Each loan has a name, an initial principal and a start date.
- **Lender and borrower** (added in 0.1.1). Each loan records who lent the money and who borrowed it.
  - Both are required when you create or edit a loan, and the form suggests names used on other loans.
  - They appear in the page heading ("Rob lent Sam $X on …"), the sidebar (Rob → Sam) and the PDF statement.
  - Loans from before 0.1.1 have them blank.
    The page then shows an "Add who lent and who borrowed" link.
- **The loan list** shows each loan's balance remaining.
- **Payments.** Each payment has:
  - amount (required, > 0)
  - payment date (required, defaults to today)
  - payment type (required, a dropdown with `Check`, `PayPal`, `Venmo`, `Cash`, `Other`)
  - note (optional, free text)
- **Edit and delete payments.** The balance recalculates. Deleting asks for confirmation or offers undo.
- **Balance remaining = principal − sum of payments.** It is always computed and never stored, so it can't drift. Payments are listed newest first, with a running balance per row. A payment that would go below zero gets a warning, but is allowed, and the balance shows as "overpaid by $X".
- **Export statement.** A readable statement of a loan (name, principal, start date, every payment, totals, balance) as PDF and CSV, to share or print.
- **Export/import data file (sharing between two people).** One person exports a loan as a `.payback` file (JSON) and the other imports it.
  - Every loan and payment gets a permanent random ID when it's created. Importing a loan whose ID already exists **replaces** the local copy, after a confirmation that shows what changes (payments added, removed or edited, and the balance before and after). A loan with an unknown ID gets added.
  - The file carries a `format_version` so later versions can still read old files.
- **Backups.** An automatic daily snapshot of the data file (keep 14), plus back up now and restore. Restore validates the file and saves a safety copy first.
- Light and dark mode.

Not in v1: interest, payment schedules or due dates, adding money to the principal later, sync between computers, multiple currencies (USD only).

## Architecture
```
crates/payback-core       Rust library, NO third-party crates (std only), works on Mac and Windows
crates/payback-devserver  std-only HTTP server: serves dist/ + core API for browser dev (port 1422)
src-tauri                 Tauri 2 shell (separate Cargo project, excluded from root workspace)
src/                      React 19 + TypeScript UI, bundled with esbuild (scripts/build.mjs), not Vite
.github/workflows         ci.yml: tests + typecheck on mac, windows and linux. release.yml: installers on v* tags
```

### payback-core (crates/payback-core/src)
- `json.rs`, `pdf.rs`, `pdf_metrics.rs` and `csv.rs` are copied from Stint.
- `model.rs`: `Loan`, `Payment`, `Method`, JSON in/out with validation. `Loan::view()` is the UI shape: payments newest first plus `balanceAfterCents`, `paidCents`, `balanceCents`.
- `store.rs`: `Store`, which loads and atomically saves `payback.json`, and `parse_loans`, which reads data files and `.payback` export files.
- `ledger.rs`: create, update and delete loans and payments. Each one works on a copy, so a rejected change touches nothing.
- `transfer.rs`: `.payback` export, plus import `preview` and `apply`, matched by ID.
- `statement.rs`: the PDF and CSV statements. `backup.rs`: daily auto-backup (keeps 14), backup now, restore.
- `money.rs` parses typed amounts ("$1,234.50") to cents. `date.rs` validates YYYY-MM-DD.
- `api.rs` holds `Api::call(cmd, json)` and `Api::export(cmd, json)`. The commands are:
  - `app.bootstrap`
  - `loans.create|update|delete`
  - `payments.add|update|delete`
  - `import.preview|apply`
  - `export.statementPdf|statementCsv|data`

### src-tauri
- `main.rs` defines the commands `call`, `export_file`, `read_text_file`, `backup_now`, `restore_backup`, `app_paths` and `reveal`. `reveal` uses `open -R` on macOS and `explorer /select,` on Windows.
- The daily backup runs at launch and then every 30 minutes, and only actually saves one if the last is over 20 hours old.
- If `payback.json` exists but can't be read, every command returns that error. The UI shows "couldn't open its data", and the file is never overwritten.
- **Mac menu:** the standard Tauri menu plus "Check for Updates…" under About in the Payback menu. It opens https://github.com/bvdon/payback/releases/latest in the browser and doesn't compare versions. It's macOS-only, since Windows has no app menu. GitHub's `latest` skips drafts, so a release has to be published before the link reaches it.
- Plugins: dialog and single-instance. There's no tray. The normal title bar is used on both OSes.

### UI (src/)
- `App.tsx`: a sidebar listing the loans, a "New loan" button, Import shared file and Settings.
- `components/LoanPage.tsx`: a balance summary, the record-payment form (with an overpay hint), and the payments table with inline edit and delete-with-undo.
- `LoanForm.tsx` creates, edits and deletes loans. `ImportDialog.tsx` shows the preview with checkboxes. `SettingsDialog.tsx` covers export all, backups and the data location.
- Design: Stint's tokens (graphite paper, teal for money, amber for overpaid), defined in `styles.css` `:root`.

### Key decisions
- **Storage is a JSON file, not SQLite.** Stint links the Mac's built-in `libsqlite3`, which Windows doesn't have. The data here is tiny, so a single `payback.json` is plenty. Write it atomically: write `payback.json.tmp`, flush, then rename it over the original. Reuse or adapt Stint's `json.rs` (its own parser/serializer, no serde).
- **Data location** is Tauri's app data dir, with `backups/` beside it:
  - macOS: `~/Library/Application Support/dev.payback.desktop/`
  - Windows: `%APPDATA%\dev.payback.desktop\`
- **Money is integer cents** (`i64`). The UI sends amounts as the text the person typed (`amount`), and the core parses it (`money::parse_amount`). Undo sends exact `amountCents`. The UI's `roughCents` is only for the live overpay hint.
- **Dates are plain `YYYY-MM-DD` strings**, with no times and no time zones. The UI supplies "today". This avoids Stint's libc `localtime_r`/`mktime` code, which isn't portable to Windows.
- **IDs** are random 128-bit hex strings, so imports can match loans and payments across machines.
- **PDF:** reuse or adapt Stint's std-only PDF writer (`pdf.rs` + `pdf_metrics.rs`, base-14 Helvetica).
- **A single command interface** like Stint's: `Api::call(cmd, json_args) -> json`. The commands are listed under payback-core above.
- **Core rules live in the core**, not the UI: validation, balance math and user-facing error messages (written for people, not developers).

## Building and releasing
```bash
npm install
npm run app          # tauri dev (esbuild watch on :5174)
npm run browser      # build UI + devserver at http://127.0.0.1:1422 (real core, no Tauri)
npm run typecheck
cargo test           # core tests
npm run release      # scripts/release.sh: test, bump version, build both installers into releases/
```
- **The installers in `releases/`:**
  - `Payback-<v>.dmg` is the universal Mac app.
  - `Payback-<v>-windows-setup.exe` is the NSIS installer, the standard double-click Windows setup. It installs per-user, so no admin is needed, and fetches WebView2 if it's missing.
  - No `.msi` is built locally, because MSI needs a Windows host. GitHub Actions builds both `.msi` and `.exe`.
- **The Windows cross-build** uses Tauri's experimental cross-compile: `npx tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis`, with Homebrew's llvm on PATH.
  - One-time setup: `brew install nsis llvm`, `rustup target add x86_64-pc-windows-msvc`, and `cargo install cargo-xwin`. cargo-xwin needs the SDKROOT fix to install.
  - cargo-xwin downloads the MSVC CRT and Windows SDK, under Microsoft's license.
  - The `LNK4099 ... .pdb` linker warnings are harmless.
- **macOS SDK gotcha (seen on the dev Mac):** if the Command Line Tools are older than the default SDK, linking fails with `tapi error: malformed file ... arm64e` unless you prefix Rust/Tauri builds with `SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk`. `release.sh` sets it automatically when that SDK exists.
- **If DMG packaging fails** with `error running bundle_dmg.sh ... Searching for mounted interstitial disk image`, a Payback DMG is probably open in Finder (mounted at `/Volumes/Payback`). Eject it, and also eject any leftover `dmg.XXXX` volume (`hdiutil info` lists them). Then rerun. The script reuses the version, since it wasn't consumed.
- **After any change to the app's code** (Rust, UI, icons or Tauri config), finish with `npm run release` so `releases/` has current installers. It bumps the patch version by default. Keep the old DMGs. Skip this for changes that only touch docs.
- The icon source is `src-tauri/icons/icon-source.png` (drawn with PIL). To regenerate the other sizes, run `npx tauri icon icons/icon-source.png -o icons` from `src-tauri`, then delete the `android/` and `ios/` folders it creates.
- **Official releases come from GitHub Actions.**
  - `tauri-apps/tauri-action` runs on `macos-latest` (universal `.dmg`) and `windows-latest` (`.msi` + NSIS `.exe`).
  - Pushing a `v*` tag triggers it, and it uploads the installers to a draft GitHub Release.
  - It also runs on `ubuntu-22.04` (`.deb`, `.rpm`, `.AppImage`), after installing `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf`. 22.04 rather than latest, so the AppImage's glibc is old enough for older distros.
  - Those builds are the trusted ones for Windows. The local cross-build is a convenience.
  - **Linux is built only by Actions.** `release.sh` doesn't build it, and nothing has been run on a real Linux desktop yet. Linux data lives in `~/.local/share/dev.payback.desktop/`. There's no Check for Updates menu there (it's Mac-only), and `reveal` opens the containing folder with `xdg-open`.
- **The apps are unsigned** (no paid certificates):
  - macOS: ad-hoc signing (`signingIdentity: "-"`). First launch needs System Settings → Privacy & Security → Open Anyway.
  - Windows: SmartScreen shows "Windows protected your PC", so click More info → Run anyway. The installer will fetch WebView2 if it's missing (rare on Windows 10/11).
- **Version** lives in package.json, tauri.conf.json and the Cargo.toml files. Keep them in sync.

## Conventions
- Keep `payback-core` std-only and cross-platform. Don't use OS-specific code in the core.
- Once a version has shipped, any change to the data file shape needs a `format_version` bump and a migration path for older files.
  - The exception is a new **optional** field that reads as blank when missing, like `lender` and `borrower` in 0.1.1. Older app versions ignore unknown keys, so it doesn't need a bump.
- Requires macOS 12+, Windows 10+, or a Linux with WebKitGTK 4.1 (Ubuntu 22.04+, Fedora 36+ or similar).
