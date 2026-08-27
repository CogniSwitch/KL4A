import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { IngestScreen } from './IngestScreen';
import { WorkbenchProvider } from '../context/WorkbenchContext';
import { IngestRunProvider } from '../context/IngestRunContext';
import { __resetStoreForTests, store } from '../mock/store';
import * as api from '../lib/api';
import { publish } from '../lib/events';

function renderIngest() {
  return render(
    <MemoryRouter>
      <WorkbenchProvider>
        <IngestRunProvider>
          <IngestScreen />
        </IngestRunProvider>
      </WorkbenchProvider>
    </MemoryRouter>,
  );
}

/**
 * Ingest is destructive (scan/normalize wipe originals/normalized text,
 * mine invalidates existing reviews), so "Run pipeline" must stay disabled
 * until the user has both selected at least one step AND ticked the
 * explicit confirmation checkbox — checking the box alone is not enough if
 * every step toggle got unchecked, and vice versa.
 */
describe('IngestScreen confirm-before-run gating', () => {
  beforeEach(() => {
    __resetStoreForTests();
    store.selectedBundle = 'glp1-healthcare';
  });

  it('disables Run until the confirmation checkbox is checked, even with steps selected', async () => {
    renderIngest();
    const runButton = await screen.findByRole('button', { name: 'Run pipeline' });
    expect(runButton).toBeDisabled();

    const confirmCheckbox = screen.getByRole('checkbox', { name: /confirm these files\/folder should update this bundle/i });
    await userEvent.click(confirmCheckbox);
    expect(runButton).toBeEnabled();
  });

  it('keeps Run disabled when confirmed but every pipeline step is unchecked', async () => {
    renderIngest();
    const runButton = await screen.findByRole('button', { name: 'Run pipeline' });
    const confirmCheckbox = screen.getByRole('checkbox', { name: /confirm these files\/folder should update this bundle/i });
    await userEvent.click(confirmCheckbox);
    expect(runButton).toBeEnabled();

    for (const label of [/^Scan/, /^Normalize/, /^Mine/, /^Validate/]) {
      await userEvent.click(screen.getByRole('checkbox', { name: label }));
    }
    expect(runButton).toBeDisabled();
  });
});

/**
 * The result panel used to be a raw `JSON.stringify(result, null, 2)` dump.
 * This confirms it now renders labeled stat tiles from the already-typed
 * `IngestResult` fields instead.
 */
describe('IngestScreen result stat tiles', () => {
  beforeEach(() => {
    __resetStoreForTests();
    store.selectedBundle = 'glp1-healthcare';
  });

  it('renders labeled stat tiles (not a JSON dump) after a successful run', async () => {
    renderIngest();
    const runButton = await screen.findByRole('button', { name: 'Run pipeline' });
    await userEvent.click(screen.getByRole('checkbox', { name: /confirm these files\/folder should update this bundle/i }));
    expect(runButton).toBeEnabled();
    await userEvent.click(runButton);

    expect(await screen.findByText('Files uploaded', {}, { timeout: 5000 })).toBeInTheDocument();
    expect(screen.getByText('Sources scanned')).toBeInTheDocument();
    expect(screen.getByText('Sections normalized')).toBeInTheDocument();
    expect(screen.getByText(/^Items mined/)).toBeInTheDocument();
    expect(screen.getByText('Validation errors')).toBeInTheDocument();
    expect(screen.getByText('Validation warnings')).toBeInTheDocument();
    // The old raw-JSON rendering is gone.
    expect(screen.queryByText(/"uploaded_files"/)).not.toBeInTheDocument();
  });
});

/**
 * The mining-provider dropdown used to hardcode `fixture`/`azure-llm`
 * regardless of whether an LLM profile was actually configured in
 * Settings, letting a user pick a provider that would just fail. It's now
 * driven off `get_settings()`'s default profile.
 */
