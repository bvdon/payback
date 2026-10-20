import { useEffect, useState } from 'react';
import { api } from './api/api';
import { pickPaybackFile } from './api/transport';
import type { ImportChange } from './api/types';
import { ImportDialog } from './components/ImportDialog';
import { LoanForm } from './components/LoanForm';
import { LoanPage } from './components/LoanPage';
import { SettingsDialog } from './components/SettingsDialog';
import { Icon, Toasts } from './components/ui';
import { money } from './lib/format';
import { errorMessage, useStore } from './state';

const SELECTED_KEY = 'payback.selectedLoan';

function remembered(): string | null {
  try {
    return localStorage.getItem(SELECTED_KEY);
  } catch {
    return null;
  }
}

type Dialog =
  | { kind: 'new' }
  | { kind: 'edit' }
  | { kind: 'settings' }
  | { kind: 'import'; file: { name: string; text: string }; changes: ImportChange[] }
  | null;

export function App() {
  const { data, toast } = useStore();
  const [selected, setSelected] = useState<string | null>(remembered);
  const [dialog, setDialog] = useState<Dialog>(null);

  const loan = data.loans.find((l) => l.id === selected) ?? data.loans[0] ?? null;

  useEffect(() => {
    try {
      if (loan) localStorage.setItem(SELECTED_KEY, loan.id);
    } catch {
      // storage unavailable; not important
    }
  }, [loan]);

  const startImport = async () => {
    try {
      const file = await pickPaybackFile();
      if (!file) return;
      const { loans } = await api.importPreview(file.text);
      setDialog({ kind: 'import', file, changes: loans });
    } catch (e) {
      toast(errorMessage(e), 'error');
    }
  };

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark">$</span>
          Payback
        </div>
        <div className="side-label">Loans</div>
        <nav className="loan-list">
          {data.loans.map((l) => (
            <button key={l.id} className={`loan-item ${loan?.id === l.id ? 'is-active' : ''}`} onClick={() => setSelected(l.id)}>
              <span className="loan-name">{l.name}</span>
              {l.borrower && <span className="loan-who">{l.lender} → {l.borrower}</span>}
              <span className={`loan-bal ${l.balanceCents <= 0 ? 'is-done' : ''}`}>
                {l.balanceCents === 0 ? 'Paid off' : l.balanceCents < 0 ? `${money(-l.balanceCents)} over` : `${money(l.balanceCents)} left`}
              </span>
            </button>
          ))}
          <button className="side-btn" onClick={() => setDialog({ kind: 'new' })}>
            <Icon name="plus" />
            New loan
          </button>
        </nav>
        <div className="side-foot">
          <button className="side-btn" onClick={() => void startImport()}>
            <Icon name="inbox" />
            Import shared file…
          </button>
          <button className="side-btn" onClick={() => setDialog({ kind: 'settings' })}>
            <Icon name="gear" />
            Settings
          </button>
        </div>
      </aside>

      <main className="main">
        {loan ? (
          <LoanPage key={loan.id} loan={loan} onEdit={() => setDialog({ kind: 'edit' })} />
        ) : (
          <div className="welcome">
            <h1>Track a loan</h1>
            <p className="muted">
              Enter what was lent and when, then record each payment as it comes in. Payback keeps the balance up to date.
            </p>
            <div className="row">
              <button className="btn btn-primary" onClick={() => setDialog({ kind: 'new' })}>
                <Icon name="plus" />
                New loan
              </button>
              <button className="btn" onClick={() => void startImport()}>
                <Icon name="inbox" />
                Import a shared file…
              </button>
            </div>
          </div>
        )}
      </main>

      {dialog?.kind === 'new' && (
        <LoanForm
          onClose={() => setDialog(null)}
          onSaved={(id) => {
            setSelected(id);
            setDialog(null);
          }}
        />
      )}
      {dialog?.kind === 'edit' && loan && <LoanForm loan={loan} onClose={() => setDialog(null)} onSaved={() => setDialog(null)} />}
      {dialog?.kind === 'settings' && <SettingsDialog onClose={() => setDialog(null)} />}
      {dialog?.kind === 'import' && (
        <ImportDialog
          file={dialog.file}
          changes={dialog.changes}
          onClose={() => setDialog(null)}
          onImported={(id) => {
            if (id) setSelected(id);
            setDialog(null);
          }}
        />
      )}
      <Toasts />
    </div>
  );
}
