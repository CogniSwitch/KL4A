import { useState } from 'react';
import { Button } from './Button';

/**
 * A real modal for an irreversible, destructive action -- deliberately not a
 * `window.confirm()` single click-through. When `confirmText` is given, the
 * confirm button stays disabled until the user types it exactly, matching
 * how other tools gate irreversible deletes (e.g. "type the bundle name to
 * confirm").
 */
export function ConfirmDialog({
  title,
  message,
  confirmLabel = 'Confirm',
  confirmText,
  danger = false,
  disabled = false,
  onConfirm,
  onCancel,
}: {
  title: string;
  message: string;
  confirmLabel?: string;
  /** When set, the confirm button is disabled until the user types this exact string. */
  confirmText?: string;
  danger?: boolean;
  /** Extra disable condition (e.g. the confirm action is already in flight). */
  disabled?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const [typed, setTyped] = useState('');
  const canConfirm = !disabled && (confirmText === undefined || typed === confirmText);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
      role="presentation"
      onClick={onCancel}
    >
      <div
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="confirm-dialog-title"
        className="w-full max-w-md rounded-lg border border-line bg-panel p-5 shadow-lg"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 id="confirm-dialog-title" className="text-base font-semibold text-ink">
          {title}
        </h2>
        <p className="mt-2 text-sm text-muted">{message}</p>

        {confirmText !== undefined && (
          <div className="mt-4">
            <label className="block text-xs font-medium text-muted">
              Type <span className="font-mono text-ink">{confirmText}</span> to confirm
            </label>
            <input
              type="text"
              value={typed}
              onChange={(e) => setTyped(e.target.value)}
              autoFocus
              className="mt-1 w-full rounded-md border border-line bg-panel-muted px-3 py-1.5 text-sm text-ink outline-none focus:border-accent"
            />
          </div>
        )}

        <div className="mt-5 flex justify-end gap-2">
          <Button variant="secondary" onClick={onCancel}>
            Cancel
          </Button>
          <Button variant={danger ? 'danger' : 'primary'} disabled={!canConfirm} onClick={onConfirm}>
            {confirmLabel}
          </Button>
        </div>
      </div>
    </div>
  );
}
