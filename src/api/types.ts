// Mirrors the JSON shapes produced by payback-core (model.rs, transfer.rs).

export type Method = 'check' | 'paypal' | 'venmo' | 'cash' | 'other';

export interface Payment {
  id: string;
  amountCents: number;
  date: string;
  method: Method;
  note: string;
  createdAt: number;
  updatedAt: number;
}

export interface LedgerPayment extends Payment {
  balanceAfterCents: number;
}

export interface Loan {
  id: string;
  name: string;
  /** Empty for loans made before these fields existed. */
  lender: string;
  borrower: string;
  principalCents: number;
  startDate: string;
  createdAt: number;
  updatedAt: number;
  /** Newest first. */
  payments: LedgerPayment[];
  paidCents: number;
  balanceCents: number;
  paymentCount: number;
  lastPaymentDate: string | null;
}

export interface Bootstrap {
  version: string;
  loans: Loan[];
}

export interface ImportChange {
  id: string;
  name: string;
  status: 'new' | 'update' | 'same';
  previousName: string | null;
  detailsChanged: boolean;
  paymentsAdded: number;
  paymentsRemoved: number;
  paymentsEdited: number;
  balanceBeforeCents: number | null;
  balanceAfterCents: number;
}

export interface PaymentFields {
  amount: string;
  date: string;
  method: Method;
  note: string;
}