describe('IngestScreen mining provider reflects Settings', () => {
  beforeEach(() => {
    __resetStoreForTests();
    store.selectedBundle = 'glp1-healthcare';
  });

  it('offers azure-llm labeled with the configured default profile name', async () => {
    renderIngest();
    // The select renders immediately with just "fixture" while get_settings()
    // is still loading; wait for the profile-derived option to actually appear.
    await screen.findByText('azure-llm — Azure OpenAI (default)', {}, { timeout: 3000 });
    expect(screen.queryByText(/no llm provider configured yet/i)).not.toBeInTheDocument();
  });

  it('omits azure-llm and shows a hint when no profile is configured', async () => {
    store.settingsProfiles = [];
    store.defaultProfileId = undefined;
    renderIngest();
    const select = (await screen.findByRole('combobox', { name: /mining provider/i })) as HTMLSelectElement;
    await screen.findByText(/no llm provider configured yet/i);
    const optionLabels = Array.from(select.options).map((o) => o.textContent);
    expect(optionLabels).toEqual(['Offline (no network)']);
  });

  /**
   * Real oss-launch's own Ingest form defaults its "Mining provider" select to
   * azure-llm unconditionally (tools/sopkb/sopkb/web_app.py, <option
   * value="azure-llm" selected>) -- a real user hit this divergence: this
   * screen used to start on `fixture` and stay there until manually changed,
   * silently skipping heading-restructuring on every run unless the user
   * remembered to flip the dropdown first.
   */
  it('defaults the selection to azure-llm once a usable profile is known', async () => {
    renderIngest();
    const select = (await screen.findByRole('combobox', { name: /mining provider/i })) as HTMLSelectElement;
    // Starts on fixture (the only safe choice before settings resolve)...
    expect(select.value).toBe('fixture');
    // ...then switches once a configured default profile is confirmed. Waiting on
    // the option TEXT alone is not enough: it appears in the DOM as soon as
    // `settings.data.profiles` loads, which can be one render tick before the
    // effect-driven `setMineProvider`/`setProfileId` calls actually update
    // `select.value` -- wait on the real condition instead of a proxy for it.
    await waitFor(() => expect(select.value).toBe('azure-llm:profile-default'), { timeout: 3000 });
  });

  it('stays on fixture when no profile is configured, never dangling on azure-llm', async () => {
    store.settingsProfiles = [];
    store.defaultProfileId = undefined;
    renderIngest();
    const select = (await screen.findByRole('combobox', { name: /mining provider/i })) as HTMLSelectElement;
    await screen.findByText(/no llm provider configured yet/i);
    expect(select.value).toBe('fixture');
  });

  it('does not override an explicit user choice of fixture once settings resolve', async () => {
    renderIngest();
    const select = (await screen.findByRole('combobox', { name: /mining provider/i })) as HTMLSelectElement;
    // See the previous test's comment: wait on `select.value` itself, not on the
    // option text (which can appear a render tick before the value updates).
    await waitFor(() => expect(select.value).toBe('azure-llm:profile-default'), { timeout: 3000 });
    await userEvent.selectOptions(select, 'fixture');
    expect(select.value).toBe('fixture');
    // Nothing re-applies the azure-llm default after an explicit change. A
    // generous margin (not a tight race) -- under full-suite CPU contention a
    // short fixed sleep here was observed to occasionally resolve before a
    // slow render settled, which isn't the thing this assertion cares about.
    await new Promise((resolve) => setTimeout(resolve, 200));
    expect(select.value).toBe('fixture');
  });

  /**
   * A user with more than one saved profile previously had no way to pick a
   * non-default one from this screen -- the dropdown only ever offered the
   * single `default_profile_id` profile. Confirmed as a real gap by a live
   * user report ("Cannot see additional LLM profile in ingest dropdown").
   */
  it('lists every configured profile, not just the default one', async () => {
    store.settingsProfiles = [
      ...store.settingsProfiles,
      { ...store.settingsProfiles[0], id: 'profile-secondary', name: 'Secondary Profile', is_default: false },
    ];
    renderIngest();
    const select = (await screen.findByRole('combobox', { name: /mining provider/i })) as HTMLSelectElement;
    await screen.findByText('azure-llm — Azure OpenAI (default)', {}, { timeout: 3000 });
    const optionLabels = Array.from(select.options).map((o) => o.textContent);
    expect(optionLabels).toContain('azure-llm — Secondary Profile');

    await userEvent.selectOptions(select, 'azure-llm:profile-secondary');
    expect(select.value).toBe('azure-llm:profile-secondary');
  });

  it('sends the selected non-default profile_id when running the pipeline', async () => {
    store.settingsProfiles = [
      ...store.settingsProfiles,
      { ...store.settingsProfiles[0], id: 'profile-secondary', name: 'Secondary Profile', is_default: false },
    ];
    const runSpy = vi.spyOn(api, 'run_ingest_pipeline');
    renderIngest();
    const select = (await screen.findByRole('combobox', { name: /mining provider/i })) as HTMLSelectElement;
    await screen.findByText('azure-llm — Secondary Profile', {}, { timeout: 3000 });
    await userEvent.selectOptions(select, 'azure-llm:profile-secondary');

    await userEvent.click(screen.getByRole('checkbox', { name: /confirm these files\/folder should update this bundle/i }));
    await userEvent.click(screen.getByRole('button', { name: 'Run pipeline' }));

    expect(runSpy).toHaveBeenCalledWith(expect.objectContaining({ mine_provider: 'azure-llm', profile_id: 'profile-secondary' }));
  });
});

