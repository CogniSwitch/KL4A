import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from 'react';
import * as api from '../lib/api';
import { subscribe } from '../lib/events';
import type { WorkbenchContext as WorkbenchContextShape } from '../types/commands';

interface WorkbenchContextValue {
  context: WorkbenchContextShape | null;
  loading: boolean;
  /** Error thrown by `get_workbench_context` itself, distinct from `context.error` (a degraded-mode reason). */
  loadError: string | null;
  refresh: () => Promise<void>;
  switchRoot: (path: string) => Promise<void>;
  selectBundle: (key: string) => Promise<void>;
  deselectBundle: () => Promise<void>;
}

const Ctx = createContext<WorkbenchContextValue | null>(null);

/**
 * Mounts once at the app root. Calls `get_workbench_context` (the §4.12
 * replacement for a `/healthz` check) and re-subscribes to
 * `workbench://context-changed` so every mutation that touches the root or
 * selection propagates without manual prop drilling. `context.mode ===
 * "Degraded"` (or a thrown error from the initial fetch) is what routes to
 * the first-run/broken-root screen — see `App.tsx`.
 */
export function WorkbenchProvider({ children }: { children: ReactNode }) {
  const [context, setContext] = useState<WorkbenchContextShape | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const ctx = await api.get_workbench_context();
      setContext(ctx);
      setLoadError(null);
    } catch (err) {
      setLoadError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
    const unsubscribe = subscribe('workbench://context-changed', (ctx) => {
      setContext(ctx);
    });
    return unsubscribe;
  }, [refresh]);

  const switchRoot = useCallback(async (path: string) => {
    const ctx = await api.set_workbench_root(path);
    setContext(ctx);
  }, []);

  const selectBundle = useCallback(async (key: string) => {
    await api.select_bundle(key);
    // `select_bundle` mutates `WorkbenchContext.selected_bundle` server-side
    // and the mock (and eventually real) backend emits
    // `workbench://context-changed`; refresh defensively in case the event
    // is missed by a listener registered after the call started.
    await refresh();
  }, [refresh]);

  const deselectBundle = useCallback(async () => {
    await api.deselect_bundle();
    await refresh();
  }, [refresh]);

  return (
    <Ctx.Provider value={{ context, loading, loadError, refresh, switchRoot, selectBundle, deselectBundle }}>
      {children}
    </Ctx.Provider>
  );
}

export function useWorkbench(): WorkbenchContextValue {
  const value = useContext(Ctx);
  if (!value) throw new Error('useWorkbench must be used within a WorkbenchProvider');
  return value;
}
