/**
 * Small filled-track confidence bar + the numeric value.
 *
 * Adapted (pattern only, not code — different stack) from
 * `origin/ui/sopkb-web-redesign:tools/sopkb/sopkb/web_app.py`'s
 * `confidence_meter` (lines 1142-1147):
 *
 *   def confidence_meter(value: float) -> str:
 *       pct = max(0, min(100, round(value * 100)))
 *       return (
 *           f'<span class="conf-track"><span class="conf-fill" style="width:{pct}%"></span></span>'
 *           f"{value:.2f}"
 *       )
 *
 * and its `.conf-track`/`.conf-fill` CSS (lines 755-756) — a real
 * horizontal `<div>`-style bar, not literal block characters
 * ("▓▓▓░░ 0.82"). That branch is a visual/interaction redesign of the
 * original Python's own separate server-rendered UI (`sopkb serve`), not
 * this React app, so only the interaction pattern is ported here, in this
 * app's own Tailwind tokens: `bg-line` for the track (matches the
 * redesign's `var(--line)`) and `bg-accent` for the fill (matches its
 * `var(--accent)`) — see `components/common/StatusBadge.tsx` /
 * `Feedback.tsx` for the same token vocabulary used elsewhere.
 */
export function ConfidenceMeter({ value }: { value: number }) {
  const pct = Math.max(0, Math.min(100, Math.round(value * 100)));
  return (
    <span className="inline-flex items-center gap-1.5 tabular-nums">
      <span className="inline-block h-1 w-12 shrink-0 overflow-hidden rounded-full bg-line">
        <span className="block h-full rounded-full bg-accent" style={{ width: `${pct}%` }} />
      </span>
      <span className="text-ink">{value.toFixed(2)}</span>
    </span>
  );
}
