import type { ReactNode } from 'react';

export function LoadingBlock({ label = 'Loading…' }: { label?: string }) {
  return (
    <div className="flex items-center gap-2 py-8 text-sm text-muted" role="status">
      <span className="h-4 w-4 animate-spin rounded-full border-2 border-line border-t-accent" />
      {label}
    </div>
  );
}

export function ErrorBanner({ message, detail }: { message: string; detail?: string }) {
  return (
    <div className="rounded-lg border border-bad/30 bg-bad-soft px-4 py-3 text-sm text-bad" role="alert">
      <p className="font-medium">{message}</p>
      {detail && <p className="mt-1 opacity-90">{detail}</p>}
    </div>
  );
}

export function EmptyState({ children }: { children: ReactNode }) {
  return (
    <div className="rounded-lg border border-dashed border-line bg-panel-muted/50 px-4 py-8 text-center text-sm text-muted">
      {children}
    </div>
  );
}

/**
 * Truncates to `cap` characters and shows a "Show more" toggle rather than
 * silently clipping — presentation caps (900/1800/86-char, per §4.4) are
 * meant to be normal, not surprising.
 */
export function Truncated({ text, cap }: { text: string; cap: number }) {
  const isTruncated = text.length > cap;
  if (!isTruncated) return <>{text}</>;
  return (
    <details className="inline">
      <summary className="inline cursor-pointer list-none text-inherit marker:hidden">
        {text.slice(0, cap)}
        <span className="text-muted-soft"> … </span>
        <span className="text-accent underline">show more</span>
      </summary>
      <span>{text.slice(cap)}</span>
    </details>
  );
}
