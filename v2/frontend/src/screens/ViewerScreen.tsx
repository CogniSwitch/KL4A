import { useState } from 'react';
import { Link, useParams } from 'react-router-dom';
import * as api from '../lib/api';
import { useAsync, useBundleInvalidation } from '../lib/hooks';
import { LoadingBlock, ErrorBanner } from '../components/common/Feedback';
import { StatusBadge } from '../components/common/StatusBadge';
import type { KnowledgeItem, Section } from '../types/commands';

/**
 * Normalized-text viewer, plus a per-source section TOC and evidence list.
 * `get_normalized_text`'s 900-char cap (§4.4) is a real param, not silently
 * applied. The two new panels are pure client-side filtering of
 * `list_sections()`/`list_knowledge_items()` — both already fetch the WHOLE
 * bundle (neither has, or needs, a `source_id` parameter). Per
 * CATCHUP_PLAN.md's 2026-08-22 sections-view research: `list_sections`/
 * `get_section` had exactly one prior caller anywhere in the app
 * (`KnowledgeScreen`'s `SectionCoverage` widget) before this screen, and the
 * evidence list restores something the original Python Viewer
 * (`web_app.py::render_viewer`, ~line 981) had that jt-dev's port dropped.
 */
export function ViewerScreen() {
  const { sourceId } = useParams<{ sourceId: string }>();
  const source = useAsync(() => api.get_source(sourceId!), [sourceId]);
  const [expanded, setExpanded] = useState(false);
  const text = useAsync(() => api.get_normalized_text(sourceId!, expanded ? undefined : 900), [sourceId, expanded]);
  const sections = useAsync(() => api.list_sections(), []);
  const items = useAsync(() => api.list_knowledge_items(), []);
  useBundleInvalidation(() => {
    source.reload();
    text.reload();
    sections.reload();
    items.reload();
  }, ['inventory', 'sections', 'items', 'reviews']);

  if (source.loading) return <LoadingBlock label="Loading source…" />;
  if (source.error) return <ErrorBanner message="Could not load source" detail={source.error} />;
  if (!source.data) return null;

  // Sections partition a source's normalized text end-to-end with no gaps or
  // overlaps (`extract_sections`, CATCHUP_PLAN.md's headline finding) — sort
  // by `start_pos` so the TOC reads top-to-bottom like the document itself,
  // not in whatever order the backend happens to return them.
  const sourceSections = (sections.data ?? [])
    .filter((s) => s.source_id === sourceId)
    .slice()
    .sort((a, b) => a.start_pos - b.start_pos);
  const sourceItems = (items.data ?? []).filter((i) => i.source_id === sourceId);
  const sectionHeadingById = new Map(sourceSections.map((s) => [s.id, s.heading]));

  return (
    <div className="space-y-4">
      <div>
        <Link to="/sources" className="text-sm text-muted underline">
          ← Back to sources
        </Link>
        <h1 className="mt-1 text-xl font-semibold text-ink">{source.data.title}</h1>
        <p className="text-xs text-muted">{source.data.original_path}</p>
      </div>

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-3">
        <div className="max-h-[70vh] overflow-y-auto rounded-lg border border-line bg-panel p-4 lg:col-span-2">
          {text.loading && <LoadingBlock label="Loading normalized text…" />}
          {text.error && <ErrorBanner message="Could not load normalized text" detail={text.error} />}
          {text.data && !text.data.present && <p className="text-sm text-muted">No normalized text on file.</p>}
          {text.data && text.data.present && (
            <>
              <pre className="whitespace-pre-wrap font-sans text-sm text-ink/90">{text.data.text}</pre>
              {text.data.truncated && (
                <button type="button" onClick={() => setExpanded(true)} className="mt-3 text-sm text-accent underline">
                  Show full text
                </button>
              )}
              {!text.data.truncated && expanded && (
                <button type="button" onClick={() => setExpanded(false)} className="mt-3 text-sm text-accent underline">
                  Collapse to preview
                </button>
              )}
            </>
          )}
        </div>

        <SectionToc sections={sourceSections} loading={sections.loading} error={sections.error} />
      </div>

      <EvidenceList items={sourceItems} sectionHeadingById={sectionHeadingById} loading={items.loading} error={items.error} />
    </div>
  );
}

const SEMANTIC_ROLE_CLASSES: Record<string, string> = {
  procedure: 'bg-accent-soft text-accent ring-accent/30',
  policy: 'bg-warn-soft text-warn ring-warn/30',
  section: 'bg-panel-muted text-muted ring-line',
};

/**
 * `semantic_role` is open-ended text on the wire, not a closed enum (unlike
 * `ReviewStatus`) — falls back to the same neutral style `StatusBadge` uses
 * for a status it doesn't recognize, rather than rendering unstyled text for
 * a role this map hasn't seen yet.
 */
