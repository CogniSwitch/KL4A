import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from 'react';
import * as api from '../lib/api';
import { subscribe } from '../lib/events';
import { useWorkbench } from './WorkbenchContext';
import type { IngestProgressPayload, IngestRequest, IngestResult } from '../types/commands';

interface IngestRunContextValue {
  running: boolean;
  /** True from the moment `cancel()` is called until the run actually stops. */
  cancelling: boolean;
  progressLog: IngestProgressPayload[];
  result: IngestResult | null;
  error: string | null;
  runPipeline: (request: IngestRequest) => Promise<void>;
  cancel: () => Promise<void>;
  /** Clears a finished run's log/result/error without starting a new one. */
  reset: () => void;
}

const Ctx = createContext<IngestRunContextValue | null>(null);

/**
 * Mounted once at the app root (alongside `WorkbenchProvider`, see `App.tsx`), so
 * `running`/`progressLog`/`result` survive navigating away from `/ingest` and back
 * -- previously this state lived in `IngestScreen`'s own `useState`, which React
 * discards on unmount, making a real in-progress run indistinguishable from "no
 * run happened" the moment the user left the screen even though the backend
 * command (`run_ingest_pipeline`, `spawn_blocking`'d on the Rust side) kept running
 * regardless of what the frontend was doing. Any mounted screen can read `running`
 * from here to show its own "an ingest is in progress" signal -- see
 * `SourcesScreen.tsx`'s banner.
 *
 * Bundle-scoped: `select_bundle` (unlike `set_workbench_root`) is NOT refused while
 * a mutation is in flight, so a run started on bundle A is still genuinely running
 * when the user switches to bundle B. The raw run state below is kept regardless of
 * which bundle is selected (so switching back to A still shows it), but every value
 * this context actually EXPOSES is gated to `runBundleKey === <currently selected
 * bundle>` -- otherwise a run just-completed on bundle A would render as "just
 * finished, N items" the moment the user switched to bundle B's own, unrelated
 * Ingest screen.
 */
export function IngestRunProvider({ children }: { children: ReactNode }) {
  const { context } = useWorkbench();
  const selectedBundle = context?.selected_bundle;

  const [running, setRunning] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [progressLog, setProgressLog] = useState<IngestProgressPayload[]>([]);
  const [result, setResult] = useState<IngestResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [runBundleKey, setRunBundleKey] = useState<string | undefined>(undefined);

  useEffect(() => subscribe('ingest://progress', (p) => setProgressLog((log) => [...log, p])), []);

  const runPipeline = useCallback(
    async (request: IngestRequest) => {
      setRunBundleKey(selectedBundle);
      setRunning(true);
      setCancelling(false);
      setError(null);
      setProgressLog([]);
      setResult(null);
      try {
        const outcome = await api.run_ingest_pipeline(request);
        setResult(outcome);
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setRunning(false);
        setCancelling(false);
      }
    },
    [selectedBundle],
  );

  const cancel = useCallback(async () => {
    // See `api.cancel_ingest`'s doc comment: this only stops the pipeline from
    // starting its NEXT step/section/chunk -- `cancelling` reflects "a stop was
    // requested and we're waiting for the run to actually settle", not "stopped".
    setCancelling(true);
    await api.cancel_ingest();
  }, []);

  const reset = useCallback(() => {
    setProgressLog([]);
    setResult(null);
    setError(null);
  }, []);

  // `runBundleKey === undefined` means either single-bundle-workbench mode (no
  // concept of a selected bundle key at all) or a run that started before
  // `WorkbenchContext`'s own initial fetch resolved -- in both cases there's no
  // POSITIVE evidence this run belongs to some OTHER bundle, so default to
  // showing it rather than hiding it. Only a run whose captured key is a real,
  // different string from the currently selected bundle is treated as "not this
  // bundle's run".
  const isCurrentBundle = runBundleKey === undefined || runBundleKey === selectedBundle;

  return (
    <Ctx.Provider
      value={{
        running: isCurrentBundle && running,
        cancelling: isCurrentBundle && cancelling,
        progressLog: isCurrentBundle ? progressLog : [],
        result: isCurrentBundle ? result : null,
        error: isCurrentBundle ? error : null,
        runPipeline,
        cancel,
        reset,
      }}
    >
      {children}
    </Ctx.Provider>
  );
}

export function useIngestRun(): IngestRunContextValue {
  const value = useContext(Ctx);
  if (!value) throw new Error('useIngestRun must be used within an IngestRunProvider');
  return value;
}
