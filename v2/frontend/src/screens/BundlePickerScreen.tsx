import { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import * as api from '../lib/api';
import { useAsync } from '../lib/hooks';
import { useWorkbench } from '../context/WorkbenchContext';
import { LoadingBlock, ErrorBanner, EmptyState } from '../components/common/Feedback';
import { Button } from '../components/common/Button';
import { Input, Select } from '../components/common/Input';
import { ConfirmDialog } from '../components/common/ConfirmDialog';
import type { BundleCard } from '../types/commands';

function switchRootErrorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

type SortMode = 'folder_name' | 'alphabetical' | 'last_created';
type SortDirection = 'asc' | 'desc';
type ViewMode = 'grid' | 'list';

// `'folder_name'` was previously labeled "Default" -- but it IS a real sort (the
// backend's `list_bundle_dirs` sorts case-insensitively by directory name, see
// sopkb-workbench/src/bundles.rs), not an unsorted/natural order, and it's a
// DIFFERENT field than `'alphabetical'` below (which sorts by the display `title`,
// not the folder/key) -- calling one "Default" and the other "Alphabetical" implied
// a distinction that wasn't the real one.
const SORT_LABELS: Record<SortMode, string> = {
  folder_name: 'Alphabetical (folder name)',
  alphabetical: 'Alphabetical (title)',
  last_created: 'Last created',
};

const SORT_MODE_STORAGE_KEY = 'sopkb.bundlePicker.sortMode';
const SORT_DIRECTION_STORAGE_KEY_PREFIX = 'sopkb.bundlePicker.sortDirection.';
const VIEW_MODE_STORAGE_KEY = 'sopkb.bundlePicker.viewMode';

// Each mode's OWN sensible default direction -- "ascending" means A→Z for the two
// alphabetical modes, but oldest→newest for a date, and the actually-useful reading
// of "Last created" is newest-first (descending). Direction is remembered PER MODE
// (not as one value shared across all three): a single shared default would either
// make alphabetical open Z→A (if defaulted to match last_created's "desc") or make
// last_created open oldest-first (if defaulted to "asc") -- a real bug caught by
// this file's own "sorts alphabetically by title" test regressing when this was
// first written as a single shared direction.
const DEFAULT_DIRECTION_BY_MODE: Record<SortMode, SortDirection> = {
  folder_name: 'asc',
  alphabetical: 'asc',
  last_created: 'desc',
};

/**
 * Persisted in `localStorage` (per-device UI preference, not bundle data --
 * nothing here needs to sync or be authoritative) so these choices survive an app
 * restart instead of silently resetting every time. Wrapped in try/catch: a
 * private-browsing context or a disabled-storage policy must never break the picker
 * over a preference that's fine to lose.
 */
function loadStored<T extends string>(key: string, valid: readonly T[], fallback: T): T {
  try {
    const stored = window.localStorage.getItem(key);
    if (stored && (valid as readonly string[]).includes(stored)) return stored as T;
  } catch {
    // Storage unavailable -- fall through to the default below.
  }
  return fallback;
}

function storeValue(key: string, value: string): void {
  try {
    window.localStorage.setItem(key, value);
  } catch {
    // Best-effort only -- losing the preference is fine, crashing is not.
  }
}

// Real, recently-created-first default (per explicit request) instead of the
// previous silent "whatever order the backend happens to return" default -- a new
// user's most relevant bundle (the one they just made) is now the first thing they
// see rather than wherever it lands alphabetically.
const DEFAULT_SORT_MODE: SortMode = 'last_created';
const DEFAULT_VIEW_MODE: ViewMode = 'grid';

/**
 * `direction` is a literal ascending/descending toggle on whichever field `mode`
 * picks (folder name / title A→Z vs Z→A; creation time oldest→newest vs
 * newest→oldest) -- it does not get silently re-interpreted per mode. Which
 * direction is "the sensible default" differs per mode (see
 * `DEFAULT_DIRECTION_BY_MODE`), but once loaded, this function treats it uniformly.
 */
function sortBundles(cards: BundleCard[], mode: SortMode, direction: SortDirection): BundleCard[] {
  const sorted = [...cards];
  if (mode === 'folder_name') {
    sorted.sort((a, b) => a.key.localeCompare(b.key));
  } else if (mode === 'alphabetical') {
    sorted.sort((a, b) => a.title.localeCompare(b.title));
  } else {
    sorted.sort((a, b) => a.created_at.localeCompare(b.created_at));
  }
  return direction === 'desc' ? sorted.reverse() : sorted;
}

/**
 * The one bundle-index model for both workbench modes (§4.1: "mode must
 * remain internal state even if the UI presents one model"). In
 * `SingleBundle` mode the list simply has one entry; this screen does not
 * branch on `mode` at all.
 *
 * `list_bundles` must surface `load_error` per card rather than silently
 * dropping broken bundles from the index (§4.2) — the mock always includes
 * one bundle with a `load_error` so that requirement stays visibly true
 * rather than becoming a claim nobody can see.
 */
export function BundlePickerScreen() {
  const { context, selectBundle } = useWorkbench();
  const bundles = useAsync(() => api.list_bundles(), []);
  const navigate = useNavigate();
  const [newTitle, setNewTitle] = useState('');
  const [creating, setCreating] = useState(false);
  const [rootDraft, setRootDraft] = useState('');
  const [switching, setSwitching] = useState(false);
  const [switchError, setSwitchError] = useState<string | null>(null);
  const { switchRoot } = useWorkbench();
  const [sortMode, setSortModeState] = useState<SortMode>(() =>
    loadStored(SORT_MODE_STORAGE_KEY, ['folder_name', 'alphabetical', 'last_created'] as const, DEFAULT_SORT_MODE),
  );
  const [sortDirection, setSortDirectionState] = useState<SortDirection>(() =>
    loadStored(SORT_DIRECTION_STORAGE_KEY_PREFIX + sortMode, ['asc', 'desc'] as const, DEFAULT_DIRECTION_BY_MODE[sortMode]),
  );
  const [viewMode, setViewModeState] = useState<ViewMode>(() => loadStored(VIEW_MODE_STORAGE_KEY, ['grid', 'list'] as const, DEFAULT_VIEW_MODE));
  function setSortMode(mode: SortMode) {
    setSortModeState(mode);
    storeValue(SORT_MODE_STORAGE_KEY, mode);
    // Direction is remembered per mode -- switching modes restores THAT mode's own
    // last direction (or its sensible default the first time), never carries over
    // the direction that happened to be active for the PREVIOUS mode.
    setSortDirectionState(loadStored(SORT_DIRECTION_STORAGE_KEY_PREFIX + mode, ['asc', 'desc'] as const, DEFAULT_DIRECTION_BY_MODE[mode]));
  }
  function toggleSortDirection() {
    const next: SortDirection = sortDirection === 'asc' ? 'desc' : 'asc';
    setSortDirectionState(next);
    storeValue(SORT_DIRECTION_STORAGE_KEY_PREFIX + sortMode, next);
  }
  function setViewMode(mode: ViewMode) {
    setViewModeState(mode);
    storeValue(VIEW_MODE_STORAGE_KEY, mode);
  }
  const [pendingDelete, setPendingDelete] = useState<BundleCard | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);

  const sortedBundles = useMemo(
    () => (bundles.data ? sortBundles(bundles.data, sortMode, sortDirection) : []),
    [bundles.data, sortMode, sortDirection],
  );

  async function handleSelect(key: string) {
    await selectBundle(key);
    navigate('/knowledge');
  }

  async function handleConfirmDelete() {
    if (!pendingDelete) return;
    setDeleting(true);
    setDeleteError(null);
    try {
      await api.delete_bundle(pendingDelete.key);
      // Reload BEFORE dismissing the dialog: closing it first would leave a
      // brief window where the dialog is gone but the deleted bundle's card
      // still shows (the list reload is its own separate, latent call).
      await bundles.reload();
      setPendingDelete(null);
    } catch (err) {
      setDeleteError(err instanceof Error ? err.message : String(err));
    } finally {
      setDeleting(false);
    }
  }

  async function handleSwitchTyped() {
    const path = rootDraft.trim();
    if (!path) return;
    setSwitching(true);
    setSwitchError(null);
    try {
      await switchRoot(path);
      setRootDraft('');
    } catch (err) {
      setSwitchError(switchRootErrorMessage(err));
    } finally {
      setSwitching(false);
    }
  }

  // Mirrors DegradedScreen.tsx's Browse button -- this screen previously had no
  // way to trigger the native folder picker at all, only the manual-path input,
  // which left "Switch workbench root..." with no way to actually pick a folder.
  async function handleBrowse() {
    setSwitching(true);
    setSwitchError(null);
    try {
      const picked = await api.pick_workbench_folder();
      if (picked) {
        await switchRoot(picked);
        setRootDraft('');
      }
    } catch (err) {
      setSwitchError(switchRootErrorMessage(err));
    } finally {
      setSwitching(false);
    }
  }

  async function handleCreate() {
    if (!newTitle.trim()) return;
    setCreating(true);
    try {
      const result = await api.create_project(newTitle.trim());
      setNewTitle('');
      await bundles.reload();
      if (result.already_existed) {
        // Surface both flags §4.2 calls for — the create/repair convergence point.
        window.alert(`"${result.bundle.title}" already existed; nothing was overwritten.`);
      }
    } finally {
      setCreating(false);
    }
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-semibold text-ink">Bundles</h1>
          {context && (
            <p className="mt-1 text-sm text-muted" title={context.root}>
              {context.bundles_root}
            </p>
          )}
        </div>
        <div className="flex flex-col items-end gap-1">
          <div className="flex gap-2">
            <Input
              type="text"
              value={rootDraft}
              onChange={(e) => setRootDraft(e.target.value)}
              placeholder="Switch workbench root…"
              className="w-64"
            />
            <Button variant="secondary" disabled={switching || !rootDraft.trim()} onClick={() => void handleSwitchTyped()}>
              Switch
            </Button>
            <Button variant="secondary" disabled={switching} onClick={() => void handleBrowse()}>
              Browse…
            </Button>
          </div>
          {switchError && <p className="text-xs text-bad">{switchError}</p>}
        </div>
      </div>

      <div className="flex items-center justify-between gap-2">
        <div className="flex gap-2">
          <Input
            type="text"
            value={newTitle}
            onChange={(e) => setNewTitle(e.target.value)}
            placeholder="New bundle title…"
            className="w-64"
          />
          <Button variant="primary" disabled={creating || !newTitle.trim()} onClick={() => void handleCreate()}>
            {creating ? 'Creating…' : 'New bundle'}
          </Button>
        </div>
        <div className="flex items-center gap-3">
          {bundles.data && bundles.data.length > 1 && (
            <label className="flex items-center gap-2 text-xs text-muted">
              Sort
              <Select value={sortMode} onChange={(e) => setSortMode(e.target.value as SortMode)} className="w-44">
                {(Object.keys(SORT_LABELS) as SortMode[]).map((mode) => (
                  <option key={mode} value={mode}>
                    {SORT_LABELS[mode]}
                  </option>
                ))}
              </Select>
              <button
                type="button"
                onClick={toggleSortDirection}
                title={sortDirection === 'asc' ? 'Ascending — click for descending' : 'Descending — click for ascending'}
                aria-label={sortDirection === 'asc' ? 'Sort ascending' : 'Sort descending'}
                className="rounded-lg border border-line bg-panel px-2 py-1 text-ink hover:bg-panel-muted"
              >
                {sortDirection === 'asc' ? '↑' : '↓'}
              </button>
            </label>
          )}
          <div className="flex items-center gap-1 rounded-lg border border-line bg-panel p-0.5 text-xs">
            <button
              type="button"
              onClick={() => setViewMode('grid')}
              aria-pressed={viewMode === 'grid'}
              title="Grid view"
              className={`rounded px-2 py-1 ${viewMode === 'grid' ? 'bg-accent-soft text-accent' : 'text-muted hover:text-ink'}`}
            >
              Grid
            </button>
            <button
              type="button"
              onClick={() => setViewMode('list')}
              aria-pressed={viewMode === 'list'}
              title="List view"
              className={`rounded px-2 py-1 ${viewMode === 'list' ? 'bg-accent-soft text-accent' : 'text-muted hover:text-ink'}`}
            >
              List
            </button>
          </div>
        </div>
      </div>

      {bundles.loading && <LoadingBlock label="Loading bundles…" />}
      {bundles.error && <ErrorBanner message="Could not load bundles" detail={bundles.error} />}
      {bundles.data && bundles.data.length === 0 && <EmptyState>No bundles found in this workbench root.</EmptyState>}
      {deleteError && <ErrorBanner message="Could not delete bundle" detail={deleteError} />}

      {bundles.data && bundles.data.length > 0 && (
        <ul className={viewMode === 'grid' ? 'grid grid-cols-2 gap-3' : 'flex flex-col gap-2'}>
          {sortedBundles.map((card) => (
            <li
              key={card.key}
              className={`rounded-lg border bg-panel p-4 ${card.load_error ? 'border-bad/40' : 'border-line'}`}
            >
              <div className="flex items-start justify-between">
                <div>
                  <p className="font-medium text-ink">{card.title}</p>
                  <p className="text-xs text-muted">{card.key}</p>
                </div>
                <div className="flex items-center gap-1">
                  {!card.load_error && (
                    <Button variant="primary" onClick={() => void handleSelect(card.key)} className="!px-3 !py-1 !text-xs">
                      Open
                    </Button>
                  )}
                  <Button
                    variant="danger"
                    onClick={() => setPendingDelete(card)}
                    className="!px-2 !py-1 !text-xs"
                    aria-label={`Delete ${card.title}`}
                  >
                    Delete
                  </Button>
                </div>
              </div>
              {card.load_error ? (
                <div className="mt-3 rounded-lg bg-bad-soft px-3 py-2 text-xs text-bad">
                  <p className="font-medium">Failed to load</p>
                  <p className="mt-1">{card.load_error}</p>
                </div>
              ) : (
                <dl className="mt-3 flex gap-4 text-xs text-muted">
                  <div>
                    <dt className="inline">Sources: </dt>
                    <dd className="inline font-medium text-ink">{card.source_count}</dd>
                  </div>
                  <div>
                    <dt className="inline">Knowledge items: </dt>
                    <dd className="inline font-medium text-ink">{card.knowledge_item_count}</dd>
                  </div>
                  <div>
                    <dt className="inline">Status: </dt>
                    <dd className="inline font-medium text-ink">{card.status}</dd>
                  </div>
                </dl>
              )}
            </li>
          ))}
        </ul>
      )}

      {pendingDelete && (
        <ConfirmDialog
          title={`Delete "${pendingDelete.title}"?`}
          message="This permanently deletes the bundle and everything in it — sources, knowledge items, reviews, exports. This cannot be undone."
          confirmLabel={deleting ? 'Deleting…' : 'Delete permanently'}
          confirmText={pendingDelete.title}
          disabled={deleting}
          danger
          onConfirm={() => void handleConfirmDelete()}
          onCancel={() => setPendingDelete(null)}
        />
      )}
    </div>
  );
}
