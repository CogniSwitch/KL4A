import { useCallback, useEffect, useRef, useState } from 'react';
import { subscribe } from './events';
import type { BundleStateScope } from '../types/commands';
import { useWorkbench } from '../context/WorkbenchContext';

export interface AsyncState<T> {
  data: T | null;
  loading: boolean;
  error: string | null;
  reload: () => void;
}

/**
 * Fetch-on-mount (and on `deps` change) with loading/error state. Screens
 * use this instead of raw `useEffect` + `useState` boilerplate so every
 * read screen handles the loading/error cases the same way.
 */
export function useAsync<T>(fn: () => Promise<T>, deps: unknown[]): AsyncState<T> {
  const [data, setData] = useState<T | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);
  const fnRef = useRef(fn);
  fnRef.current = fn;

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    fnRef
      .current()
      .then((result) => {
        if (!cancelled) setData(result);
      })
      .catch((err) => {
        if (!cancelled) setError(err instanceof Error ? err.message : String(err));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [...deps, tick]);

  const reload = useCallback(() => setTick((t) => t + 1), []);

  return { data, loading, error, reload };
}

/**
 * §4.11: mutations are followed by a `bundle://state-changed { key, scope }`
 * event, and that event — not a locally-patched copy of the mutation's
 * return value — is the invalidation signal every screen should react to.
 * This hook re-runs `onInvalidate` whenever the event fires for the
 * currently-selected bundle and its `scope` intersects `watchScopes` (pass
 * no scopes to react to every mutation on the bundle regardless of scope).
 */
export function useBundleInvalidation(onInvalidate: () => void, watchScopes?: BundleStateScope[]): void {
  const { context } = useWorkbench();
  const selectedBundle = context?.selected_bundle;
  const onInvalidateRef = useRef(onInvalidate);
  onInvalidateRef.current = onInvalidate;

  useEffect(() => {
    return subscribe('bundle://state-changed', (payload) => {
      if (selectedBundle && payload.key !== selectedBundle) return;
      if (watchScopes && watchScopes.length > 0 && !payload.scope.some((s) => watchScopes.includes(s))) {
        return;
      }
      onInvalidateRef.current();
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedBundle, watchScopes?.join(',')]);
}
