import { useState } from 'react';
import { Link } from 'react-router-dom';
import { marked } from 'marked';
import DOMPurify from 'dompurify';
import * as api from '../lib/api';
import { useAsync, useBundleInvalidation } from '../lib/hooks';
import { LoadingBlock, ErrorBanner } from '../components/common/Feedback';
import type { ReviewStatus } from '../types/commands';

/** Reports are real Markdown files (see `sopkb-export::reports`) -- sanitized after
 * parsing since report text can embed values pulled from ingested documents
 * (headings, titles) that this app did not author itself. */
function renderReportMarkdown(text: string): string {
  return DOMPurify.sanitize(marked.parse(text, { async: false }));
}

const TILE_CLASS = 'block rounded-lg border border-line bg-panel-muted px-3 py-2.5 text-left transition-colors hover:bg-panel';

/**
 * At-a-glance bundle dashboard (round 7, item 17 -- renamed from a bare
 * tabbed Reports viewer, which is kept below as the detail half of this same
 * screen). Every stat tile aggregates a command some OTHER screen already
 * calls on its own (get_source_stats for Sources, list_knowledge_items for
 * Knowledge, get_concept_index for Concepts, get_validation_summary for the
 * Validation report) -- no new backend command needed, this screen just puts
 * them next to each other for the first time. Route stays `/reports`
 * (cosmetic rename only, see AppShell.tsx's NAV_ITEMS.reports.label).
 */
export function OverviewScreen() {
  const sourceStats = useAsync(() => api.get_source_stats(), []);
  const items = useAsync(() => api.list_knowledge_items(), []);
  const concepts = useAsync(() => api.get_concept_index(), []);
  const validation = useAsync(() => api.get_validation_summary(), []);
  const reports = useAsync(() => api.get_reports(), []);
  useBundleInvalidation(() => {
    sourceStats.reload();
    items.reload();
    concepts.reload();
    validation.reload();
    reports.reload();
  });
  const [activeIndex, setActiveIndex] = useState(0);

  const statusCounts: Partial<Record<ReviewStatus, number>> = {};
  for (const item of items.data ?? []) {
    statusCounts[item.review_status] = (statusCounts[item.review_status] ?? 0) + 1;
  }

  // Export summary isn't shown here -- OKF/graph/RDF export already happens
  // automatically on every ingest run and review action (see AppShell.tsx's
  // own note on why there's no dedicated Export screen either), so a report
  // about it doesn't carry the same "did something go wrong" signal the
  // other five do. Case-insensitive for the same reason `validationIndex`
  // below is: the real backend names each entry from its raw lowercase
  // `BUNDLE_REPORT_NAMES` identifier, the mock display-names them.
  const visibleReports = (reports.data ?? []).filter((r) => r.name.toLowerCase() !== 'export_summary');
  const validationIndex = visibleReports.findIndex((r) => r.name.toLowerCase() === 'validation');

  if (reports.loading) return <LoadingBlock label="Loading overview…" />;
  if (reports.error) return <ErrorBanner message="Could not load overview" detail={reports.error} />;
  if (!reports.data) return null;

  const active = visibleReports[activeIndex];
  // Not memoized: see the original ReportsScreen's own note -- `marked.parse` +
  // `DOMPurify.sanitize` on a ≤1800-char report body is cheap enough not to need it.
  const activeHtml = active?.present ? renderReportMarkdown(active.text) : '';

  return (
    <div className="space-y-6">
      <h1 className="text-xl font-semibold text-ink">Overview</h1>

      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        <Link to="/sources" className={TILE_CLASS}>
          <p className="text-xs text-muted">Sources</p>
          <p className="mt-0.5 text-lg font-semibold text-ink">{sourceStats.data ? sourceStats.data.total : '—'}</p>
          {sourceStats.data && (
            <p className="mt-0.5 truncate text-xs text-muted-soft">
              {Object.entries(sourceStats.data.by_parse_status)
                .map(([status, count]) => `${count} ${status}`)
                .join(', ') || 'no sources yet'}
            </p>
          )}
        </Link>
        <Link to="/knowledge" className={TILE_CLASS}>
          <p className="text-xs text-muted">Knowledge items</p>
          <p className="mt-0.5 text-lg font-semibold text-ink">{items.data ? items.data.length : '—'}</p>
          {items.data && (
            <p className="mt-0.5 truncate text-xs text-muted-soft">
              {Object.entries(statusCounts)
                .map(([status, count]) => `${count} ${status}`)
                .join(', ') || 'no items yet'}
            </p>
          )}
        </Link>
        <Link to="/concepts" className={TILE_CLASS}>
          <p className="text-xs text-muted">Concepts</p>
          <p className="mt-0.5 text-lg font-semibold text-ink">{concepts.data ? concepts.data.length : '—'}</p>
        </Link>
        <button
          type="button"
          disabled={validationIndex < 0}
          onClick={() => validationIndex >= 0 && setActiveIndex(validationIndex)}
          className={`${TILE_CLASS} disabled:cursor-default disabled:opacity-60`}
          title={validationIndex >= 0 ? 'Jump to the Validation report below' : undefined}
          aria-label="Overview stat: Validation"
        >
          <p className="text-xs text-muted">Validation</p>
          <p className="mt-0.5 text-lg font-semibold text-ink">
            {validation.data ? `${validation.data.errors} error${validation.data.errors === 1 ? '' : 's'}` : '—'}
          </p>
          {validation.data && (
            <p className="mt-0.5 text-xs text-muted-soft">
              {validation.data.warnings} warning{validation.data.warnings === 1 ? '' : 's'}
            </p>
          )}
        </button>
      </div>

      <div>
        <div className="flex gap-2 border-b border-line">
          {visibleReports.map((report, i) => (
            <button
              key={report.name}
              type="button"
              onClick={() => setActiveIndex(i)}
              className={`px-3 py-2 text-sm ${
                i === activeIndex ? 'border-b-2 border-accent font-medium text-ink' : 'text-muted'
              } ${!report.present ? 'opacity-50' : ''}`}
            >
              {report.name}
              {!report.present && ' (missing)'}
            </button>
          ))}
        </div>
        {active && (
          <div className="rounded-lg border border-line bg-panel p-4">
            {active.present ? (
              <div className="report-markdown" dangerouslySetInnerHTML={{ __html: activeHtml }} />
            ) : (
              <p className="text-sm text-muted">This report has not been generated for this bundle yet.</p>
            )}
            <p className="mt-3 text-xs text-muted-soft">{active.path}</p>
          </div>
        )}
      </div>
    </div>
  );
}
