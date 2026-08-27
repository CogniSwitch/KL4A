import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SourcesScreen } from './SourcesScreen';
import { WorkbenchProvider } from '../context/WorkbenchContext';
import { IngestRunProvider } from '../context/IngestRunContext';
import { __resetStoreForTests, store } from '../mock/store';
import * as api from '../lib/api';

function renderSources() {
  return render(
    <MemoryRouter>
      <WorkbenchProvider>
        <IngestRunProvider>
          <SourcesScreen />
        </IngestRunProvider>
      </WorkbenchProvider>
    </MemoryRouter>,
  );
}

/** The "Sections" column is the 6th `<td>` — Title, Type, Parse status, Size, Warnings, Sections, (View link). */
function sectionsCellFor(title: string): HTMLElement {
  const row = screen.getByText(title).closest('tr')!;
  return within(row).getAllByRole('cell')[5];
}

/**
 * Per-source section count column — see CATCHUP_PLAN.md's 2026-08-22
 * sections-view research. Before this, nothing in the UI showed how many
 * sections a source produced; the `glp1-healthcare` fixture bundle has 4
 * sources with 3/4/3/3 sections respectively (13 total, matching
 * `KnowledgeScreen.test.tsx`'s section-coverage count).
 */
describe('SourcesScreen sections column', () => {
  beforeEach(() => {
    __resetStoreForTests();
    store.selectedBundle = 'glp1-healthcare';
  });

  it('shows the per-source section count grouped client-side from list_sections', async () => {
    renderSources();
    await screen.findByText('follow up monitoring procedure');

    expect(sectionsCellFor('follow up monitoring procedure').textContent).toBe('3');
    expect(sectionsCellFor('primary care glp1 sop').textContent).toBe('4');
    expect(sectionsCellFor('prior authorization workflow').textContent).toBe('3');
    expect(sectionsCellFor('safety policy').textContent).toBe('3');
  });

  /**
   * The single-giant-section pathology (CATCHUP_PLAN.md's headline finding):
   * a source with exactly one section did not carve on any real Markdown
   * heading, because `extract_sections` always spans a lone section from
   * the start of the file to EOF. Neither fixture bundle naturally has a
   * single-section source, so this test trims the escalation bundle's
   * sections down to one directly in the mock store (test-only mutation,
   * undone by `__resetStoreForTests` before the next test) rather than
   * reshaping a golden-corpus-derived fixture module just for this case.
   */
  it('flags a source with exactly one section', async () => {
    store.selectedBundle = 'escalation-runbook';
    const bundle = store.requireBundle();
    bundle.sections = [bundle.sections[0]];

    renderSources();
    await screen.findByText('escalation runbook');

    const cell = sectionsCellFor('escalation runbook');
    expect(cell.textContent).toBe('1');
    const badge = within(cell).getByText('1');
    expect(badge).toHaveClass('text-warn');
    expect(badge).toHaveAttribute('title', expect.stringContaining('spanning the entire normalized text'));
  });
});

describe('SourcesScreen last-run summary', () => {
  beforeEach(() => {
    __resetStoreForTests();
    store.selectedBundle = 'glp1-healthcare';
  });

  it('shows a summary of the persisted last completed run, with a link to /ingest', async () => {
    store.lastIngestRun = {
      ok: true,
      finished_at: '2026-08-24T08:33:56Z',
      status: { scan: 'done', normalize: 'done', mine: 'done', validate: 'done' },
      detail: { scan: '4 sources', normalize: '83 sections', mine: '16 items' },
    };

    renderSources();

    await screen.findByText(/Last ingest: 4 sources · 83 sections · 16 items/);
    expect(screen.getByRole('link', { name: 'View details' })).toHaveAttribute('href', '/ingest');
  });

  it('names the failed step when the persisted last run did not succeed', async () => {
    store.lastIngestRun = {
      ok: false,
      finished_at: '2026-08-24T08:33:56Z',
      status: { scan: 'done', normalize: 'error', mine: 'pending', validate: 'pending' },
      detail: { scan: '4 sources', normalize: 'LLM request timed out' },
    };

    renderSources();

    expect(await screen.findByText('Last ingest failed at step: Normalize')).toBeInTheDocument();
  });

  it('shows no summary at all when no run has ever completed', async () => {
    renderSources();
    await screen.findByText('follow up monitoring procedure');
    expect(screen.queryByText(/Last ingest/)).not.toBeInTheDocument();
  });
});

// "Reveal bundle folder" / "Force resync" moved here from the now-removed
// Export screen (round 6) -- see SourcesScreen.tsx's own doc comment.
describe('SourcesScreen bundle-folder utility actions', () => {
  beforeEach(() => {
    __resetStoreForTests();
    store.selectedBundle = 'glp1-healthcare';
  });

  it('reveals the bundle export folder via get_export_dir + reveal_path', async () => {
    const revealSpy = vi.spyOn(api, 'reveal_path');
    renderSources();
    await screen.findByText('follow up monitoring procedure');

    await userEvent.click(screen.getByRole('button', { name: 'Reveal bundle folder' }));

    await waitFor(() => expect(revealSpy).toHaveBeenCalledWith(store.requireBundle().export_path));
  });

  it('force-resyncs OKF documents and shows a success message', async () => {
    renderSources();
    await screen.findByText('follow up monitoring procedure');

    await userEvent.click(screen.getByRole('button', { name: 'Force resync' }));

    expect(await screen.findByText('OKF documents resynced.')).toBeInTheDocument();
  });
});

// "Delete a source" is really "retire" it -- non-destructive, see
// `retire_source`'s own doc comment (backend and api.ts) for why.
describe('SourcesScreen retire (delete) action', () => {
  beforeEach(() => {
    __resetStoreForTests();
    store.selectedBundle = 'glp1-healthcare';
  });

  function rowFor(title: string): HTMLElement {
    return screen.getByText(title).closest('tr')!;
  }

  it('retires a source after one explicit confirmation and reflects it in the row', async () => {
    renderSources();
    await screen.findByText('safety policy');

    await userEvent.click(within(rowFor('safety policy')).getByRole('button', { name: 'Retire' }));
    expect(await screen.findByText('Retire "safety policy"?')).toBeInTheDocument();
    await userEvent.click(within(screen.getByRole('alertdialog')).getByRole('button', { name: 'Retire' }));

    await screen.findByText('Retired');
    expect(within(rowFor('safety policy')).queryByRole('button', { name: 'Retire' })).not.toBeInTheDocument();
    expect(store.requireBundle().sources.find((s) => s.title === 'safety policy')?.status).toBe('retired');
  });

  it('cancelling the confirm dialog leaves the source untouched', async () => {
    renderSources();
    await screen.findByText('safety policy');

    await userEvent.click(within(rowFor('safety policy')).getByRole('button', { name: 'Retire' }));
    await screen.findByText('Retire "safety policy"?');
    await userEvent.click(screen.getByRole('button', { name: 'Cancel' }));

    expect(screen.queryByText('Retire "safety policy"?')).not.toBeInTheDocument();
    expect(within(rowFor('safety policy')).getByRole('button', { name: 'Retire' })).toBeInTheDocument();
    expect(store.requireBundle().sources.find((s) => s.title === 'safety policy')?.status).not.toBe('retired');
  });
});
