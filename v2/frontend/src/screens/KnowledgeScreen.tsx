import { useState } from 'react';
import { Link } from 'react-router-dom';
import * as api from '../lib/api';
import { useAsync, useBundleInvalidation } from '../lib/hooks';
import { LoadingBlock, ErrorBanner, EmptyState } from '../components/common/Feedback';
import { StatusBadge } from '../components/common/StatusBadge';
import { ConfidenceMeter } from '../components/common/ConfidenceMeter';
import { PredicatePill } from '../components/common/PredicatePill';
import { Input } from '../components/common/Input';
import type { KnowledgeItem, KnowledgeSearchHit, Section, Source } from '../types/commands';

/**
 * §4.4: the original Python UI never wired `search_knowledge` to this
 * screen despite the backend supporting it — flagged in PORT_PLAN as an
 * easy, documented win. Wired here for real. Per D §4 / D 3: an
 * empty/whitespace query would return the *entire bundle* unlimited if we
 * called it — so we don't call `search_knowledge` at all until there's a
 * non-blank query, and fall back to the plain `list_knowledge_items` view
 * otherwise.
 */
export function KnowledgeScreen() {
  const [query, setQuery] = useState('');
  const trimmed = query.trim();

  const items = useAsync(() => api.list_knowledge_items(), []);
  const sections = useAsync(() => api.list_sections(), []);
  const sources = useAsync(() => api.list_sources(), []);
  const searchResults = useAsync(
    () => (trimmed ? api.search_knowledge(trimmed) : Promise.resolve<KnowledgeSearchHit[] | null>(null)),
    [trimmed],
  );
  useBundleInvalidation(() => {
    items.reload();
    sections.reload();
    sources.reload();
    if (trimmed) searchResults.reload();
  }, ['items', 'reviews', 'sections', 'inventory']);

  const showingSearch = trimmed.length > 0;
  const loading = showingSearch ? searchResults.loading : items.loading;
  const error = showingSearch ? searchResults.error : items.error;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold text-ink">Knowledge</h1>
        <Input
          type="search"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search subject, predicate, object, or source text…"
          className="w-80"
        />
      </div>

      {items.data && sections.data && (
        <SectionCoverage items={items.data} sections={sections.data} sources={sources.data ?? []} />
      )}

      {loading && <LoadingBlock label={showingSearch ? 'Searching…' : 'Loading knowledge items…'} />}
      {error && <ErrorBanner message="Could not load knowledge items" detail={error} />}

      {!loading && !error && showingSearch && searchResults.data && (
        <ResultTable rows={searchResults.data} kind="search" />
      )}
      {!loading && !error && !showingSearch && items.data && <ResultTable rows={items.data} kind="list" />}
    </div>
  );
}

/**
 * Knowledge-gap visualization ported from oss-launch's EvidencePanel
 * "Section coverage" block (`web/src/components/EvidencePanel.tsx` +
 * `InspectSurface.tsx`'s `coverageFraction`/`gapHeadings` computation).
 * Not present in any form on jt-dev before this — confirmed by grepping for
 * `list_sections`/`get_section` usage across `src/screens`, which turned up
 * nothing outside `lib/api.ts`/`mock/api.mock.ts` themselves.
 *
 * oss-launch computed this only over "active" (non-retired) items —
 * jt-dev's `KnowledgeItem` has no lifecycle/retirement field yet (that's
 * catch-up plan workstream 2, not this one), so every item returned by
 * `list_knowledge_items` counts here; there is currently no other notion
 * of "inactive" to exclude.
 */
