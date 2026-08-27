import { useEffect, useRef, useState } from 'react';
import * as api from '../lib/api';
import { useAsync } from '../lib/hooks';
import { USE_HTTP_BACKEND } from '../lib/runtime';
import { useIngestRun } from '../context/IngestRunContext';
import { ErrorBanner, LoadingBlock } from '../components/common/Feedback';
import { Button } from '../components/common/Button';
import { Select } from '../components/common/Input';
import { providerLabel } from '../lib/providerLabel';
import type { IngestPreviewResult, IngestProgressPayload, IngestResult, IngestRunStatus, IngestSource, MineProvider } from '../types/commands';

const FIXTURE_OPTION = 'fixture';
const AZURE_LLM_PREFIX = 'azure-llm:';

// 'export' dropped from this round's tracked steps (round 6): OKF-only now,
// no format left for a dedicated Export step to produce -- see
// AppShell.tsx's own doc comment on why the Export nav item is gone too.
const STEP_ORDER = ['scan', 'normalize', 'mine', 'validate', 'sync'] as const;
const STEP_LABELS: Record<(typeof STEP_ORDER)[number], string> = {
  scan: 'Scan',
  normalize: 'Normalize',
  mine: 'Mine',
  validate: 'Validate',
  sync: 'Sync OKF documents',
};

/**
 * Last screen built per PORT_PLAN §6.11's own staging order — the most
 * operationally dangerous one. Several destructive behaviors the plan
 * calls out are surfaced as warnings here rather than silently reproduced
 * with no UI signal: `scan` wipes originals before validating the target
 * is even a bundle and destroys accumulated normalization warnings;
 * `normalize` wipes normalized text first; re-mining rewrites every item
 * id, turning every stored review into a hard validation error.
 */
