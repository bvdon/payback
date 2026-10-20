import { useEffect, useState } from 'react';
import { api } from '../api/api';
import { appPaths, backupNow, fileManagerName, isDesktop, restoreBackup, reveal } from '../api/transport';
import type { AppPaths } from '../api/transport';
import { errorMessage, useStore } from '../state';
import { Icon, Modal } from './ui';

export function SettingsDialog({ onClose }: { onClose: () => void }) {
  const { data, toast, reload } = useStore();
  const [paths, setPaths] = useState<AppPaths | null>(null);
  useEffect(() => {
    appPaths().then(setPaths, () => setPaths(null));
  }, []);

  const attempt = async (fn: () => Promise<void>) => {
    try {
      await fn();
    } catch (e) {
      toast(errorMessage(e), 'error');
    }
  };

  return (
    <Modal title="Settings" onClose={onClose}>
      <div className="settings">
        <section>
          <h3>Share all loans</h3>
          <p className="muted">Save every loan into one .payback file for the other person to import.</p>
          <button
            className="btn"
            disabled={data.loans.length === 0}
            onClick={() =>
              void attempt(async () => {
                if (await api.exportData(data.loans)) toast('Saved the share file');
              })
            }
          >
            <Icon name="share" />
            Export all loans…
          </button>
        </section>

        <section>
          <h3>Backups</h3>
          <p className="muted">Payback saves a copy of your data once a day and keeps the last 14.</p>
          <div className="row">
            <button className="btn" onClick={() => void attempt(async () => toast(`Backup saved: ${(await backupNow()).split(/[\\/]/).pop()}`))}>
              Back up now
            </button>
            {isDesktop && (
              <button
                className="btn"
                onClick={() =>
                  void attempt(async () => {
                    const safety = await restoreBackup(paths?.backupsDir);
                    if (!safety) return;
                    await reload();
                    toast('Backup restored. Your previous data was saved to the backups folder first.');
                  })
                }
              >
                Restore from backup…
              </button>
            )}
          </div>
        </section>

        {paths && (
          <section>
            <h3>Where your data lives</h3>
            <code className="path">{paths.dataFile ?? paths.dataDir}</code>
            <div className="row">
              <button className="btn" onClick={() => void attempt(() => reveal(paths.dataFile ?? paths.dataDir))}>
                <Icon name="folder" />
                Show in {fileManagerName}
              </button>
            </div>
          </section>
        )}

        <p className="muted small">Payback {data.version}</p>
      </div>
    </Modal>
  );
}
