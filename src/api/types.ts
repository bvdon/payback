// Mirrors the JSON shapes produced by payback-core (model.rs, transfer.rs).

export type Method = 'check' | 'paypal' | 'venmo' | 'cash' | 'other';

export type InterestKind = 'none' | 'simple';

export interface Payment {
  id: string;
  amountCents: number;
  date: string;
  method: Method;
  note: string;
  createdAt: number;
  updatedAt: number;
}

/** A payment as applied to its loan (see payback-core/src/interest.rs). */
export interface LedgerPayment extends Payment {
  balanceAfterCents: number;
  /** The stretch this payment closes: from the previous payment, or the start. */
  fromDate: string;
  days: number;
  principalBeforeCents: number;
  interestAccruedCents: number;
  toInterestCents: number;
  toPrincipalCents: number;
  /** Interest still owed after this payment, when it didn't cover it all. */
  interestOwedAfterCents: number;
}

export interface Loan {
  id: string;
  name: string;
  /** Empty for loans made before these fields existed. */
  lender: string;
  borrower: string;
  principalCents: number;
  startDate: string;
  interest: InterestKind;
  /** Thousandths of a percent a year: 5.25% is 5250. 0 when interest-free. */
  interestRate: number;
  /** "5.25%". */
  rateLabel: string;
  createdAt: number;
  updatedAt: number;
  /** Newest first. */
  payments: LedgerPayment[];
  paidCents: number;
  /** What's owed today: principal plus interest built up. Negative means overpaid. */
  balanceCents: number;
  principalOwedCents: number;
  interestOwedCents: number;
  /** All interest charged from the start to `asOf`. */
  interestTotalCents: number;
  interestPaidCents: number;
  dailyInterestCents: number;
  /** The open stretch since the last payment (or the start). */
  sinceDate: string;
  daysSince: number;
  accruedSinceCents: number;
  /** Today, or the last payment's date if that's later. */
  asOf: string;
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
