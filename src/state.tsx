import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { api } from './api/api';
import { ApiError } from './api/transport';
import type { Bootstrap } from './api/types';

export interface Toast {
  id: number;
  message: string;
  kind: 'info' | 'error';
  action?: { label: string; run: () => void };
}

interface Store {
  data: Bootstrap;
  reload: () => Promise<void>;
  /** Run an action: errors become toasts, success refetches all data. */
  act: <T>(fn: () => Promise<T>, success?: string) => Promise<T | undefined>;
  toast: (message: string, kind?: Toast['kind'], action?: Toast['action']) => void;
  dismiss: (id: number) => void;
  toasts: Toast[];
}

const Ctx = createContext<Store | null>(null);

export function useStore(): Store {
  const s = useContext(Ctx);
  if (!s) throw new Error('useStore outside StoreProvider');
  return s;
}

export function errorMessage(e: unknown): string {
  if (e instanceof ApiError || e instanceof Error) return e.message;
  return String(e);
}

export function StoreProvider({ initial, children }: { initial: Bootstrap; children: ReactNode }) {
  const [data, setData] = useState<Bootstrap>(initial);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const nextToast = useRef(1);

  const dismiss = useCallback((id: number) => setToasts((t) => t.filter((x) => x.id !== id)), []);

  const toast = useCallback(
    (message: string, kind: Toast['kind'] = 'info', action?: Toast['action']) => {
      const id = nextToast.current++;
      setToasts((t) => [...t.slice(-2), { id, message, kind, action }]);
      setTimeout(() => dismiss(id), kind === 'error' ? 8000 : action ? 8000 : 3500);
    },
    [dismiss],
  );

  const reload = useCallback(async () => {
    try {
      setData(await api.bootstrap());
    } catch (e) {
      toast(errorMessage(e), 'error');
    }
  }, [toast]);

  const act = useCallback(
    async <T,>(fn: () => Promise<T>, success?: string): Promise<T | undefined> => {
      try {
        const r = await fn();
        if (success) toast(success);
        setData(await api.bootstrap());
        return r;
      } catch (e) {
        toast(errorMessage(e), 'error');
        return undefined;
      }
    },
    [toast],
  );

  // Another window or a restore may have changed things.
  useEffect(() => {
    const onFocus = () => void reload();
    window.addEventListener('focus', onFocus);
    return () => window.removeEventListener('focus', onFocus);
  }, [reload]);

  const value = useMemo<Store>(
    () => ({ data, reload, act, toast, dismiss, toasts }),
    [data, reload, act, toast, dismiss, toasts],
  );

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}
