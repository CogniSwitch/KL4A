import type { ReviewStatus } from '../../types/commands';

const STYLES: Record<ReviewStatus, string> = {
  proposed: 'bg-panel-muted text-muted ring-line',
  approved: 'bg-ok-soft text-ok ring-ok/30',
  rejected: 'bg-bad-soft text-bad ring-bad/30',
  deferred: 'bg-warn-soft text-warn ring-warn/30',
  edited: 'bg-accent-soft text-accent ring-accent/30',
};

export function StatusBadge({ status }: { status: ReviewStatus | string }) {
  const style = STYLES[status as ReviewStatus] ?? 'bg-panel-muted text-muted ring-line';
  return (
    <span className={`inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium ring-1 ring-inset ${style}`}>
      {status}
    </span>
  );
}
