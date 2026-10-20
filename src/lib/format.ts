import type { Method } from '../api/types';

export const METHODS: { key: Method; label: string }[] = [
  { key: 'check', label: 'Check' },
  { key: 'paypal', label: 'PayPal' },
  { key: 'venmo', label: 'Venmo' },
  { key: 'cash', label: 'Cash' },
  { key: 'other', label: 'Other' },
];

export const methodLabel = (m: Method) => METHODS.find((x) => x.key === m)?.label ?? m;

/** "$1,234.56" from integer cents (exact; no floating point). */
export function money(cents: number): string {
  const neg = cents < 0;
  const c = Math.abs(Math.trunc(cents));
  const whole = Math.floor(c / 100)
    .toString()
    .replace(/\B(?=(\d{3})+(?!\d))/g, ',');
  return `${neg ? '-' : ''}$${whole}.${String(c % 100).padStart(2, '0')}`;
}

/** Cents as an editable amount: 25050 -> "250.50". */
export function amountInput(cents: number): string {
  return `${Math.floor(cents / 100)}.${String(cents % 100).padStart(2, '0')}`;
}

/**
 * A quick read of a typed amount, used only for live hints (like the
 * overpayment warning). The core does the real parsing and validation.
 */
export function roughCents(text: string): number | null {
  const m = /^\s*\$?\s*([\d,]*)(?:\.(\d{0,2}))?\s*$/.exec(text);
  if (!m || (!m[1] && !m[2])) return null;
  const dollars = Number(m[1].replace(/,/g, '') || '0');
  const cents = Number((m[2] ?? '').padEnd(2, '0') || '0');
  return Number.isFinite(dollars) ? dollars * 100 + cents : null;
}

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

/** "Oct 19, 2026" from "2026-10-19". */
export function formatDate(iso: string): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
  if (!m) return iso;
  return `${MONTHS[Number(m[2]) - 1]} ${Number(m[3])}, ${m[1]}`;
}

/** Today on this computer's calendar, as YYYY-MM-DD. */
export function todayIso(): string {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

export const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;