/**
 * A long-running network-bound step (mine, normalize's LLM heading-
 * restructuring) used to sit on a frozen "Running…" from the moment it
 * started until the moment it finished, with nothing in between -- a real
 * user could not tell "slow but working" apart from "genuinely hung" and
 * reported exactly that after a mining run ran for over an hour with no
 * visible movement. The backend now emits `status: 'progress'` events
 * mid-step (`emit_ingest_progress(..., "progress", "42/215 sections
 * mined")`); this confirms the UI actually surfaces them instead of only
 * reacting to `started`/`done`/`failed`.
 */
describe('IngestScreen live progress for long-running steps', () => {
  beforeEach(() => {
    __resetStoreForTests();
    store.selectedBundle = 'glp1-healthcare';
  });

  it('shows an in-flight progress detail for a running step, not a frozen "Running…"', async () => {
    renderIngest();
    // Establishes the progress panel is visible at all (it only renders once
    // `running || progressLog.length > 0`) before the real event under test.
    publish('ingest://progress', { step: 'mine', status: 'started', detail: '' });
    await screen.findByText('Running…');

    // A real mid-step progress event, exactly the shape the Rust side now
    // emits (`emit_ingest_progress(..., "progress", "42/215 sections mined")`).
    publish('ingest://progress', { step: 'mine', status: 'progress', detail: '42/215 sections mined' });
    await screen.findByText('42/215 sections mined');
    expect(screen.queryByText('Running…')).not.toBeInTheDocument();
  });
});

/**
 * `pick_source_folder` (Tauri command, desktop-only per dialogs.rs) was
 * already wired end-to-end through api.ts/mock/api.mock.ts/the
 * `IngestSource` type before this screen change, but no button ever called
 * it — folder selection was unreachable from the ingest UI even though the
 * backend fully supported it.
 */
