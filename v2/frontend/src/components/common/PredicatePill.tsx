/**
 * Small mono, uppercase, accent-tinted chip for a predicate string.
 *
 * Adapted (pattern only, not code — different stack) from
 * `origin/ui/sopkb-web-redesign:tools/sopkb/sopkb/web_app.py`'s
 * `predicate_pill` (lines 1150-1151):
 *
 *   def predicate_pill(predicate: str) -> str:
 *       return f'<span class="predicate">{escape(predicate)}</span>'
 *
 * and its `.predicate` CSS (line 757: mono font, 10.5px, uppercase,
 * 0.03em tracking, `var(--accent-soft)` background / `var(--accent)` text).
 * That branch restyles the original Python's own separate server-rendered
 * UI (`sopkb serve`), not this React app — only the interaction pattern is
 * ported here, reusing this app's existing `accent`/`accent-soft` pill
 * tokens (see `components/common/StatusBadge.tsx`) rather than inventing
 * new ones.
 */
export function PredicatePill({ predicate }: { predicate: string }) {
  return (
    <span className="inline-flex items-center rounded bg-accent-soft px-1.5 py-0.5 font-mono text-[10.5px] font-semibold uppercase tracking-[0.03em] text-accent">
      {predicate}
    </span>
  );
}