function SectionRoleBadge({ role }: { role: string }) {
  const style = SEMANTIC_ROLE_CLASSES[role] ?? 'bg-panel-muted text-muted ring-line';
  return (
    <span className={`inline-flex shrink-0 items-center rounded-full px-2 py-0.5 text-xs font-medium ring-1 ring-inset ${style}`}>
      {role}
    </span>
  );
}

/**
 * Per-source section TOC — heading, semantic-role badge, and span size in
 * characters (`end_pos - start_pos`). All three fields already exist on
 * `SectionRecord` (`sopkb-core/src/models.rs`) and `list_sections` already
 * exposes them end-to-end (`reads.rs:95-103`, `api.ts:385-392`); this was
 * simply never rendered per-source anywhere before now. A source with
 * exactly one section here means that section spans the WHOLE normalized
 * file, by construction of `extract_sections` — see `SourcesScreen.tsx`'s
 * `SectionsCell` for where that pathology is flagged at the list level.
 */
function SectionToc({ sections, loading, error }: { sections: Section[]; loading: boolean; error: string | null }) {
  return (
    <section className="rounded-lg border border-line bg-panel p-4">
      <h2 className="text-sm font-semibold text-ink">Sections{sections.length > 0 && ` (${sections.length})`}</h2>
      {loading && <LoadingBlock label="Loading sections…" />}
      {error && <ErrorBanner message="Could not load sections" detail={error} />}
      {!loading && !error && sections.length === 0 && (
        <p className="mt-2 text-sm text-muted">No sections recorded for this source yet.</p>
      )}
      {!loading && !error && sections.length > 0 && (
        <ul className="mt-2 max-h-[70vh] space-y-2 overflow-y-auto text-sm">
          {sections.map((s) => (
            <li key={s.id} className="border-b border-line-soft pb-2 last:border-0 last:pb-0">
              <div className="flex items-center justify-between gap-2">
                <span className="font-medium text-ink" title={s.heading}>
                  {s.heading}
                </span>
                <SectionRoleBadge role={s.semantic_role} />
              </div>
              <p className="mt-0.5 text-xs text-muted">{(s.end_pos - s.start_pos).toLocaleString()} chars</p>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

/**
 * Restores the original Python Viewer's per-source evidence list
 * (`web_app.py::render_viewer`, ~line 981 — see CATCHUP_PLAN.md's
 * 2026-08-22 sections-view research) — every knowledge item mined from THIS
 * source, shown below its text. No backend change needed: `KnowledgeItem`
 * already carries `source_id` on every item, so this is pure client-side
 * filtering of the same `list_knowledge_items()` `KnowledgeScreen` already
 * calls. Row visuals intentionally mirror `KnowledgeScreen`'s `ResultTable`
 * (subject bolded, `StatusBadge`, truncate-with-title) rather than
 * inventing a new item-row style; the Section column cross-references
 * `section_id` back to the TOC above.
 */
function EvidenceList({
  items,
  sectionHeadingById,
  loading,
  error,
}: {
  items: KnowledgeItem[];
  sectionHeadingById: Map<string, string>;
  loading: boolean;
  error: string | null;
}) {
  return (
    <section className="rounded-lg border border-line bg-panel p-4">
      <h2 className="text-sm font-semibold text-ink">Evidence{items.length > 0 && ` (${items.length})`}</h2>
      {loading && <LoadingBlock label="Loading knowledge items…" />}
      {error && <ErrorBanner message="Could not load knowledge items" detail={error} />}
      {!loading && !error && items.length === 0 && (
        <p className="mt-2 text-sm text-muted">No knowledge items mined from this source yet.</p>
      )}
      {!loading && !error && items.length > 0 && (
        <div className="mt-3 max-h-[420px] overflow-y-auto rounded-lg border border-line">
          <table className="w-full text-sm">
            <thead className="sticky top-0 bg-panel-muted text-left text-xs uppercase tracking-wide text-muted">
              <tr>
                <th className="px-4 py-2">Subject</th>
                <th className="px-4 py-2">Section</th>
                <th className="px-4 py-2">Excerpt</th>
                <th className="px-4 py-2">Status</th>
                <th className="px-4 py-2" />
              </tr>
            </thead>
            <tbody className="divide-y divide-line-soft">
              {items.map((item) => (
                <tr key={item.id}>
                  <td className="px-4 py-2 font-medium text-ink">{item.subject}</td>
                  <td
                    className="max-w-[10rem] truncate px-4 py-2 text-muted"
                    title={sectionHeadingById.get(item.section_id) ?? item.section_id}
                  >
                    {sectionHeadingById.get(item.section_id) ?? item.section_id}
                  </td>
                  <td className="max-w-md truncate px-4 py-2 text-muted" title={item.source_text}>
                    {item.source_text}
                  </td>
                  <td className="px-4 py-2">
                    <StatusBadge status={item.review_status} />
                  </td>
                  <td className="px-4 py-2 text-right">
                    <Link to={`/review/${item.id}`} className="text-accent underline">
                      Review
                    </Link>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
