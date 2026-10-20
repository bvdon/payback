// Typed wrappers over the core commands (see payback-core/src/api.rs).

import { call, exportFile } from './transport';
import type { Bootstrap, ImportChange, Loan, Payment, PaymentFields } from './types';
import { todayIso } from '../lib/format';

export interface LoanFields {
  name: string;
  lender: string;
  borrower: string;
  amount: string;
  startDate: string;
}

export const api = {
  bootstrap: () => call<Bootstrap>('app.bootstrap'),

  createLoan: (f: LoanFields) => call<Loan>('loans.create', f),
  updateLoan: (id: string, f: LoanFields) => call<Loan>('loans.update', { id, ...f }),
  deleteLoan: (id: string) => call<Loan>('loans.delete', { id }),

  addPayment: (loanId: string, f: PaymentFields) => call<Payment>('payments.add', { loanId, ...f }),
  updatePayment: (loanId: string, id: string, f: PaymentFields) => call<Payment>('payments.update', { loanId, id, ...f }),
  deletePayment: (loanId: string, id: string) => call<Payment>('payments.delete', { loanId, id }),
  /** Put back a deleted payment exactly as it was. */
  restorePayment: (loanId: string, p: Payment) =>
    call<Payment>('payments.add', {
      loanId,
      id: p.id,
      amountCents: p.amountCents,
      date: p.date,
      method: p.method,
      note: p.note,
      createdAt: p.createdAt,
    }),

  importPreview: (text: string) => call<{ loans: ImportChange[] }>('import.preview', { text }),
  importApply: (text: string, loanIds: string[]) =>
    call<{ added: number; updated: number }>('import.apply', { text, loanIds }),

  exportStatement: (loan: Loan, kind: 'pdf' | 'csv') =>
    exportFile(kind === 'pdf' ? 'export.statementPdf' : 'export.statementCsv', { loanId: loan.id, today: todayIso() }, `${fileStem(loan.name)}.${kind}`),
  exportData: (loans: Loan[]) =>
    exportFile(
      'export.data',
      { loanIds: loans.map((l) => l.id), today: todayIso() },
      loans.length === 1 ? `${fileStem(loans[0].name)}.payback` : `payback-loans-${todayIso()}.payback`,
    ),
};

/** Same naming as the core's statement::file_stem. */
function fileStem(name: string): string {
  const slug = name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
  return `${slug || 'loan'}-${todayIso()}`;
}
