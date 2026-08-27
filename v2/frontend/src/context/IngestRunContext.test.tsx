import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest';
import { IngestScreen } from '../screens/IngestScreen';
import { SourcesScreen } from '../screens/SourcesScreen';
import { WorkbenchProvider } from './WorkbenchContext';
import { IngestRunProvider } from './IngestRunContext';
import { __resetStoreForTests, store } from '../mock/store';
import * as api from '../lib/api';
import { publish } from '../lib/events';
import type { IngestResult } from '../types/commands';

/**
 * Real cross-screen scenarios `IngestScreen.test.tsx`/`SourcesScreen.test.tsx`
 * can't exercise on their own (each renders its screen under its own isolated
 * provider tree, same as `App.tsx` never actually does): a real ingest run
 * that survives navigating away from `/ingest` and back, and `SourcesScreen`
 * noticing a run that was started from `/ingest` while it's the screen
 * currently on-screen. Mounts a SINGLE `IngestRunProvider` (matching
 * `App.tsx`'s real structure -- one provider at the app root, screens mount
 * and unmount underneath it as routes change) and swaps which screen is
 * rendered underneath it via `rerender`, the same way `<Routes>` swaps
 * `<Route element>`s without ever unmounting the provider itself.
 *
 * Also waits out the mock API's own artificial `LATENCY_MS` (120ms,
 * api.mock.ts) before returning: `WorkbenchProvider`'s initial
 * `get_workbench_context()` fetch shares that same delay, and
 * `IngestRunContext` reads `selected_bundle` from it to decide which bundle a
 * run belongs to. In the real app this race can't happen -- `Shell`
 * (`App.tsx`) blocks all routing on `loading`, so a user can never reach
 * `/ingest` before the context resolves -- but these tests mount
 * `IngestScreen`/`SourcesScreen` directly, bypassing that gate.
 */
async function renderUnder(screen_: 'ingest' | 'sources') {
  const result = render(
    <MemoryRouter>
      <WorkbenchProvider>
        <IngestRunProvider>{screen_ === 'ingest' ? <IngestScreen /> : <SourcesScreen />}</IngestRunProvider>
      </WorkbenchProvider>
    </MemoryRouter>,
  );
  await new Promise((resolve) => setTimeout(resolve, 200));
  return result;
}

