import { useState } from 'react';
import type { FormEvent } from 'react';
import { api } from '../api/api';
import type { LedgerPayment, Loan, Method, PaymentFields } from '../api/types';
import { METHODS, amountInput, formatDate, methodLabel, money, plural, roughCents, todayIso } from '../lib/format';
import { errorMessage, useStore } from '../state';
import { Icon, Menu } from './ui';

export function LoanPage({ loan, onEdit }: { loan: Loan; onEdit: () => void }) {
  const { toast } = useStore();
  const [editing, setEditing] = useState<string | null>(null);

  const exportWith = async (fn: () => Promise<string | null>, what: string) => {
    try {
      const path = await fn();
      if (path) toast(`Saved ${what}`);
    } catch (e) {
      toast(errorMessage(e), 'error');
    }
  };

  const paidPct = Math.min(100, Math.round((loan.paidCents / loan.principalCents) * 100));
  const over = loan.balanceCents < 0;
  const done = loan.balanceCents === 0;

  return (
    <div className="page">
      <header className="page-head">
        <div>
          <h1>{loan.name}</h1>
          <p className="muted">
            {loan.lender && loan.borrower ? (
              <>
                <strong className="person">{loan.lender}</strong> lent <strong className="person">{loan.borrower}</strong>{' '}
                {money(loan.principalCents)} on {formatDate(loan.startDate)}
              </>
            ) : (
              <>
                {money(loan.principalCents)} lent on {formatDate(loan.startDate)} ·{' '}
                <button className="link" onClick={onEdit}>
                  Add who lent and who borrowed
                </button>
              </>
            )}
          </p>
        </div>
        <div className="page-actions">
          <Menu
            label="Export"
            icon="download"
            items={[
              { label: 'Statement (PDF)', hint: 'To print or send', run: () => void exportWith(() => api.exportStatement(loan, 'pdf'), 'the statement') },
              { label: 'Statement (CSV)', hint: 'For a spreadsheet', run: () => void exportWith(() => api.exportStatement(loan, 'csv'), 'the CSV') },
              {
                label: 'Share file (.payback)',
                hint: 'For the other person to import',
                run: () => void exportWith(() => api.exportData([loan]), 'the share file'),
              },
            ]}
          />
          <button className="btn" onClick={onEdit}>
            <Icon name="pencil" />
            Edit loan
          </button>
        </div>
      </header>

      <section className="summary">
        <div className="balance">
          <span className="label">{over ? 'Overpaid by' : 'Balance remaining'}</span>
          <span className={`big ${over ? 'is-over' : done ? 'is-done' : ''}`}>
            {done ? 'Paid off' : money(Math.abs(loan.balanceCents))}
          </span>
        </div>
        <div className="progress" aria-label={`${paidPct}% paid`}>
          <div className={`progress-fill ${over ? 'is-over' : ''}`} style={{ width: `${paidPct}%` }} />
        </div>
        <div className="figures">
          <span>
            <strong>{money(loan.paidCents)}</strong> paid of {money(loan.principalCents)}
          </span>
          <span>{plural(loan.paymentCount, 'payment')}</span>
          {loan.lastPaymentDate && <span>Last payment {formatDate(loan.lastPaymentDate)}</span>}
        </div>
      </section>

      <section className="card">
        <h2>Record a payment</h2>
        <PaymentEditor loan={loan} />
      </section>

      <section className="card">
        <h2>Payments</h2>
        {loan.payments.length === 0 ? (
          <p className="muted empty-row">No payments yet. Recorded payments show up here, newest first.</p>
        ) : (
          <table className="ledger">
            <thead>
              <tr>
                <th>Date</th>
                <th>Type</th>
                <th>Note</th>
                <th className="num">Amount</th>
                <th className="num">Balance after</th>
                <th aria-label="Actions" />
              </tr>
            </thead>
            <tbody>
              {loan.payments.map((p) =>
                editing === p.id ? (
                  <tr key={p.id} className="editing">
                    <td colSpan={6}>
                      <PaymentEditor loan={loan} payment={p} onDone={() => setEditing(null)} />
                    </td>
                  </tr>
                ) : (
                  <PaymentRow key={p.id} loan={loan} p={p} onEdit={() => setEditing(p.id)} />
                ),
              )}
              <tr className="opening">
                <td>{formatDate(loan.startDate)}</td>
                <td>Loan</td>
                <td className="note muted">Amount lent</td>
                <td className="num">{money(loan.principalCents)}</td>
                <td className="num strong">{money(loan.principalCents)}</td>
                <td />
              </tr>
            </tbody>
          </table>
        )}
      </section>
    </div>
  );
}