describe('IngestScreen folder source selection', () => {
  beforeEach(() => {
    __resetStoreForTests();
    store.selectedBundle = 'glp1-healthcare';
  });

  it('shows the picked folder path and disables the staged-files actions while it is selected', async () => {
    renderIngest();
    await userEvent.click(await screen.findByRole('button', { name: 'Pick folder…' }));

    expect(await screen.findByText(/Reading directly from folder/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Pick files…' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Clear staged' })).toBeDisabled();
  });

  it('clears the folder selection and re-enables staged-files actions', async () => {
    renderIngest();
    await userEvent.click(await screen.findByRole('button', { name: 'Pick folder…' }));
    await screen.findByText(/Reading directly from folder/);

    await userEvent.click(screen.getByRole('button', { name: 'Clear folder' }));
    expect(screen.queryByText(/Reading directly from folder/)).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Pick files…' })).toBeEnabled();
  });

  // A folder selection silently overrides staged files (see `ingestSource`) --
  // a user who picks a folder once, then later clears staged files and stages
  // just one new file, gets every file in the still-selected folder on Run,
  // not the one they just staged. The only prior explanation was a note in
  // the Source section, easy to miss; this summary makes the actually-
  // effective source visible right next to the Run button too.
  it('unmistakably shows the folder, not stale staged-file state, as what a run would scan', async () => {
    renderIngest();
    await userEvent.click(await screen.findByRole('button', { name: 'Pick folder…' }));
    // "This run will scan:" is present in every state (folder, staged, or
    // neither), so it alone isn't a reliable wait condition -- wait for the
    // folder pick to actually settle first, same as the other folder tests.
    await screen.findByText(/Reading directly from folder/);

    const summary = screen.getByText(/This run will scan:/);
    expect(summary).toHaveTextContent('inbox');
  });
});

describe('IngestScreen staged-file list', () => {
  beforeEach(() => {
    __resetStoreForTests();
    store.selectedBundle = 'glp1-healthcare';
  });

  it('states the supported file types up front, before anything is picked', async () => {
    renderIngest();
    expect(await screen.findByText('Supports .md, .txt, .docx, .pdf')).toBeInTheDocument();
  });

  it('lists a staged file individually and removes just that one on click', async () => {
    renderIngest();
    await userEvent.click(await screen.findByRole('button', { name: 'Pick files…' }));
    expect(await screen.findByText('new-procedure.md')).toBeInTheDocument();
    expect(screen.getByText(/1 file\(s\) staged/)).toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: 'Remove' }));
    await waitFor(() => expect(screen.queryByText('new-procedure.md')).not.toBeInTheDocument());
    expect(await screen.findByText('No files staged.')).toBeInTheDocument();
  });
});

describe('IngestScreen source-to-run summary', () => {
  beforeEach(() => {
    __resetStoreForTests();
    store.selectedBundle = 'glp1-healthcare';
  });

  it('shows "no staged files" when nothing is staged and no folder is selected', async () => {
    renderIngest();
    expect(await screen.findByText(/no staged files — nothing to scan yet/)).toBeInTheDocument();
  });
});

/**
 * "Preview source changes" is a read-only dry run, distinct from "Run
 * pipeline" — it must work without ticking the destructive-action
 * confirmation checkbox, and must render a classification table rather
 * than mutate anything.
 */
describe('IngestScreen preview (dry run)', () => {
  beforeEach(() => {
    __resetStoreForTests();
    store.selectedBundle = 'glp1-healthcare';
  });

  it('is usable without checking the confirm-before-run checkbox', async () => {
    renderIngest();
    const previewButton = await screen.findByRole('button', { name: 'Preview source changes' });
    expect(previewButton).toBeEnabled();

    await userEvent.click(previewButton);
    expect(await screen.findByText('Pending source changes', {}, { timeout: 3000 })).toBeInTheDocument();
    // Run pipeline is still gated on confirmation — preview didn't bypass that.
    expect(screen.getByRole('button', { name: 'Run pipeline' })).toBeDisabled();
  });

  it('renders a classification row per already-ingested source as "unchanged"', async () => {
    renderIngest();
    await userEvent.click(await screen.findByRole('button', { name: 'Preview source changes' }));
    await screen.findByText('Pending source changes', {}, { timeout: 3000 });

    const unchangedChips = screen.getAllByText('unchanged');
    expect(unchangedChips.length).toBeGreaterThan(0);
  });
});

/**
 * `.sopkb/ingest_run.json` persists a run's outcome across reloads — a
 * failure banner naming the failed step, and completed steps' checkboxes
 * pre-unchecked so resubmitting only re-runs what's left.
 */