function SectionCoverage({ items, sections, sources }: { items: KnowledgeItem[]; sections: Section[]; sources: Source[] }) {
  if (sections.length === 0) return null;

  const coveredSectionIds = new Set(items.map((item) => item.section_id));
  const coveredCount = sections.filter((s) => coveredSectionIds.has(s.id)).length;
  const pct = Math.round((coveredCount / sections.length) * 100);
  const gapSections = sections.filter((s) => !coveredSectionIds.has(s.id));
  const sourceTitleById = new Map(sources.map((s) => [s.id, s.title]));

  return (
    <section className="rounded-lg border border-line bg-panel p-4">
      <h2 className="text-sm font-semibold text-ink">Section coverage</h2>
      <div className="mt-2 h-2 w-full overflow-hidden rounded-full bg-line">
        <div className="h-full rounded-full bg-ok transition-all" style={{ width: `${pct}%` }} />
      </div>
      <p className="mt-1.5 text-xs text-muted">
        {pct}% of sections ({coveredCount} / {sections.length}) have at least one knowledge item.
      </p>

      {gapSections.length > 0 && (
        <div className="mt-3">
          <h3 className="text-xs font-semibold uppercase tracking-wide text-muted-soft">
            Sections with no knowledge item ({gapSections.length})
          </h3>
          <ul className="mt-1.5 max-h-48 space-y-1 overflow-y-auto text-sm text-ink/80">
            {gapSections.map((s) => (
              <li key={s.id} className="truncate" title={`${sourceTitleById.get(s.source_id) ?? s.source_id} › ${s.heading}`}>
                <span className="text-muted-soft">{sourceTitleById.get(s.source_id) ?? s.source_id}</span>
                <span className="mx-1 text-muted-soft">›</span>
                {s.heading}
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}

/**
 * Predicate and Confidence columns use `PredicatePill`/`ConfidenceMeter`
 * (CATCHUP_PLAN.md's "ui/sopkb-web-redesign branch scan", idea 5) instead
 * of plain text / a bare number — matching the original Python's
 * `render_knowledge` table (`web_app.py` lines 1154-1164), which pairs the
 * same two components on every knowledge row. The Confidence column itself
 * is new here: jt-dev's `list_knowledge_items` table had no confidence
 * column at all before this, even though `KnowledgeItem` always carries the
 * field — only `search_knowledge` hits (`KnowledgeSearchHit`) omit it.
 */
function ResultTable({ rows, kind }: { rows: (KnowledgeItem | KnowledgeSearchHit)[]; kind: 'list' | 'search' }) {
  if (rows.length === 0) {
    // "search" with zero hits means the bundle has data but this query didn't match
    // it — pointing at Ingest there would be misleading. "list" (no query typed) with
    // zero rows means the bundle itself has nothing yet.
    if (kind === 'search') return <EmptyState>No knowledge items match.</EmptyState>;
    return (
      <EmptyState>
        <p>No knowledge items in this bundle yet.</p>
        <Link to="/ingest" className="mt-2 inline-block text-accent underline">
          Go to Ingest to add and mine sources
        </Link>
      </EmptyState>
    );
  }
  return (
    <div className="max-h-[70vh] overflow-auto rounded-lg border border-line bg-panel">
      <table className="w-full table-fixed text-sm">
        <thead className="sticky top-0 bg-panel-muted text-left text-xs uppercase tracking-wide text-muted">
          <tr>
            <th className="w-[14%] px-4 py-2">Subject</th>
            <th className="w-[12%] px-4 py-2">Predicate</th>
            <th className="w-[36%] px-4 py-2">Object</th>
            <th className="w-[12%] px-4 py-2">Status</th>
            <th className="w-[16%] px-4 py-2">Confidence</th>
            {/* Deliberately no fixed/percentage width -- with `table-fixed`, this
                column gets whatever's left after the others (~10%), which reliably
                leaves room for "Review" even after the wrapper's vertical scrollbar
                (see `overflow-auto` below) eats into the available width. Explicit
                percentages summing to exactly 100% left no slack for that and the
                link was rendering clipped under the scrollbar track. */}
            <th className="px-4 py-2" />
          </tr>
        </thead>
        <tbody className="divide-y divide-line-soft">
          {rows.map((row) => (
            <tr key={row.id}>
              <td className="truncate px-4 py-2 font-medium text-ink" title={row.subject}>
                {row.subject}
              </td>
              <td className="px-4 py-2">
                <PredicatePill predicate={row.predicate} />
              </td>
              <td className="truncate px-4 py-2 text-muted" title={row.object}>
                {row.object}
              </td>
              <td className="px-4 py-2">
                <StatusBadge status={row.review_status} />
              </td>
              <td className="px-4 py-2">
                {/* `KnowledgeSearchHit` (kind === 'search') omits `confidence` — see the caption below. */}
                {'confidence' in row ? <ConfidenceMeter value={row.confidence} /> : <span className="text-muted-soft">—</span>}
              </td>
              <td className="px-4 py-2 text-right">
                <Link to={`/review/${row.id}`} className="text-accent underline">
                  Review
                </Link>
              </td>
            </tr>
          ))}
        </tbody>
        {kind === 'search' && (
          <caption className="caption-bottom px-4 py-2 text-left text-xs text-muted-soft">
            Search hits use the `evidence` field (a rename of `source_text`) and omit confidence/span_status — the
            list view shows the full item shape.
          </caption>
        )}
      </table>
    </div>
  );
}
