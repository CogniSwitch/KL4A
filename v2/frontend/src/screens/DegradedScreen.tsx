import { useState } from 'react';
import { useWorkbench } from '../context/WorkbenchContext';
import { pick_workbench_folder } from '../lib/api';
import { Button } from '../components/common/Button';
import { Input } from '../components/common/Input';

/**
 * Shown when `get_workbench_context` either fails outright or returns
 * `mode: "Degraded"` (root doesn't exist, is unreadable, or has a malformed
 * manifest — §4.12 explicitly retires `/healthz` in favor of exactly this
 * check at mount). This replaces a crash or a blank window with a legible
 * recovery path: re-pick a folder, or manually enter a path.
 */
export function DegradedScreen() {
  const { context, loadError, switchRoot, refresh } = useWorkbench();
  const [pathInput, setPathInput] = useState('');
  const [busy, setBusy] = useState(false);
  const [switchError, setSwitchError] = useState<string | null>(null);

  const reason = loadError ?? context?.error ?? 'The workbench root could not be loaded.';

  async function handleSwitch(path: string) {
    setBusy(true);
    setSwitchError(null);
    try {
      await switchRoot(path);
    } catch (err) {
      setSwitchError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleBrowse() {
    setBusy(true);
    setSwitchError(null);
    try {
      const picked = await pick_workbench_folder();
      if (picked) {
        await switchRoot(picked);
      }
    } catch (err) {
      setSwitchError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex h-screen w-screen items-center justify-center bg-bg px-6">
      <div className="w-full max-w-md rounded-xl border border-line bg-panel p-6 shadow-sm">
        <h1 className="text-lg font-semibold text-ink">Workbench unavailable</h1>
        <p className="mt-2 text-sm text-muted">
          The app could not open a workbench root. This usually means the folder was moved or
          deleted, or its manifest is malformed.
        </p>
        <div className="mt-3 rounded-lg bg-bad-soft px-3 py-2 text-sm text-bad">{reason}</div>

        <div className="mt-5 space-y-2">
          <label className="text-sm font-medium text-ink" htmlFor="root-path">
            Point at a different folder
          </label>
          <div className="flex gap-2">
            <Input
              id="root-path"
              type="text"
              value={pathInput}
              onChange={(e) => setPathInput(e.target.value)}
              placeholder="C:\Users\you\KL4A Workbench"
              className="flex-1"
            />
            <Button variant="primary" disabled={busy || !pathInput.trim()} onClick={() => handleSwitch(pathInput.trim())}>
              Switch
            </Button>
          </div>
          {switchError && <p className="text-sm text-bad">{switchError}</p>}
        </div>

        <div className="mt-4 flex items-center justify-between border-t border-line-soft pt-4">
          <Button variant="ghost" onClick={() => void refresh()} className="!text-muted decoration-muted/40">
            Retry
          </Button>
          <Button variant="ghost" disabled={busy} onClick={() => void handleBrowse()} className="!text-muted decoration-muted/40">
            Browse for a folder…
          </Button>
        </div>
      </div>
    </div>
  );
}