describe('IngestRunContext survives navigation', () => {
  beforeEach(() => {
    __resetStoreForTests();
    store.selectedBundle = 'glp1-healthcare';
  });

  afterEach(() => {
    // Only the fake-timers test in this block turns them on, but reset
    // unconditionally so a test that throws before its own cleanup can't
    // leak fake timers into every test that runs after it.
    vi.useRealTimers();
  });

  it('keeps a running ingest visible after unmounting and remounting IngestScreen', async () => {
    const { rerender } = await renderUnder('ingest');
    await userEvent.click(screen.getByRole('checkbox', { name: /confirm these files\/folder should update this bundle/i }));
    await userEvent.click(screen.getByRole('button', { name: 'Run pipeline' }));

    // Simulate navigating away: unmount IngestScreen (a placeholder stands in
    // for whatever route the user went to), leaving IngestRunProvider mounted
    // -- exactly what `<Routes>` does on a route change in the real app.
    rerender(
      <MemoryRouter>
        <WorkbenchProvider>
          <IngestRunProvider>
            <div>elsewhere</div>
          </IngestRunProvider>
        </WorkbenchProvider>
      </MemoryRouter>,
    );
    expect(screen.getByText('elsewhere')).toBeInTheDocument();

    // Navigate back: IngestScreen remounts. Before this fix, its own local
    // `useState` would have reset to defaults here, losing all visibility
    // into the run that (on the backend) never stopped.
    rerender(
      <MemoryRouter>
        <WorkbenchProvider>
          <IngestRunProvider>
            <IngestScreen />
          </IngestRunProvider>
        </WorkbenchProvider>
      </MemoryRouter>,
    );
    expect(await screen.findByText('Progress')).toBeInTheDocument();
    expect(await screen.findByText('Files uploaded', {}, { timeout: 5000 })).toBeInTheDocument();
  });

  it('shows the Sources banner while an ingest started on the Ingest screen is still running, and it links back to /ingest', async () => {
    // A real-timer version of this test was observed to occasionally see the
    // whole run (and even SourcesScreen's own separate mock data-loading
    // delay) already finished by the time it got to assert, under nothing
    // more than normal test-harness overhead -- and fake timers turned out
    // to fight `userEvent`'s own internal scheduling badly enough to hang
    // outright. The reliable fix: don't race real (or simulated) time at
    // all -- hold `api.run_ingest_pipeline` open on a promise THIS test
    // controls directly, so "still running" is a fact, not a timing guess.
    let resolveRun!: (value: never) => void;
    const runSpy = vi.spyOn(api, 'run_ingest_pipeline').mockReturnValue(new Promise((resolve) => {
      resolveRun = resolve;
    }) as ReturnType<typeof api.run_ingest_pipeline>);

    const { rerender } = await renderUnder('ingest');
    await userEvent.click(screen.getByRole('checkbox', { name: /confirm these files\/folder should update this bundle/i }));
    await userEvent.click(screen.getByRole('button', { name: 'Run pipeline' }));
    expect(runSpy).toHaveBeenCalled();

    rerender(
      <MemoryRouter>
        <WorkbenchProvider>
          <IngestRunProvider>
            <SourcesScreen />
          </IngestRunProvider>
        </WorkbenchProvider>
      </MemoryRouter>,
    );

    // SourcesScreen's own (real, unmocked) data loads still need to settle.
    const banner = await screen.findByText('An ingest is currently running — view progress', {}, { timeout: 3000 });
    expect(banner.closest('a')).toHaveAttribute('href', '/ingest');

    resolveRun({} as never);
    runSpy.mockRestore();
  });

  it('does not show the Sources banner once the run has finished', async () => {
    await renderUnder('sources');
    expect(screen.queryByText('An ingest is currently running — view progress')).not.toBeInTheDocument();
  });

  it('does not leak a just-finished run\'s result across a bundle switch', async () => {
    // A real user report: switching bundles and returning to /ingest showed the
    // PREVIOUS bundle's completed run as if it just happened for the newly
    // selected one. `select_bundle` (unlike `set_workbench_root`) is never
    // refused while a mutation is in flight, so this can happen even without
    // navigating away first.
    let resolveRun!: (value: IngestResult) => void;
    const runSpy = vi.spyOn(api, 'run_ingest_pipeline').mockReturnValue(
      new Promise((resolve) => {
        resolveRun = resolve;
      }) as ReturnType<typeof api.run_ingest_pipeline>,
    );

    await renderUnder('ingest');
    await userEvent.click(screen.getByRole('checkbox', { name: /confirm these files\/folder should update this bundle/i }));
    await userEvent.click(screen.getByRole('button', { name: 'Run pipeline' }));
    expect(runSpy).toHaveBeenCalled();

    resolveRun({ uploaded_files: 4, sources: 4, sections: 12 } as IngestResult);
    await screen.findByText('Files uploaded');

    // Switch the selected bundle WITHOUT navigating away from /ingest, exactly
    // the way `WorkbenchProvider` learns of a real `select_bundle` call.
    const ctx = await api.get_workbench_context();
    publish('workbench://context-changed', { ...ctx, selected_bundle: 'escalation-runbook' });

    await waitFor(() => expect(screen.queryByText('Files uploaded')).not.toBeInTheDocument());

    runSpy.mockRestore();
  });
});

describe('IngestRunContext cancellation', () => {
  beforeEach(() => {
    __resetStoreForTests();
    store.selectedBundle = 'glp1-healthcare';
  });

  it('cancelling mid-run stops a not-yet-started step and the UI reflects it', async () => {
    await renderUnder('ingest');
    await userEvent.click(screen.getByRole('checkbox', { name: /confirm these files\/folder should update this bundle/i }));
    const runPromise = userEvent.click(screen.getByRole('button', { name: 'Run pipeline' }));

    const cancelButton = await screen.findByRole('button', { name: 'Cancel' });
    await userEvent.click(cancelButton);

    // "Cancelling…" while the request is in flight, then the run settles.
    expect(await screen.findByText('Cancelling…')).toBeInTheDocument();
    await runPromise;

    // At least one step never got to run -- the mock's own cooperative check
    // (mirroring the real backend) stops before the NEXT not-yet-started
    // step, same semantics as the real Rust side.
    expect(await screen.findByText('Cancelled', {}, { timeout: 3000 })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /cancel/i })).not.toBeInTheDocument();
  });
});
