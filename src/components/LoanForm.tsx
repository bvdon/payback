import { useState } from 'react';
import type { FormEvent } from 'react';
import { api } from '../api/api';
import type { Loan } from '../api/types';
import { amountInput, plural, todayIso } from '../lib/format';
import { useStore } from '../state';
import { Modal } from './ui';

/** Create a loan, or edit (and delete) an existing one. */
export function LoanForm({ loan, onClose, onSaved }: { loan?: Loan; onClose: () => void; onSaved: (id: string) => void }) {
  const { act, data } = useStore();
  const [name, setName] = useState(loan?.name ?? '');
  const [lender, setLender] = useState(loan?.lender ?? '');
  const [borrower, setBorrower] = useState(loan?.borrower ?? '');
  // Names used on other loans, offered as suggestions.
  const people = [...new Set(data.loans.flatMap((l) => [l.lender, l.borrower]).filter(Boolean))].sort();
  const [amount, setAmount] = useState(loan ? amountInput(loan.principalCents) : '');
  const [startDate, setStartDate] = useState(loan?.startDate ?? todayIso());
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [busy, setBusy] = useState(false);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    const fields = { name, lender, borrower, amount, startDate };
    const saved = await act(
      () => (loan ? api.updateLoan(loan.id, fields) : api.createLoan(fields)),
      loan ? 'Loan updated' : undefined,
    );
    setBusy(false);
    if (saved) onSaved(saved.id);
  };

  const remove = async () => {
    if (!loan) return;
    const gone = await act(() => api.deleteLoan(loan.id), `Deleted “${loan.name}”`);
    if (gone) onClose();
  };

  return (
    <Modal
      title={loan ? 'Edit loan' : 'New loan'}
      onClose={onClose}
      footer={
        <>
          {loan && !confirmDelete && (
            <button type="button" className="btn btn-danger-quiet" onClick={() => setConfirmDelete(true)}>
              Delete loan…
            </button>
          )}
          <span className="grow" />
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          <button type="submit" form="loan-form" className="btn btn-primary" disabled={busy}>
            {loan ? 'Save' : 'Create loan'}
          </button>
        </>
      }
    >
      <form id="loan-form" className="form" onSubmit={submit}>
        <label className="field">
          <span>Loan name</span>
          <input autoFocus value={name} onChange={(e) => setName(e.target.value)} placeholder="e.g. Car loan to Sam" maxLength={100} />
        </label>
        <div className="field-row">
          <label className="field">
            <span>Lent by</span>
            <input value={lender} onChange={(e) => setLender(e.target.value)} placeholder="Who lent the money" list="people" maxLength={100} />
          </label>
          <label className="field">
            <span>Borrowed by</span>
            <input value={borrower} onChange={(e) => setBorrower(e.target.value)} placeholder="Who borrowed it" list="people" maxLength={100} />
          </label>
        </div>
        <datalist id="people">
          {people.map((p) => (
            <option key={p} value={p} />
          ))}
        </datalist>
        <div className="field-row">
          <label className="field">
            <span>Amount lent</span>
            <div className="money-input">
              <span>$</span>
              <input inputMode="decimal" value={amount} onChange={(e) => setAmount(e.target.value)} placeholder="0.00" />
            </div>
          </label>
          <label className="field">
            <span>Start date</span>
            <input type="date" value={startDate} onChange={(e) => setStartDate(e.target.value)} />
          </label>
        </div>
      </form>
      {loan && confirmDelete && (
        <div className="danger-box">
          <p>
            Delete <strong>{loan.name}</strong> and its {plural(loan.paymentCount, 'payment')} from this computer? A copy stays
            in the daily backups.
          </p>
          <div className="row-end">
            <button className="btn" onClick={() => setConfirmDelete(false)}>
              Keep it
            </button>
            <button className="btn btn-danger" onClick={() => void remove()}>
              Delete loan
            </button>
          </div>
        </div>
      )}
    </Modal>
  );
}
