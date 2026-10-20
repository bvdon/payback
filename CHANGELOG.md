# Changelog

What changed in each version of Payback. Downloads are on the [releases page](https://github.com/bvdon/payback/releases).

## 0.1.3 — 2026-10-19
- **Linux:** Payback now comes as a `.deb` (Ubuntu, Debian), an `.rpm` (Fedora) and an `.AppImage` (most other distributions). They're built on Ubuntu 22.04, so they run on older systems too.
- The README and the release notes have install steps for Linux.

## 0.1.2 — 2026-10-19
- **Check for Updates…** on the Mac, in the Payback menu under About. It opens the latest release on GitHub. It doesn't compare versions or download anything.

## 0.1.1 — 2026-10-19
- **Lender and borrower:** each loan records who lent the money and who borrowed it. Both are required when you create or edit a loan, and the form suggests names used on other loans.
  - They appear in the page heading ("Rob lent Sam $X on …"), the sidebar and the PDF statement.
  - Loans made in 0.1.0 have them blank and show an "Add who lent and who borrowed" link. Files stay compatible both ways, so 0.1.0 can still open them.
- First release published on GitHub, with Mac and Windows installers built by GitHub Actions.

## 0.1.0 — 2026-10-19
First version, built locally for Mac and Windows only (not published on GitHub).
- Any number of loans, each with a name, the amount lent and a start date.
- Payments with an amount, a date, a type (Check, PayPal, Venmo, Cash or Other) and an optional note. You can edit them, and delete them with undo.
- The balance remaining is always worked out from the payments, with a running balance on each payment. Overpaying is allowed and shows as "overpaid by $X".
- Statements as PDF or CSV.
- Sharing between two computers with a `.payback` file. Importing shows exactly what will change before anything is replaced.
- An automatic daily backup (keeps the last 14), plus back up now and restore.
- Light and dark mode.