export function IngestScreen() {
  const staged = useAsync(() => api.get_staged_sources(), []);
  const settings = useAsync(() => api.get_settings(), []);
  const lastRun = useAsync(() => api.get_last_ingest_run(), []);

  const [scan, setScan] = useState(true);
  const [normalize, setNormalize] = useState(true);
  const [mine, setMine] = useState(true);
  const [validate, setValidate] = useState(true);
  const [mineProvider, setMineProvider] = useState<MineProvider>('fixture');
  // Which of possibly SEVERAL configured azure-llm profiles to use --
  // `undefined` when `mineProvider` is `'fixture'`, or before any profile has
  // been selected. Previously this screen only ever offered the single
  // default profile (`get_settings().default_profile_id`); a user with more
  // than one saved profile had no way to pick a non-default one from here.
  const [profileId, setProfileId] = useState<string | undefined>(undefined);
  const [confirmed, setConfirmed] = useState(false);

  // Lifted to `IngestRunContext` (mounted at the app root, see App.tsx) so a
  // real in-progress run survives navigating away from this screen and back --
  // see that context's own doc comment for why this used to be local state here.
  const { running, cancelling, progressLog, result, error, runPipeline, cancel } = useIngestRun();

  const [previewing, setPreviewing] = useState(false);
  const [preview, setPreview] = useState<IngestPreviewResult | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);

  // A picked folder is a distinct ingest source from staged files (§4.3
  // `IngestSource`) — it bypasses staging entirely and reads directly from
  // disk (docs/port/DECISIONS.md Q8), rather than being a different way to
  // populate the staging directory. `pick_source_folder` (Tauri command,
  // desktop-only per `dialogs.rs`) was already wired end-to-end through
  // `api.ts`/`mock/api.mock.ts`/the `IngestSource` type before this, but
  // this screen never called it — folder selection was unreachable from the
  // UI even though the backend fully supported it. When set, this takes
  // precedence over staged files for both Preview and Run.
  const [folderPath, setFolderPath] = useState<string | null>(null);
  const ingestSource: IngestSource = folderPath ? { kind: 'folder', path: folderPath } : { kind: 'staged' };

  // Settings holds at most one usable LLM profile at a time (the default
  // profile) — so "azure-llm" is offered here only when one is actually
  // configured, instead of always listing it regardless of whether it would
  // work.
  const defaultProfile = settings.data?.profiles.find((p) => p.id === settings.data?.default_profile_id) ?? null;

  // Real oss-launch's own Ingest form (tools/sopkb/sopkb/web_app.py's
  // render_ingest, the "Mining provider" <select>) has azure-llm marked
  // `selected` unconditionally — it does NOT default to fixture. `mineProvider`
  // can't start there directly: settings load asynchronously, so at mount time
  // it isn't yet known whether a usable profile even exists (offering azure-llm
  // with no profile configured would just be a dangling, doomed-to-fail
  // selection). Once settings resolve, switch to azure-llm (and its default
  // profile) to match oss-launch's real default — but only the first time, and
  // only if a profile is actually usable, so this never overrides a choice the
  // user has already made.
  const appliedProviderDefaultRef = useRef(false);
  useEffect(() => {
    if (appliedProviderDefaultRef.current || settings.loading) return;
    appliedProviderDefaultRef.current = true;
    if (defaultProfile) {
      setMineProvider('azure-llm');
      setProfileId(defaultProfile.id);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settings.loading, defaultProfile]);

  useEffect(() => {
    // If the selected profile disappears (deleted, or settings hadn't loaded
    // yet when "azure-llm" was selected), fall back to the always-available
    // fixture provider rather than leaving a dangling selection the backend
    // would reject.
    if (mineProvider === 'azure-llm' && settings.data && !settings.data.profiles.some((p) => p.id === profileId)) {
      setMineProvider('fixture');
      setProfileId(undefined);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settings.data, mineProvider, profileId]);

  // Mirrors the Python original's `step_checked(step)` (ui/sopkb-web-redesign
  // branch, tools/sopkb/sopkb/web_app.py lines 970-971): a step's checkbox
  // defaults to UNCHECKED only when its last recorded status was exactly
  // "done" -- "error"/"pending"/no prior run at all leave the existing
  // default (checked) alone, since there's nothing finished to skip. Applied
  // once on mount only -- the user can always re-check a step manually
  // afterward to force a re-run; this is a default, not a lock.
  const appliedResumeDefaultsRef = useRef(false);
  useEffect(() => {
    if (appliedResumeDefaultsRef.current || lastRun.loading) return;
    appliedResumeDefaultsRef.current = true;
    const statusByStep = lastRun.data?.status ?? {};
    if (statusByStep.scan === 'done') setScan(false);
    if (statusByStep.normalize === 'done') setNormalize(false);
    if (statusByStep.mine === 'done') setMine(false);
    if (statusByStep.validate === 'done') setValidate(false);
  }, [lastRun.loading, lastRun.data]);

  const lastRunStatusByStep = lastRun.data?.status ?? {};
  const anyStepPreUnchecked = ['scan', 'normalize', 'mine', 'validate'].some((step) => lastRunStatusByStep[step] === 'done');

  async function handlePickFiles() {
    const paths = await api.pick_source_files();
    if (paths.length > 0) {
      await api.stage_source_files(paths, 'files', false);
      await staged.reload();
    }
  }

  /** Web-mode replacement for `handlePickFiles` -- see `upload_source_files`'s own doc comment. */
  async function handleUploadFiles(fileList: FileList | null) {
    if (!fileList || fileList.length === 0) return;
    await api.upload_source_files(Array.from(fileList));
    await staged.reload();
  }

  async function handleClearStaged() {
    await api.clear_staged_sources();
    await staged.reload();
  }

  async function handleRemoveStagedFile(relativePath: string) {
    await api.remove_staged_source(relativePath);
    await staged.reload();
  }

  async function handlePickFolder() {
    const path = await api.pick_source_folder();
    if (path) setFolderPath(path);
  }

  /**
   * Read-only dry run — separate from Run, and NOT gated on the confirm
   * checkbox below (nothing destructive happens here). Shows what a real
   * run would touch before the user commits to one.
   */
  async function handlePreview() {
    setPreviewing(true);
    setPreviewError(null);
    try {
      setPreview(await api.preview_ingest_pipeline(ingestSource));
    } catch (err) {
      setPreviewError(err instanceof Error ? err.message : String(err));
    } finally {
      setPreviewing(false);
    }
  }

  async function handleRun() {
    if (!confirmed) return;
    await runPipeline({
      source: ingestSource,
      scan,
      normalize,
      mine,
      validate,
      export: false,
      mine_provider: mineProvider,
      profile_id: mineProvider === 'azure-llm' ? profileId : undefined,
    });
    void lastRun.reload();
  }

  return (
    <div className="space-y-6">
      <h1 className="text-xl font-semibold text-ink">Ingest</h1>

      {!lastRun.loading && lastRun.data && !lastRun.data.ok && (
        <div className="space-y-1">
          <ErrorBanner
            message={
              failedIngestStep(lastRun.data)
                ? `Ingest failed at step: ${STEP_LABELS[failedIngestStep(lastRun.data)!]}`
                : 'The last ingest run did not complete successfully.'
            }
            detail={failedIngestStep(lastRun.data) ? lastRun.data.detail[failedIngestStep(lastRun.data)!] : undefined}
          />
          <p className="text-xs text-muted">
            Steps already completed are pre-unchecked below — just re-run the rest. Recheck everything if you want a full re-run.
          </p>
        </div>
      )}

      {/*
        The banner above only fires for a FAILED previous run -- but the exact
        same per-step "done" pre-unchecking also happens after a fully
        SUCCESSFUL previous run (Python's own `step_checked` has no concept of
        "successful but on stale/degenerate input"). Real case that surfaced
        this: a source that used to collapse into one degenerate section (a
        bug, since fixed) ran the whole pipeline "successfully", including
        Mine against that garbage input. Fixing the upstream bug and
        re-running produces the correct sections, but Mine's stale "done"
        silently leaves it unchecked with no failed-run banner to explain why
        -- "the pipeline succeeded" while quietly mining nothing. This is
        purely a visibility addition: it does not change which steps default
        to checked/unchecked, only makes an otherwise-silent default visible.
      */}
      {!lastRun.loading && lastRun.data?.ok && anyStepPreUnchecked && (
        <p className="rounded-lg border border-line bg-panel-muted px-3 py-2 text-xs text-muted">
          The last run completed successfully, so some steps below are pre-unchecked (see "skipped" notes) and will reuse
          that prior result. If anything upstream of a skipped step changed since then, recheck it before running.
        </p>
      )}

      <section className="rounded-lg border border-line bg-panel p-4">
        <h2 className="text-sm font-semibold text-ink">Pipeline steps</h2>
        <div className="mt-2 grid grid-cols-2 gap-2 text-sm">
          <StepToggle
            label="Scan (wipes originals first — destroys stale_after/warnings)"
            checked={scan}
            onChange={setScan}
            skippedHint={lastRunStatusByStep.scan === 'done'}
          />
          <StepToggle
            label="Normalize (wipes normalized text first)"
            checked={normalize}
            onChange={setNormalize}
            skippedHint={lastRunStatusByStep.normalize === 'done'}
          />
          <StepToggle
            label="Mine (rewrites items.json — invalidates existing reviews)"
            checked={mine}
            onChange={setMine}
            skippedHint={lastRunStatusByStep.mine === 'done'}
          />
          <StepToggle label="Validate" checked={validate} onChange={setValidate} skippedHint={lastRunStatusByStep.validate === 'done'} />
        </div>
        <label className="mt-3 flex items-center gap-2 text-sm text-ink">
          Mining provider
          <Select
            value={mineProvider === 'azure-llm' ? `${AZURE_LLM_PREFIX}${profileId ?? ''}` : FIXTURE_OPTION}
            onChange={(e) => {
              const value = e.target.value;
              if (value === FIXTURE_OPTION) {
                setMineProvider('fixture');
                setProfileId(undefined);
              } else {
                setMineProvider('azure-llm');
                setProfileId(value.slice(AZURE_LLM_PREFIX.length));
              }
            }}
            className="!py-1"
          >
            <option value={FIXTURE_OPTION}>{providerLabel('fixture')}</option>
            {settings.data?.profiles.map((p) => (
              <option key={p.id} value={`${AZURE_LLM_PREFIX}${p.id}`}>
                {providerLabel('azure-llm', p.name)}
              </option>
            ))}
          </Select>
        </label>
        {(settings.data?.profiles.length ?? 0) === 0 && !settings.loading && (
          <p className="mt-1 text-xs text-muted-soft">
            No LLM provider configured yet — set one up in Settings to enable LLM-based mining.
          </p>
        )}

        {/*
         * Folder selection silently overrides staged files (see `ingestSource`
         * above and the "Reading directly from folder" note in the Source
         * section) -- a user who picked a folder once, then later cleared
         * staged files and picked just one new file expecting THAT file to be
         * what runs, gets every file in the still-selected folder instead,
         * with the only explanation being a note in a DIFFERENT section above
         * that's easy to have scrolled past. This makes the actually-effective
         * source unmissable right where the decision to run gets made,
         * regardless of which section the user was last looking at.
         */}
        <p className="mt-4 rounded-lg border border-line bg-panel-muted px-3 py-2 text-sm text-ink">
          This run will scan:{' '}
          {folderPath ? (
            <>
              every file in <span className="font-medium">{folderPath}</span>
            </>
          ) : staged.data ? (
            <>
              <span className="font-medium">{staged.data.file_count}</span> staged file(s)
            </>
          ) : (
            <span className="text-muted">no staged files — nothing to scan yet</span>
          )}
        </p>

        {/*
         * Ingest is destructive (scan/normalize wipe originals/normalized text
         * first, mine rewrites items.json and invalidates existing reviews —
         * see the StepToggle warnings above), so Run stays disabled until the
         * user explicitly confirms, separate from "at least one step is
         * selected". Mirrors oss-launch's IngestPage `confirmBundleUpdate`
         * checkbox.
         */}
        <label className="mt-2 flex items-start gap-2 rounded-lg border border-line bg-panel-muted px-3 py-2.5 text-sm">
          <input
            type="checkbox"
            checked={confirmed}
            onChange={(e) => setConfirmed(e.target.checked)}
            className="mt-0.5 accent-accent"
          />
          <span className="text-ink">Confirm these files/folder should update this bundle</span>
        </label>

        {error && <p className="mt-2 text-sm text-bad">{error}</p>}
        {previewError && <p className="mt-2 text-sm text-bad">{previewError}</p>}

        <div className="mt-3 flex gap-2">
          <Button variant="secondary" disabled={previewing} onClick={() => void handlePreview()}>
            {previewing ? 'Previewing…' : 'Preview source changes'}
          </Button>
          <Button
            variant="primary"
            disabled={running || !confirmed || !(scan || normalize || mine || validate)}
            onClick={() => void handleRun()}
            title={!confirmed ? 'Confirm the update above before running the pipeline' : undefined}
          >
            {running ? 'Running…' : 'Run pipeline'}
          </Button>
          {/*
           * Cancellation is cooperative, not instant (see IngestRunContext's
           * `cancel` doc comment) -- "Cancelling…" stays shown, disabled, until
           * the run actually settles (`running` flips false), rather than
           * pretending the stop already happened.
           */}
          {running && (
            <Button variant="ghost" disabled={cancelling} onClick={() => void cancel()} className="!text-bad decoration-bad/40">
              {cancelling ? 'Cancelling…' : 'Cancel'}
            </Button>
          )}
        </div>
      </section>

      <section className="rounded-lg border border-line bg-panel p-4">
        <h2 className="text-sm font-semibold text-ink">Source</h2>

        {folderPath ? (
          <p className="mt-1 text-sm text-muted">
            Reading directly from folder <span className="text-ink">{folderPath}</span> — staged files below are ignored while a
            folder source is selected.
          </p>
        ) : staged.loading ? (
          <LoadingBlock label="Checking staged sources…" />
        ) : staged.data ? (
          <>
            <p className="mt-1 text-sm text-muted">
              {staged.data.file_count} file(s) staged in {staged.data.staging_dir}
              {staged.data.skipped.length > 0 && ` (${staged.data.skipped.length} skipped)`}
            </p>
            <ul className="mt-2 max-h-[240px] space-y-1 overflow-y-auto rounded-lg border border-line-soft">
              {staged.data.files.map((file) => (
                <li key={file} className="flex items-center justify-between gap-2 border-b border-line-soft px-3 py-1.5 text-sm last:border-0">
                  <span className="truncate text-ink/80" title={file}>
                    {file}
                  </span>
                  <button
                    type="button"
                    onClick={() => void handleRemoveStagedFile(file)}
                    className="shrink-0 text-xs text-bad underline decoration-bad/40"
                  >
                    Remove
                  </button>
                </li>
              ))}
            </ul>
          </>
        ) : (
          <p className="mt-1 text-sm text-muted">No files staged.</p>
        )}

        <div className="mt-2 flex flex-wrap items-center gap-2">
          {USE_HTTP_BACKEND ? (
            // No native file dialog in a browser -- a real <input type="file"> upload
            // instead of "Pick files…"/"Pick folder…", which have nothing to call here
            // (see `upload_source_files`'s own doc comment).
            <label className="cursor-pointer">
              <span className="inline-block rounded-md border border-line bg-panel px-3 py-1.5 text-sm text-ink hover:bg-panel-muted">
                Upload files…
              </span>
              <input type="file" multiple disabled={!!folderPath} className="hidden" onChange={(e) => void handleUploadFiles(e.target.files)} />
            </label>
          ) : (
            <>
              <Button variant="secondary" disabled={!!folderPath} onClick={() => void handlePickFiles()}>
                Pick files…
              </Button>
              <Button variant="secondary" onClick={() => void handlePickFolder()}>
                Pick folder…
              </Button>
            </>
          )}
          <Button variant="secondary" disabled={!!folderPath} onClick={() => void handleClearStaged()}>
            Clear staged
          </Button>
          {folderPath && (
            <Button variant="ghost" onClick={() => setFolderPath(null)} className="!text-bad decoration-bad/40">
              Clear folder
            </Button>
          )}
        </div>
        {/* Filters the file picker's OS dialog already, but a folder pick can't be
            filtered by contained file type (an OS folder browser has no concept of
            "only show folders containing X") -- stated up front here so it's known
            before picking, not just discoverable after running Preview. */}
        <p className="mt-1.5 text-xs text-muted-soft">Supports .md, .txt, .docx, .pdf</p>
      </section>

      {preview && <IngestPreviewTable preview={preview} />}

      {(running || progressLog.length > 0) && (
        <IngestProgress
          progressLog={progressLog}
          expectedSteps={STEP_ORDER.filter((step) => {
            if (step === 'sync') return scan || normalize || mine || validate;
            return { scan, normalize, mine, validate }[step];
          })}
        />
      )}

      {result && (
        <section className="rounded-lg border border-line bg-panel p-4 text-sm">
          <h2 className="text-sm font-semibold text-ink">Result</h2>
          <IngestResultStats result={result} />
        </section>
      )}
    </div>
  );
}

const PREVIEW_CLASSIFICATION_CLASSES: Record<string, string> = {
  new: 'bg-ok-soft text-ok',
  updated: 'bg-warn-soft text-warn',
  unchanged: 'bg-panel-muted text-muted',
  unsupported: 'bg-bad-soft text-bad',
};

/**
 * Read-only preview of what `Run pipeline`'s scan step would do — path,
 * classification, and (when known) the source id it maps to — shown before
 * committing to a destructive run. Mirrors oss-launch's IngestPage "Pending
 * source changes" table, minus the `version_number` column: jt-dev's Rust
 * engine has no source-versioning concept yet (see the Rust
 * `classify_source_updates` doc comment), so there's no version number to
 * show.
 */
function IngestPreviewTable({ preview }: { preview: IngestPreviewResult }) {
  return (
    <section className="rounded-lg border border-line bg-panel p-4 text-sm">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold text-ink">Pending source changes</h2>
        <span className="text-xs text-muted">{preview.source_dir}</span>
      </div>
      {preview.files.length === 0 ? (
        <p className="mt-2 text-sm text-muted">No pending changes detected for this source folder.</p>
      ) : (
        <table className="mt-3 w-full overflow-hidden rounded-lg border border-line text-sm">
          <thead className="bg-panel-muted text-left text-xs uppercase tracking-wide text-muted">
            <tr>
              <th className="px-3 py-2">Path</th>
              <th className="px-3 py-2">Classification</th>
              <th className="px-3 py-2">Source</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-line-soft">
            {preview.files.map((file, i) => (
              <tr key={`${file.path}-${i}`}>
                <td className="px-3 py-2 text-ink/80">{file.path}</td>
                <td className="px-3 py-2">
                  <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${PREVIEW_CLASSIFICATION_CLASSES[file.classification] ?? 'bg-panel-muted text-muted'}`}>
                    {file.classification}
                  </span>
                </td>
                <td className="px-3 py-2 text-muted">{file.source_id ?? ''}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}

/**
 * Every field on `IngestResult` is optional — only meaningful when the
 * corresponding pipeline step actually ran (§4.3). A scannable stat-tile
 * layout replaces the raw `JSON.stringify(result, null, 2)` dump that used
 * to be here: the data was already fully typed, this is presentation only.
 *
 * `!= null` (not `!== undefined`), deliberately: the Rust side's `Option<T>`
 * fields serialize to a present JSON key with value `null` for a skipped
 * step, not an omitted key, so a skipped step's field arrives as `null`, not
 * `undefined`, despite the `field?: T` TS type implying only the latter. An
 * `!== undefined` check let a skipped `exports` step's `null` through to
 * `result.exports.length`, crashing every real run that didn't check
 * "Export" — invisible until now because every earlier run in this session
 * ended in a caught mining error before a result ever reached this
 * component.
 */
function IngestResultStats({ result }: { result: IngestResult }) {
  const tiles: { label: string; value: string }[] = [];
  if (result.uploaded_files != null) tiles.push({ label: 'Files uploaded', value: String(result.uploaded_files) });
  if (result.sources != null) tiles.push({ label: 'Sources scanned', value: String(result.sources) });
  if (result.sections != null) tiles.push({ label: 'Sections normalized', value: String(result.sections) });
  if (result.items != null) {
    tiles.push({ label: `Items mined${result.mine_provider ? ` (${result.mine_provider})` : ''}`, value: String(result.items) });
  }
  if (result.validation != null) {
    tiles.push({ label: 'Validation errors', value: String(result.validation.errors) });
    tiles.push({ label: 'Validation warnings', value: String(result.validation.warnings) });
  }
  if (result.exports != null) tiles.push({ label: 'Export outputs', value: String(result.exports.length) });

  if (tiles.length === 0) {
    return <p className="mt-2 text-sm text-muted">No pipeline steps ran.</p>;
  }

  return (
    <dl className="mt-3 grid grid-cols-2 gap-3 sm:grid-cols-3">
      {tiles.map((tile) => (
        <div key={tile.label} className="rounded-lg border border-line bg-panel-muted px-3 py-2.5">
          <dt className="text-xs text-muted">{tile.label}</dt>
          <dd className="mt-0.5 text-lg font-semibold text-ink">{tile.value}</dd>
        </div>
      ))}
    </dl>
  );
}

/**
 * The first of the 5 checkbox-backed steps whose last recorded status was
 * `"error"`, in pipeline order -- mirrors the Python original's resume
 * banner, which names the first failed step. `undefined` covers both "no
 * step actually errored" (e.g. the Rust-only derived "sync" step failed
 * instead, an edge case outside the 5 tracked keys) and "no prior run".
 */
function failedIngestStep(lastRun: IngestRunStatus): (typeof STEP_ORDER)[number] | undefined {
  return STEP_ORDER.find((step) => lastRun.status[step] === 'error');
}

function StepToggle({
  label,
  checked,
  onChange,
  skippedHint,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
  /**
   * Shown when this box defaulted to unchecked because the last recorded run
   * marked this exact step "done" (`step_checked`'s own real oss-launch
   * semantics -- see the resume-defaults effect above). That default is
   * per-step, with no awareness of whether an EARLIER step is about to
   * re-run and change this step's input -- e.g. re-running Normalize after
   * fixing a section-extraction bug does not un-stick Mine's stale "done"
   * default, so a "successful" re-run can silently mine nothing. The
   * underlying default logic matches Python's `step_checked` exactly
   * (unchanged here); this hint only makes an otherwise silent default
   * visible at the exact control it affects, so it's never a surprise.
   */
  skippedHint?: boolean;
}) {
  return (
    <label className="flex items-start gap-2">
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} className="mt-0.5 accent-accent" />
      <span className="text-ink/80">
        {label}
        {skippedHint && !checked && (
          <span className="ml-1.5 text-xs text-muted-soft" title="Unchecked because a previous run already completed this step -- recheck it if anything upstream changed since then">
            (skipped — completed previously)
          </span>
        )}
      </span>
    </label>
  );
}

/**
 * The backend emits two log lines per step (`started` then `done`/`failed`) so a
 * 5-step run produces ~10 near-duplicate entries with no sense of overall progress.
 * This collapses that raw log to one row per expected step, keyed by the *latest*
 * event for that step name, plus a completed-count summary — so "what's happening"
 * is answerable at a glance instead of by reading a scrolling transcript.
 */
function IngestProgress({
  progressLog,
  expectedSteps,
}: {
  progressLog: IngestProgressPayload[];
  expectedSteps: readonly (typeof STEP_ORDER)[number][];
}) {
  const latestByStep = new Map<string, IngestProgressPayload>();
  for (const p of progressLog) latestByStep.set(p.step, p);

  const doneCount = expectedSteps.filter((step) => latestByStep.get(step)?.status === 'done').length;
  const hasFailed = expectedSteps.some((step) => latestByStep.get(step)?.status === 'failed');
  const hasCancelled = !hasFailed && expectedSteps.some((step) => latestByStep.get(step)?.status === 'cancelled');
  const isComplete = !hasFailed && !hasCancelled && doneCount === expectedSteps.length && expectedSteps.length > 0;
  const pct = expectedSteps.length > 0 ? Math.round((doneCount / expectedSteps.length) * 100) : 0;

  return (
    <section>
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold text-ink">Progress</h2>
        <span className="text-xs text-muted">
          {hasFailed ? 'Failed' : hasCancelled ? 'Cancelled' : isComplete ? 'Complete' : `${doneCount} / ${expectedSteps.length} steps`}
        </span>
      </div>
      <div className="mt-1.5 h-1.5 w-full overflow-hidden rounded-full bg-line">
        <div
          className={`h-full rounded-full transition-all ${hasFailed ? 'bg-bad' : hasCancelled ? 'bg-warn' : 'bg-ok'}`}
          style={{ width: `${pct}%` }}
        />
      </div>
      <ul className="mt-3 space-y-1.5 text-sm">
        {expectedSteps.map((step) => {
          const latest = latestByStep.get(step);
          const status = latest?.status ?? 'pending';
          return (
            <li key={step} className="flex items-center gap-2">
              <span
                className={`h-2 w-2 shrink-0 rounded-full ${
                  status === 'done'
                    ? 'bg-ok'
                    : status === 'failed'
                      ? 'bg-bad'
                      : status === 'cancelled'
                        ? 'bg-warn'
                        : status === 'started' || status === 'progress'
                          ? 'animate-pulse bg-warn'
                          : 'bg-line'
                }`}
              />
              <span className="w-44 shrink-0 font-medium text-ink">{STEP_LABELS[step]}</span>
              <span className="text-muted">
                {status === 'pending' && 'Waiting…'}
                {status === 'started' && 'Running…'}
                {status === 'progress' && (latest?.detail || 'Running…')}
                {status === 'cancelled' && 'Cancelled — never started'}
                {(status === 'done' || status === 'failed') && (latest?.detail || (status === 'done' ? 'Done' : 'Failed'))}
              </span>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
