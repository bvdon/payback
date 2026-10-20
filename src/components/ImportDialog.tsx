import { useState } from 'react';
import { api } from '../api/api';
import type { ImportChange } from '../api/types';
import { money, plural } from '../lib/format';
import { useStore } from '../state';
import { Modal } from './ui';

/** Preview what a `.payback` file would change, then import the chosen loans. */
export function ImportDialog({
  file,
  changes,
  onClose,
  onImported,
}: {
  file: { name: string; text: string };
  changes: ImportChange[];
  onClose: () => void;
  onImported: (firstId: string) => void;
}) {
  const { act, toast } = useStore();
  const [picked, setPicked] = useState<Set<string>>(() => new Set(changes.filter((c) => c.status !== 'same').map((c) => c.id)));
  const toggle = (id: string) =>
    setPicked((s) => {
      const n = new Set(s);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });

  const run = async () => {
    const ids = [...picked];
    const r = await act(() => api.importApply(file.text, ids));
    if (!r) return;
    const parts = [r.added && `added ${plural(r.added, 'loan')}`, r.updated && `updated ${plural(r.updated, 'loan')}`].filter(Boolean);
    toast(parts.length ? `Import done: ${parts.join(', ')}` : 'Nothing needed to change');
    onImported(ids[0]);
  };

  const anyChanges = changes.some((c) => c.status !== 'same');

  return (
    <Modal
      title="Import shared file"
      onClose={onClose}
      wide
      footer={
        <>
          <span className="grow" />
          <button className="btn" onClick={onClose}>
            {anyChanges ? 'Cancel' : 'Close'}
          </button>
          {anyChanges && (
            <button className="btn btn-primary" disabled={picked.size === 0} onClick={() => void run()}>
              Import {picked.size === 1 ? '1 loan' : `${picked.size} loans`}
            </button>
          )}
        </>
      }
    >
      <p className="muted">
        From <strong>{file.name}</strong>. A loan already on this computer is replaced by the version in the file.
      </p>
      <ul className="import-list">
        {changes.map((c) => (
          <li key={c.id} className={c.status === 'same' ? 'is-same' : ''}>
            <label>
              <input type="checkbox" disabled={c.status === 'same'} checked={picked.has(c.id)} onChange={() => toggle(c.id)} />
              <div className="import-body">
                <div className="import-title">
                  <strong>{c.name}</strong>
                  <span className={`badge badge-${c.status}`}>
                    {c.status === 'new' ? 'New' : c.status === 'update' ? 'Changed' : 'Already up to date'}
                  </span>
                </div>
                <Describe c={c} />
              </div>
            </label>
          </li>
        ))}
      </ul>
    </Modal>
  );
}

function Describe({ c }: { c: ImportChange }) {
  if (c.status === 'new') {
    return (
      <p className="muted">
        {plural(c.paymentsAdded, 'payment')} · balance {money(c.balanceAfterCents)}
      </p>
    );
  }
  if (c.status === 'same') return <p className="muted">Matches what’s on this computer.</p>;
  const bits = [
    c.paymentsAdded && `${plural(c.paymentsAdded, 'new payment')}`,
    c.paymentsEdited && `${plural(c.paymentsEdited, 'edited payment')}`,
    c.detailsChanged && (c.previousName ? `renamed from “${c.previousName}”` : 'loan details changed'),
  ].filter(Boolean);
  return (
    <>
      <p className="muted">
        {bits.length > 0 && `${bits.join(' · ')} · `}
        balance {money(c.balanceBeforeCents ?? 0)} → <strong>{money(c.balanceAfterCents)}</strong>
      </p>
      {c.paymentsRemoved > 0 && (
        <p className="warn">
          {plural(c.paymentsRemoved, 'payment')} recorded on this computer {c.paymentsRemoved === 1 ? 'isn’t' : 'aren’t'} in the
          file and will be removed. If they’re real, add them again after importing or ask for a newer file.
        </p>
      )}
    </>
  );
}
