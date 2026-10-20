// How the UI reaches the Rust core. In the desktop app that's Tauri IPC; in a
// plain browser (development) it's HTTP to the dev server. Both run the same
// core commands.

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export const isDesktop = typeof window !== 'undefined' && window.__TAURI_INTERNALS__ !== undefined;

export class ApiError extends Error {
  kind: string;
  constructor(kind: string, message: string) {
    super(message);
    this.kind = kind;
  }
}

function toError(raw: unknown): ApiError {
  if (raw instanceof ApiError) return raw;
  const text = typeof raw === 'string' ? raw : raw instanceof Error ? raw.message : String(raw);
  try {
    const j = JSON.parse(text) as { kind?: string; message?: string };
    if (j && typeof j.message === 'string') return new ApiError(j.kind ?? 'error', j.message);
  } catch {
    // not JSON
  }
  return new ApiError('error', text || 'Something went wrong');
}

async function invoke<T>(cmd: string, args: Record<string, unknown>): Promise<T> {
  const core = await import('@tauri-apps/api/core');
  try {
    return await core.invoke<T>(cmd, args);
  } catch (e) {
    throw toError(e);
  }
}

/** Run a core command. */
export async function call<T>(cmd: string, args: object = {}): Promise<T> {
  const body = JSON.stringify(args);
  if (isDesktop) {
    const text = await invoke<string>('call', { cmd, args: body });
    return JSON.parse(text) as T;
  }
  let res: Response;
  try {
    res = await fetch(`/api/call/${cmd}`, { method: 'POST', body, headers: { 'Content-Type': 'application/json' } });
  } catch {
    throw new ApiError('io', 'Lost contact with the Payback dev server. Is it still running?');
  }
  const text = await res.text();
  if (!res.ok) throw toError(text);
  return JSON.parse(text) as T;
}

const FILE_TYPES: Record<string, string> = { pdf: 'PDF', csv: 'CSV', payback: 'Payback file' };

/** Run an export command and save the file. Returns where it went, or null if cancelled. */
export async function exportFile(cmd: string, args: object, suggestedName: string): Promise<string | null> {
  const ext = suggestedName.split('.').pop() ?? '';
  if (isDesktop) {
    const { save } = await import('@tauri-apps/plugin-dialog');
    const path = await save({ defaultPath: suggestedName, filters: [{ name: FILE_TYPES[ext] ?? ext, extensions: [ext] }] });
    if (!path) return null;
    return invoke<string>('export_file', { cmd, args: JSON.stringify(args), path });
  }
  const res = await fetch(`/api/export/${cmd}`, { method: 'POST', body: JSON.stringify(args) });
  if (!res.ok) throw toError(await res.text());
  const blob = await res.blob();
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = suggestedName;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 5000);
  return suggestedName;
}

/** Ask the person for a `.payback` file to import. */
export async function pickPaybackFile(): Promise<{ name: string; text: string } | null> {
  if (isDesktop) {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const picked = await open({
      multiple: false,
      directory: false,
      filters: [{ name: 'Payback file', extensions: ['payback', 'json'] }],
    });
    const path = Array.isArray(picked) ? picked[0] : picked;
    if (!path) return null;
    const text = await invoke<string>('read_text_file', { path });
    return { name: path.split(/[\\/]/).pop() ?? path, text };
  }
  return new Promise((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.payback,.json';
    input.onchange = () => {
      const file = input.files?.[0];
      if (!file) return resolve(null);
      file.text().then((text) => resolve({ name: file.name, text }));
    };
    input.click();
  });
}

export interface AppPaths {
  dataDir: string;
  dataFile: string | null;
  backupsDir: string;
}

export async function appPaths(): Promise<AppPaths | null> {
  if (!isDesktop) return null;
  return JSON.parse(await invoke<string>('app_paths', {})) as AppPaths;
}

export async function backupNow(): Promise<string> {
  if (isDesktop) return invoke<string>('backup_now', {});
  const res = await fetch('/api/backup', { method: 'POST' });
  const text = await res.text();
  if (!res.ok) throw toError(text);
  return JSON.parse(text) as string;
}

/** Pick a backup file and restore it. Returns the safety copy's path, or null if cancelled. */
export async function restoreBackup(defaultDir?: string): Promise<string | null> {
  if (!isDesktop) throw new ApiError('invalid', 'Restoring backups is only available in the desktop app.');
  const { open } = await import('@tauri-apps/plugin-dialog');
  const picked = await open({
    multiple: false,
    directory: false,
    defaultPath: defaultDir,
    filters: [{ name: 'Payback backup', extensions: ['json', 'payback'] }],
  });
  const path = Array.isArray(picked) ? picked[0] : picked;
  if (!path) return null;
  return invoke<string>('restore_backup', { path });
}

export async function reveal(path: string): Promise<void> {
  if (isDesktop) await invoke<void>('reveal', { path });
}

/** "Finder" on a Mac, "File Explorer" on Windows. */
export const fileManagerName = /Windows/i.test(navigator.userAgent) ? 'File Explorer' : 'Finder';
