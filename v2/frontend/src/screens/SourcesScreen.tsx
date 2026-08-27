import { useState } from 'react';
import { Link } from 'react-router-dom';
import * as api from '../lib/api';
import { useAsync, useBundleInvalidation } from '../lib/hooks';
import { useIngestRun } from '../context/IngestRunContext';
import { LoadingBlock, ErrorBanner, EmptyState } from '../components/common/Feedback';
import { EntityBadge } from '../components/common/EntityBadge';
import { ConfirmDialog } from '../components/common/ConfirmDialog';
import { Button } from '../components/common/Button';
import type { IngestResult, IngestRunStatus, Source } from '../types/commands';

export function SourcesScreen() {
  // `IngestRunContext` survives navigation (see its own doc comment), so a
  // run started on `/ingest` and left running is still visible here -- a real
  // user gap this closes: no way to tell an ingest was still going once you'd
  // navigated away from the Ingest screen to check something else. Both
  // `running` and `result` are scoped to the CURRENTLY selected bundle (see
  // IngestRunContext's own doc comment) -- switching bundles never leaks a
  // different bundle's run status onto this screen.
  const { running: ingestRunning, result: sessionResult } = useIngestRun();
  const sources = useAsync(() => api.list_sources(), []);
  const stats = useAsync(() => api.get_source_stats(), []);
  const sections = useAsync(() => api.list_sections(), []);
  // Persisted, per-bundle record of the last completed run (`.sopkb/ingest_run.json`)
  // -- the fallback for "what happened last time" across an app restart, when
  // `sessionResult` (this session's own in-memory run, if any) is unavailable.
  const lastRun = useAsync(() => api.get_last_ingest_run(), []);
  useBundleInvalidation(() => {
    sources.reload();
    stats.reload();
    sections.reload();
    lastRun.reload();
  }, ['inventory', 'sections']);

  // Grouped client-side, the same pattern `KnowledgeScreen.tsx`'s
  // `SectionCoverage` widget already uses — `list_sections()` returns the
  // whole bundle (no `source_id` filter exists or is needed) and grouping it
  // by `source_id` here is cheap. Per CATCHUP_PLAN.md's 2026-08-22
  // sections-view research, this column is "the single most actionable
  // finding" from that pass: a source collapsing into exactly one section
  // spanning its entire text (sparse/absent Markdown headings) was, until
  // now, invisible anywhere in the UI short of opening `.sopkb/sections.json`
  // by hand.
  const sectionCountBySource = new Map<string, number>();
  for (const section of sections.data ?? []) {
    sectionCountBySource.set(section.source_id, (sectionCountBySource.get(section.source_id) ?? 0) + 1);
  }

  const [pendingRetire, setPendingRetire] = useState<Source | null>(null);
  const [retiring, setRetiring] = useState(false);
  const [retireError, setRetireError] = useState<string | null>(null);

  // "Reveal bundle folder" / "Force resync" -- moved here from the now-removed
  // Export screen (round 6: OKF sync already runs automatically on every
  // ingest run and review action, so a dedicated export page had nothing left
  // to do besides these two utility actions).
  const [resyncing, setResyncing] = useState(false);
  const [utilityMessage, setUtilityMessage] = useState<{ text: string; ok: boolean } | null>(null);

  async function handleRevealBundleFolder() {
    setUtilityMessage(null);
    try {
      const dir = await api.get_export_dir();
      await api.reveal_path(dir);
    } catch (err) {
      setUtilityMessage({ text: err instanceof Error ? err.message : String(err), ok: false });
    }
  }

  async function handleForceResync() {
    setResyncing(true);
    setUtilityMessage(null);
    try {
      await api.sync_okf_documents();
      setUtilityMessage({ text: 'OKF documents resynced.', ok: true });
    } catch (err) {
      setUtilityMessage({ text: err instanceof Error ? err.message : String(err), ok: false });
    } finally {
      setResyncing(false);
    }
  }

  async function handleConfirmRetire() {
    if (!pendingRetire) return;
    setRetiring(true);
    setRetireError(null);
    try {
      await api.retire_source(pendingRetire.id, `Retired from Sources screen: ${pendingRetire.title}`);
      setPendingRetire(null);
      await sources.reload();
    } catch (err) {
      setRetireError(err instanceof Error ? err.message : String(err));
    } finally {
      setRetiring(false);
    }
  }

  if (sources.loading) return <LoadingBlock label="Loading sources…" />;
  if (sources.error) return <ErrorBanner message="Could not load sources" detail={sources.error} />;

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h1 className="text-xl font-semibold text-ink">Sources</h1>
          {stats.data && (
            <p className="mt-1 text-sm text-muted">
              {stats.data.total} total · {Object.entries(stats.data.by_parse_status).map(([status, count]) => `${count} ${status}`).join(', ')}
            </p>
          )}
        </div>
        <div className="flex flex-col items-end gap-1">
          <div className="flex gap-2">
            <Button variant="secondary" className="!py-1 !text-xs" onClick={() => void handleRevealBundleFolder()}>
              Reveal bundle folder
            </Button>
            <Button variant="secondary" className="!py-1 !text-xs" disabled={resyncing} onClick={() => void handleForceResync()}>
              {resyncing ? 'Resyncing…' : 'Force resync'}
            </Button>
          </div>
          {utilityMessage && <p className={`text-xs ${utilityMessage.ok ? 'text-ok' : 'text-bad'}`}>{utilityMessage.text}</p>}
        </div>
      </div>

      {ingestRunning && (
        <Link
          to="/ingest"
          className="flex items-center gap-2 rounded-lg border border-warn/40 bg-warn-soft px-3 py-2 text-sm text-warn hover:underline"
        >
          <span className="h-2 w-2 shrink-0 animate-pulse rounded-full bg-warn" />
          An ingest is currently running — view progress
        </Link>
      )}

      {!ingestRunning && (sessionResult || lastRun.data) && (
        <LastRunSummary sessionResult={sessionResult} persisted={lastRun.data} />
      )}

      {sources.data && sources.data.length === 0 && (
        <EmptyState>
          <p>No sources ingested into this bundle yet.</p>
          <Link to="/ingest" className="mt-2 inline-block text-accent underline">
            Go to Ingest to add sources
          </Link>
        </EmptyState>
      )}

      {sources.data && sources.data.length > 0 && (
        <div className="max-h-[70vh] overflow-y-auto rounded-lg border border-line bg-panel">
        <table className="w-full text-sm">
          <thead className="sticky top-0 bg-panel-muted text-left text-xs uppercase tracking-wide text-muted">
            <tr>
              <th className="px-4 py-2">Title</th>
              <th className="px-4 py-2">Type</th>
              <th className="px-4 py-2">Parse status</th>
              <th className="px-4 py-2">Size</th>
              <th className="px-4 py-2">Warnings</th>
              <th className="px-4 py-2">Sections</th>
              <th className="px-4 py-2" />
            </tr>
          </thead>
          <tbody className="divide-y divide-line-soft">
            {sources.data.map((source) => (
              <tr key={source.id} className={source.status === 'retired' ? 'opacity-60' : undefined}>
                <td className="px-4 py-2 font-medium text-ink">
                  {source.title}
                  {source.status === 'retired' && (
                    <span
                      className="ml-2 rounded-full bg-panel-muted px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-muted"
                      title="Retired — kept on disk, hidden from default agent context, reversible in principle"
                    >
                      Retired
                    </span>
                  )}
                </td>
                <td className="px-4 py-2"><EntityBadge kind="source" label={source.type} /></td>
                <td className="px-4 py-2 text-muted">{source.parse_status}</td>
                <td className="px-4 py-2 text-muted">{source.size.toLocaleString()} B</td>
                <td className="px-4 py-2 text-muted">
                  {source.warnings.length > 0 ? (
                    <span className="text-warn">{source.warnings.length}</span>
                  ) : (
                    <span className="text-muted-soft">0</span>
                  )}
                </td>
                <td className="px-4 py-2">
                  {sections.loading ? (
                    <span className="text-muted-soft">…</span>
                  ) : (
                    <SectionsCell count={sectionCountBySource.get(source.id) ?? 0} />
                  )}
                </td>
                <td className="px-4 py-2 text-right whitespace-nowrap">
                  <Link to={`/sources/${source.id}`} className="text-accent underline">
                    View
                  </Link>
                  <span className="mx-1.5 text-muted-soft">·</span>
                  <Link to="/ingest" className="text-accent underline" title="View the ingestion run that produced this source">
                    View run
                  </Link>
                  {source.status !== 'retired' && (
                    <>
                      <span className="mx-1.5 text-muted-soft">·</span>
                      <button
                        type="button"
                        onClick={() => setPendingRetire(source)}
                        className="text-bad underline decoration-bad/40"
                        title="Retire this source — non-destructive, reversible in principle"
                      >
                        Retire
                      </button>
                    </>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        </div>
      )}

      {retireError && <p className="text-sm text-bad">{retireError}</p>}

      {pendingRetire && (
        <ConfirmDialog
          title={`Retire "${pendingRetire.title}"?`}
          message="Non-destructive: the source's original file, normalized text, and evidence all stay on disk. Its status is marked retired and any still-active knowledge items mined from it stop appearing in the default agent context. Reversible in principle (there is no un-retire button yet, but nothing is deleted)."
          confirmLabel={retiring ? 'Retiring…' : 'Retire'}
          danger
          disabled={retiring}
          onConfirm={() => void handleConfirmRetire()}
          onCancel={() => setPendingRetire(null)}
        />
      )}
    </div>
  );
}

const RUN_STEPS = ['scan', 'normalize', 'mine', 'validate'] as const;

/**
 * "What happened in that run" -- shown once an ingest is no longer running,
 * so a completed/failed run is never invisible the moment the Ingest screen
 * is left. Prefers `sessionResult` (this session's own just-finished run,
 * richer and already bundle-scoped by `IngestRunContext`) over `persisted`
 * (`.sopkb/ingest_run.json`, the fallback that survives an app restart, when
 * nothing ran yet in this session).
 */
function LastRunSummary({ sessionResult, persisted }: { sessionResult: IngestResult | null; persisted: IngestRunStatus | null }) {
  const parts: string[] = [];
  let failedStep: string | undefined;
  let finishedAt: string | undefined;

  if (sessionResult) {
    if (sessionResult.uploaded_files != null) parts.push(`${sessionResult.uploaded_files} file(s) uploaded`);
    if (sessionResult.sources != null) parts.push(`${sessionResult.sources} source(s) scanned`);
    if (sessionResult.sections != null) parts.push(`${sessionResult.sections} section(s)`);
    if (sessionResult.items != null) parts.push(`${sessionResult.items} item(s) mined`);
    if (sessionResult.validation != null) {
      parts.push(`${sessionResult.validation.errors} validation error(s), ${sessionResult.validation.warnings} warning(s)`);
    }
  } else if (persisted) {
    for (const step of RUN_STEPS) {
      if (persisted.detail[step]) parts.push(persisted.detail[step]);
    }
    if (!persisted.ok) failedStep = RUN_STEPS.find((step) => persisted.status[step] === 'error');
    finishedAt = persisted.finished_at;
  }

  if (parts.length === 0 && !failedStep) return null;

  return (
    <div className="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-line bg-panel-muted px-3 py-2 text-sm">
      <div className="text-ink/80">
        {failedStep ? (
          <span className="text-bad">Last ingest failed at step: {failedStep.charAt(0).toUpperCase() + failedStep.slice(1)}</span>
        ) : (
          <span>Last ingest: {parts.join(' · ')}</span>
        )}
        {finishedAt && <span className="ml-2 text-xs text-muted-soft">{new Date(finishedAt).toLocaleString()}</span>}
      </div>
      <Link to="/ingest" className="shrink-0 text-accent underline">
        View details
      </Link>
    </div>
  );
}

/**
 * A source with exactly one section did not carve on any real Markdown
 * heading: `extract_sections` (`sopkb-core/src/normalize.rs`, lines 47-84)
 * always spans a lone section from the start of the file to EOF, so
 * "1 section" and "1 section covering the entire normalized text" are the
 * same fact here, not two things to check separately. Flagged in warn color
 * (matching this table's own Warnings column, not a new badge style)
 * because it's the likely root cause of at least one real mining-quality
 * complaint — a single oversized, undifferentiated section becomes one
 * oversized LLM request (`mine_with_author`'s per-section loop has no
 * length guard). See CATCHUP_PLAN.md's headline finding, 2026-08-22.
 */
function SectionsCell({ count }: { count: number }) {
  if (count === 1) {
    return (
      <span
        className="text-warn"
        title="Exactly one section, spanning the entire normalized text — likely no real headings were found in this source."
      >
        {count}
      </span>
    );
  }
  return <span className="text-muted">{count}</span>;
}