describe('IngestScreen resume-from-failure banner', () => {
  beforeEach(() => {
    __resetStoreForTests();
    store.selectedBundle = 'glp1-healthcare';
  });

  it('shows a banner naming the failed step and detail, and pre-unchecks completed steps only', async () => {
    store.lastIngestRun = {
      ok: false,
      finished_at: '2026-08-22T00:00:00Z',
      status: { scan: 'done', normalize: 'error', mine: 'pending', validate: 'pending' },
      detail: { scan: '3 sources', normalize: 'normalization failed: bad encoding' },
    };
    renderIngest();

    expect(await screen.findByText('Ingest failed at step: Normalize')).toBeInTheDocument();
    expect(screen.getByText('normalization failed: bad encoding')).toBeInTheDocument();

    // The pre-uncheck effect fires on a passive-effect pass *after* the banner
    // text commits, not in the same render — findBy/waitFor (not a bare getBy)
    // is required here so the assertion doesn't race that follow-up render.
    await screen.findByRole('checkbox', { name: /^Scan/, checked: false });
    expect(screen.getByRole('checkbox', { name: /^Normalize/ })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: /^Mine/ })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: /^Validate/ })).toBeChecked();
  });

  it('shows no banner when there is no prior run', async () => {
    renderIngest();
    await screen.findByRole('button', { name: 'Run pipeline' });
    expect(screen.queryByText(/Ingest failed at step/)).not.toBeInTheDocument();
  });

  it('shows no banner when the last run succeeded', async () => {
    store.lastIngestRun = {
      ok: true,
      finished_at: '2026-08-22T00:00:00Z',
      status: { scan: 'done', normalize: 'done', mine: 'done', validate: 'done' },
      detail: {},
    };
    renderIngest();
    await screen.findByRole('button', { name: 'Run pipeline' });
    expect(screen.queryByText(/Ingest failed at step/)).not.toBeInTheDocument();
  });

  // A fully successful prior run pre-unchecks "done" steps exactly like a
  // failed one does (Python's own `step_checked` has no "successful but on
  // stale input" concept), but had no banner explaining why -- silently
  // reused Mine's stale "done" default. Real case: a source used to collapse
  // to one degenerate section (bug, since fixed); Mine ran "successfully"
  // against that garbage; fixing Normalize and re-running left Mine
  // unchecked with zero indication, silently mining nothing on a run that
  // reported success.
  it('shows a distinct notice (not the failed-run banner) when a successful prior run left steps pre-unchecked', async () => {
    store.lastIngestRun = {
      ok: true,
      finished_at: '2026-08-22T00:00:00Z',
      status: { scan: 'done', normalize: 'done', mine: 'done', validate: 'pending' },
      detail: {},
    };
    renderIngest();

    expect(await screen.findByText(/last run completed successfully/i)).toBeInTheDocument();
    expect(screen.queryByText(/Ingest failed at step/)).not.toBeInTheDocument();

    await screen.findByRole('checkbox', { name: /^Scan/, checked: false });
    expect(screen.getByRole('checkbox', { name: /^Mine/ })).not.toBeChecked();
    // Validate never ran ("pending"), so it keeps its default-checked state and gets no hint.
    expect(screen.getByRole('checkbox', { name: /^Validate/ })).toBeChecked();
  });

  it('shows the per-checkbox "skipped" hint only on steps that actually defaulted to unchecked', async () => {
    store.lastIngestRun = {
      ok: true,
      finished_at: '2026-08-22T00:00:00Z',
      status: { scan: 'done', normalize: 'pending', mine: 'pending', validate: 'pending' },
      detail: {},
    };
    renderIngest();

    await screen.findByRole('checkbox', { name: /^Scan/, checked: false });
    expect(screen.getByText('(skipped — completed previously)')).toBeInTheDocument();
    // Only one hint -- every other step is still checked (never ran/pending), so no other hint renders.
    expect(screen.getAllByText('(skipped — completed previously)')).toHaveLength(1);
  });
});