function PaymentRow({ loan, p, onEdit }: { loan: Loan; p: LedgerPayment; onEdit: () => void }) {
  const { act, toast } = useStore();
  const remove = async () => {
    const gone = await act(() => api.deletePayment(loan.id, p.id));
    if (gone) {
      toast(`Deleted the ${money(gone.amountCents)} payment`, 'info', {
        label: 'Undo',
        run: () => void act(() => api.restorePayment(loan.id, gone)),
      });
    }
  };
  return (
    <tr>
      <td>{formatDate(p.date)}</td>
      <td>
        <span className={`pill pill-${p.method}`}>{methodLabel(p.method)}</span>
      </td>
      <td className="note">{p.note}</td>
      <td className="num">−{money(p.amountCents)}</td>
      <td className={`num strong ${p.balanceAfterCents < 0 ? 'is-over' : ''}`}>
        {p.balanceAfterCents < 0 ? `${money(-p.balanceAfterCents)} over` : money(p.balanceAfterCents)}
      </td>
      <td className="row-actions">
        <button className="icon-btn" onClick={onEdit} aria-label="Edit payment" title="Edit">
          <Icon name="pencil" size={15} />
        </button>
        <button className="icon-btn danger" onClick={() => void remove()} aria-label="Delete payment" title="Delete">
          <Icon name="trash" size={15} />
        </button>
      </td>
    </tr>
  );
}

/** The add-payment form, or the inline editor for an existing payment. */
function PaymentEditor({ loan, payment, onDone }: { loan: Loan; payment?: LedgerPayment; onDone?: () => void }) {
  const { act } = useStore();
  const blank: PaymentFields = { amount: '', date: todayIso(), method: 'venmo', note: '' };
  const [f, setF] = useState<PaymentFields>(
    payment ? { amount: amountInput(payment.amountCents), date: payment.date, method: payment.method, note: payment.note } : blank,
  );
  const [busy, setBusy] = useState(false);
  const set = (patch: Partial<PaymentFields>) => setF((x) => ({ ...x, ...patch }));

  // Balance this payment would be applied against.
  const available = loan.balanceCents + (payment?.amountCents ?? 0);
  const cents = roughCents(f.amount);
  const overBy = cents !== null && cents > 0 && cents > available ? cents - Math.max(available, 0) : 0;

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    const ok = await act(
      () => (payment ? api.updatePayment(loan.id, payment.id, f) : api.addPayment(loan.id, f)),
      payment ? 'Payment updated' : `Recorded ${money(cents ?? 0)} payment`,
    );
    setBusy(false);
    if (!ok) return;
    if (payment) onDone?.();
    else setF({ ...blank, method: f.method, date: f.date });
  };

  return (
    <form className={`payment-form ${payment ? 'is-inline' : ''}`} onSubmit={submit}>
      <label className="field f-amount">
        <span>Amount</span>
        <div className="money-input">
          <span>$</span>
          <input
            autoFocus={!!payment}
            inputMode="decimal"
            value={f.amount}
            onChange={(e) => set({ amount: e.target.value })}
            placeholder="0.00"
          />
        </div>
      </label>
      <label className="field f-date">
        <span>Date</span>
        <input type="date" value={f.date} onChange={(e) => set({ date: e.target.value })} />
      </label>
      <label className="field f-method">
        <span>Type</span>
        <select value={f.method} onChange={(e) => set({ method: e.target.value as Method })}>
          {METHODS.map((m) => (
            <option key={m.key} value={m.key}>
              {m.label}
            </option>
          ))}
        </select>
      </label>
      <label className="field f-note">
        <span>
          Note <em>optional</em>
        </span>
        <input value={f.note} onChange={(e) => set({ note: e.target.value })} maxLength={500} placeholder="e.g. birthday money" />
      </label>
      <div className="f-actions">
        {payment && (
          <button type="button" className="btn" onClick={onDone}>
            Cancel
          </button>
        )}
        <button type="submit" className="btn btn-primary" disabled={busy || !f.amount.trim()}>
          {payment ? (
            'Save'
          ) : (
            <>
              <Icon name="plus" />
              Add payment
            </>
          )}
        </button>
      </div>
      {overBy > 0 && (
        <p className="hint-over">
          {available <= 0
            ? `This loan is already paid off. Saving this will record ${money(overBy)} overpaid.`
            : `That’s ${money(overBy)} more than the ${money(available)} still owed. You can still save it.`}
        </p>
      )}
    </form>
  );
}
