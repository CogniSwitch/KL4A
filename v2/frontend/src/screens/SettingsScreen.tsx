import { useState } from 'react';
import * as api from '../lib/api';
import { useAsync } from '../lib/hooks';
import { LoadingBlock, ErrorBanner } from '../components/common/Feedback';
import { Button } from '../components/common/Button';
import { Input, Textarea } from '../components/common/Input';
import { ConfirmDialog } from '../components/common/ConfirmDialog';
import type { BundlePromptOverrides, McpClientTarget, McpInvocation, ProfileInput, ProfileView } from '../types/commands';

const EMPTY_DRAFT: ProfileInput = {
  name: '',
  base_url: '',
  auth_style: 'api-key',
  model: '',
  max_output_tokens: 2048,
  timeout_seconds: 60,
  reasoning_effort: 'medium',
  // Blank means "no override" -- the Rust/Python backend fully REPLACES its built-in
  // system prompt with whatever non-blank string is stored here (P-M18: no
  // prepend/append/templating). These two fields aren't exposed as inputs on this
  // screen yet, so a non-blank default here would silently poison every new profile's
  // mining/chat prompt with literal placeholder text -- which is exactly what
  // 'default-mining-v1'/'default-chat-v1' did before this fix.
  mining_prompt: '',
  chat_prompt: '',
  api_key: '',
};

function profileToDraft(p: ProfileView): ProfileInput {
  // Deliberately omits `api_key` — a `ProfileView` never carries one, so
  // there is nothing to copy in. The draft's `api_key` field stays blank
  // until the user types a new one; blank-on-save means "keep existing".
  return {
    id: p.id,
    name: p.name,
    base_url: p.base_url,
    auth_style: p.auth_style,
    model: p.model,
    max_output_tokens: p.max_output_tokens,
    timeout_seconds: p.timeout_seconds,
    reasoning_effort: p.reasoning_effort,
    mining_prompt: p.mining_prompt,
    chat_prompt: p.chat_prompt,
    api_key: '',
  };
}

export function SettingsScreen() {
  const settings = useAsync(() => api.get_settings(), []);
  const defaultPrompts = useAsync(() => api.get_default_prompts(), []);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [draft, setDraft] = useState<ProfileInput | null>(null);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [reviewerDraft, setReviewerDraft] = useState<string | null>(null);
  const [workersDraft, setWorkersDraft] = useState<number | null>(null);
  const [testResult, setTestResult] = useState<{ ok: boolean; detail: string } | null>(null);

  if (settings.loading) return <LoadingBlock label="Loading settings…" />;
  if (settings.error) return <ErrorBanner message="Could not load settings" detail={settings.error} />;
  if (!settings.data) return null;

  const overrideByField = new Map(settings.data.env_overrides.map((o) => [o.field, o]));
  const reviewerName = reviewerDraft ?? settings.data.reviewer_name;
  const maxWorkers = workersDraft ?? settings.data.max_parallel_workers;

  function selectProfile(profile: ProfileView) {
    setSelectedId(profile.id);
    setDraft(profileToDraft(profile));
    setTestResult(null);
    setSaveError(null);
  }

  function startNewProfile() {
    setSelectedId('__new__');
    setDraft({ ...EMPTY_DRAFT });
    setTestResult(null);
    setSaveError(null);
  }

  async function handleSave() {
    if (!draft) return;
    setSaving(true);
    setSaveError(null);
    try {
      // Blank/absent api_key means "keep existing" — don't send an empty string as a real key.
      const payload: ProfileInput = { ...draft, api_key: draft.api_key ? draft.api_key : undefined };
      await api.save_profile(payload);
      await settings.reload();
      setSelectedId(null);
      setDraft(null);
    } catch (err) {
      setSaveError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  }

  async function handleDelete(id: string) {
    await api.delete_profile(id);
    await settings.reload();
    if (selectedId === id) {
      setSelectedId(null);
      setDraft(null);
    }
  }

  async function handleSetDefault(id: string) {
    await api.set_default_profile(id);
    await settings.reload();
  }

  async function handleTestConnection(id: string) {
    setTestResult(null);
    const result = await api.test_profile_connection(id);
    setTestResult(result);
  }

  async function handleSaveReviewer() {
    if (reviewerDraft === null) return;
    await api.set_reviewer_name(reviewerDraft);
    await settings.reload();
    setReviewerDraft(null);
  }

  async function handleSaveWorkers() {
    if (workersDraft === null) return;
    await api.set_max_parallel_workers(workersDraft);
    await settings.reload();
    setWorkersDraft(null);
  }

  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-xl font-semibold text-ink">Settings</h1>
        <p className="mt-1 text-sm text-muted">
          Mostly global — independent of any bundle, stored at{' '}
          <code className="rounded bg-panel-muted px-1 py-0.5 font-mono text-xs">{settings.data.settings_path}</code>.
          Two sections below (Bundle prompt overrides, MCP invocation) apply only to the currently selected bundle.
        </p>
      </div>

      <section>
        <h2 className="text-sm font-semibold text-ink">Reviewer name</h2>
        <div className="mt-2 flex max-w-sm gap-2">
          <Input
            type="text"
            value={reviewerName}
            onChange={(e) => setReviewerDraft(e.target.value)}
            className="flex-1"
          />
          <Button
            variant="primary"
            disabled={reviewerDraft === null || reviewerDraft === settings.data.reviewer_name}
            onClick={() => void handleSaveReviewer()}
          >
            Save
          </Button>
        </div>
        <p className="mt-1 text-xs text-muted">
          Used as the default reviewer on review actions when none is explicitly given.
        </p>
      </section>

      <section>
        <h2 className="text-sm font-semibold text-ink">Parallel LLM requests</h2>
        <div className="mt-2 flex max-w-sm gap-2">
          <Input
            type="number"
            min={1}
            max={32}
            value={maxWorkers}
            onChange={(e) => setWorkersDraft(Math.max(1, Math.trunc(Number(e.target.value) || 1)))}
            className="w-24"
          />
          <Button
            variant="primary"
            disabled={workersDraft === null || workersDraft === settings.data.max_parallel_workers}
            onClick={() => void handleSaveWorkers()}
          >
            Save
          </Button>
        </div>
        <p className="mt-1 text-xs text-muted">
          How many sections/chunks/sources are sent to the LLM at once during Normalize and Mine. Lower this if your
          LLM endpoint throttles or errors under concurrent requests. Default: 6.
        </p>
      </section>

      <section>
        <h2 className="text-sm font-semibold text-ink">Prompts reference</h2>
        <p className="mt-1 text-xs text-muted">
          What each pipeline step actually sends the LLM. Read-only here — Mining/Chat can be overridden per profile
          below; Normalize's two prompts cannot be.
        </p>
        <div className="mt-2 space-y-2">
          <PromptDetails label="Normalize — heading index (per chunk)" text={defaultPrompts.data?.heading_index_prompt} />
          <PromptDetails label="Normalize — heading relevel (whole document)" text={defaultPrompts.data?.heading_relevel_prompt} />
          <PromptDetails label="Mine — default author prompt" text={defaultPrompts.data?.mining_prompt} />
          <PromptDetails label="Agent — default chat prompt" text={defaultPrompts.data?.chat_prompt} />
        </div>
      </section>

      <section>
        <h2 className="text-sm font-semibold text-ink">Bundle prompt overrides</h2>
        <p className="mt-1 text-xs text-muted">
          Applies only to the currently selected bundle, and wins over even a non-blank profile-level override below —
          useful for one bundle (e.g. a specialized domain) that needs different prompt wording without changing a
          profile every other bundle also uses. Blank uses the profile's own prompt, or the built-in default if the
          profile has none.
        </p>
        <BundlePromptOverridesPanel defaultPrompts={defaultPrompts.data} />
      </section>

      <section>
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-semibold text-ink">LLM profiles</h2>
          <Button variant="secondary" onClick={startNewProfile} className="!py-1">
            + New profile
          </Button>
        </div>

        <ul className="mt-3 divide-y divide-line rounded-lg border border-line bg-panel">
          {settings.data.profiles.map((p) => (
            <li key={p.id} className="flex items-center justify-between px-4 py-2.5">
              <button type="button" onClick={() => selectProfile(p)} className="text-left">
                <span className="text-sm font-medium text-ink">{p.name}</span>
                {p.is_default && (
                  <span className="ml-2 rounded-full bg-accent px-2 py-0.5 text-xs text-white">default</span>
                )}
                <span className="ml-2 text-xs text-muted">{p.model}</span>
                {/* NEVER the key itself — only this boolean crosses the IPC boundary. */}
                <span className="ml-2 text-xs text-muted-soft">{p.has_api_key ? 'key set' : 'no key'}</span>
              </button>
              <div className="flex items-center gap-3 text-sm">
                {!p.is_default && (
                  <Button variant="ghost" onClick={() => void handleSetDefault(p.id)} className="!text-muted decoration-muted/40">
                    Make default
                  </Button>
                )}
                <Button variant="ghost" onClick={() => void handleTestConnection(p.id)} className="!text-muted decoration-muted/40">
                  Test
                </Button>
                <Button variant="ghost" onClick={() => void handleDelete(p.id)} className="!text-bad decoration-bad/40">
                  Delete
                </Button>
              </div>
            </li>
          ))}
          {settings.data.profiles.length === 0 && (
            <li className="px-4 py-6 text-center text-sm text-muted">No profiles configured yet.</li>
          )}
        </ul>

        {testResult && (
          <div className={`mt-2 rounded-lg px-3 py-2 text-sm ${testResult.ok ? 'bg-ok-soft text-ok' : 'bg-bad-soft text-bad'}`}>
            {testResult.detail}
          </div>
        )}
      </section>

      {draft && (
        <section className="rounded-lg border border-line bg-panel p-5">
          <h2 className="text-sm font-semibold text-ink">
            {selectedId === '__new__' ? 'New profile' : `Edit ${draft.name || 'profile'}`}
          </h2>
          <div className="mt-3 grid grid-cols-2 gap-4">
            <Field label="Name" overridden={false}>
              <Input className="w-full" value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} />
            </Field>
            <Field label="Base URL" overridden={overrideByField.get('base_url')?.active_value_present}>
              <Input className="w-full" value={draft.base_url} onChange={(e) => setDraft({ ...draft, base_url: e.target.value })} />
            </Field>
            <Field label="Model" overridden={false}>
              <Input className="w-full" value={draft.model} onChange={(e) => setDraft({ ...draft, model: e.target.value })} />
            </Field>
            <Field label="Auth style" overridden={false}>
              <Input className="w-full" value={draft.auth_style} onChange={(e) => setDraft({ ...draft, auth_style: e.target.value })} />
            </Field>
            <Field label="Max output tokens" overridden={false}>
              <Input
                className="w-full"
                type="number"
                value={draft.max_output_tokens}
                onChange={(e) => setDraft({ ...draft, max_output_tokens: Number(e.target.value) })}
              />
            </Field>
            <Field label="Timeout (seconds)" overridden={false}>
              <Input
                className="w-full"
                type="number"
                value={draft.timeout_seconds}
                onChange={(e) => setDraft({ ...draft, timeout_seconds: Number(e.target.value) })}
              />
            </Field>
            <Field label="Reasoning effort" overridden={false}>
              <Input className="w-full" value={draft.reasoning_effort} onChange={(e) => setDraft({ ...draft, reasoning_effort: e.target.value })} />
            </Field>
            <Field label="API key" overridden={overrideByField.get('api_key')?.active_value_present}>
              <Input
                className="w-full"
                type="password"
                autoComplete="new-password"
                placeholder="Leave blank to keep existing key"
                value={draft.api_key ?? ''}
                onChange={(e) => setDraft({ ...draft, api_key: e.target.value })}
              />
            </Field>
          </div>

          <div className="mt-4 grid grid-cols-1 gap-4 sm:grid-cols-2">
            <PromptField
              label="Mining prompt"
              value={draft.mining_prompt}
              defaultValue={defaultPrompts.data?.mining_prompt}
              onChange={(v) => setDraft({ ...draft, mining_prompt: v })}
            />
            <PromptField
              label="Chat prompt"
              value={draft.chat_prompt}
              defaultValue={defaultPrompts.data?.chat_prompt}
              onChange={(v) => setDraft({ ...draft, chat_prompt: v })}
            />
          </div>

          {saveError && <p className="mt-3 text-sm text-bad">{saveError}</p>}

          <div className="mt-4 flex gap-2">
            <Button
              variant="primary"
              disabled={saving || !draft.name || !draft.base_url || !draft.model}
              onClick={() => void handleSave()}
            >
              {saving ? 'Saving…' : 'Save profile'}
            </Button>
            <Button
              variant="secondary"
              onClick={() => {
                setSelectedId(null);
                setDraft(null);
              }}
            >
              Cancel
            </Button>
          </div>
        </section>
      )}

      <section>
        <h2 className="text-sm font-semibold text-ink">MCP invocation</h2>
        <McpPanel />
      </section>

      <section>
        <h2 className="text-sm font-semibold text-ink">Diagnostics</h2>
        <DiagnosticsPanel />
      </section>
    </div>
  );
}

/**
 * Blank means "use the built-in default" -- any non-blank text here fully REPLACES
 * that default (no merging, per P-M18/`build_author_messages`'s doc comment). Shows
 * the actual default inline via <details> so a user writing an override can see
 * exactly what they're discarding, rather than guessing -- the previous silent
 * poisoning of this exact field (a UI placeholder saved as a real value with no
 * visibility into what it replaced) is why these fields are editable at all now.
 */
function PromptField({
  label,
  value,
  defaultValue,
  onChange,
}: {
  label: string;
  value: string;
  defaultValue: string | undefined;
  onChange: (v: string) => void;
}) {
  return (
    <label className="block text-sm">
      <span className="font-medium text-ink">{label}</span>
      <Textarea
        className="mt-1 w-full font-mono text-xs"
        rows={4}
        placeholder="Leave blank to use the built-in default (shown below)"
        value={value}
        onChange={(e) => onChange(e.target.value)}
      />
      <p className="mt-1 text-xs text-muted">
        Blank uses the built-in default. Any text here completely replaces it — it is not merged in.
      </p>
      {defaultValue && (
        <details className="mt-1">
          <summary className="cursor-pointer text-xs text-accent">View built-in default</summary>
          <pre className="mt-1 max-h-48 overflow-y-auto whitespace-pre-wrap rounded-lg bg-panel-muted px-2 py-1.5 text-xs text-muted">
            {defaultValue}
          </pre>
        </details>
      )}
    </label>
  );
}

function Field({ label, overridden, children }: { label: string; overridden?: boolean; children: React.ReactNode }) {
  return (
    <label className="block text-sm">
      <span className="flex items-center gap-1.5 font-medium text-ink">
        {label}
        {overridden && (
          <span
            title="An environment variable is currently overriding this field; editing it here has no effect until that env var is unset."
            className="rounded-full bg-warn-soft px-1.5 py-0.5 text-[10px] font-semibold text-warn"
          >
            env override active
          </span>
        )}
      </span>
      <div className={overridden ? 'opacity-50' : ''}>{children}</div>
    </label>
  );
}

/** A collapsible, read-only, verbatim view of one system prompt. */
function PromptDetails({ label, text }: { label: string; text: string | undefined }) {
  return (
    <details className="rounded-lg border border-line bg-panel">
      <summary className="cursor-pointer px-3 py-2 text-sm font-medium text-ink">{label}</summary>
      <pre className="max-h-64 overflow-auto whitespace-pre-wrap border-t border-line bg-panel-muted px-3 py-2 font-mono text-xs text-ink/80">
        {text ?? 'Loading…'}
      </pre>
    </details>
  );
}

/** Bundle-scoped -- unlike everything else on this screen, see the section's own copy above. */
function BundlePromptOverridesPanel({ defaultPrompts }: { defaultPrompts: { mining_prompt: string; chat_prompt: string } | null }) {
  const overrides = useAsync(() => api.get_bundle_prompt_overrides(), []);
  const [draft, setDraft] = useState<BundlePromptOverrides | null>(null);
  const [saving, setSaving] = useState(false);

  if (overrides.loading) return <LoadingBlock label="Loading bundle prompt overrides…" />;
  if (overrides.error) {
    // Bundle-scoped and no bundle may be selected -- that's an expected state here, not a failure to surface loudly.
    return <p className="mt-2 text-sm text-muted">Select a bundle to configure its prompt overrides.</p>;
  }
  if (!overrides.data) return null;

  const current = draft ?? overrides.data;
  const dirty = draft !== null && (draft.mining_prompt !== overrides.data.mining_prompt || draft.chat_prompt !== overrides.data.chat_prompt);

  async function handleSave() {
    if (!draft) return;
    setSaving(true);
    try {
      await api.set_bundle_prompt_overrides(draft);
      await overrides.reload();
      setDraft(null);
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="mt-2">
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <PromptField
          label="Mining prompt override"
          value={current.mining_prompt}
          defaultValue={defaultPrompts?.mining_prompt}
          onChange={(v) => setDraft({ ...current, mining_prompt: v })}
        />
        <PromptField
          label="Chat prompt override"
          value={current.chat_prompt}
          defaultValue={defaultPrompts?.chat_prompt}
          onChange={(v) => setDraft({ ...current, chat_prompt: v })}
        />
      </div>
      <div className="mt-3">
        <Button variant="primary" disabled={!dirty || saving} onClick={() => void handleSave()}>
          {saving ? 'Saving…' : 'Save bundle overrides'}
        </Button>
      </div>
    </div>
  );
}

/**
 * Task #38: one zip a user can hand over when troubleshooting, instead of being
 * walked through finding `sopkb-startup.log`/`settings.json`/a bundle's `.sopkb/`
 * state by hand. Never includes a raw API key or full knowledge content -- see
 * `commands::diagnostics`'s own doc comment on the backend side for exactly what's
 * redacted and why.
 */
function DiagnosticsPanel() {
  const [saving, setSaving] = useState(false);
  const [savedPath, setSavedPath] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function handleExport() {
    setSaving(true);
    setError(null);
    setSavedPath(null);
    try {
      const path = await api.export_diagnostics_bundle();
      if (path) setSavedPath(path);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="mt-2">
      <p className="text-xs text-muted">
        Saves a zip with app/OS info, the startup log (if any), a redacted settings summary (never your API keys), and
        the currently selected bundle's status — useful to attach when reporting a problem.
      </p>
      <div className="mt-2">
        <Button variant="secondary" disabled={saving} onClick={() => void handleExport()}>
          {saving ? 'Saving…' : 'Export diagnostics bundle…'}
        </Button>
      </div>
      {savedPath && (
        <p className="mt-2 text-sm text-ok">
          Saved to <code className="rounded bg-panel-muted px-1 py-0.5 font-mono text-xs">{savedPath}</code>.
        </p>
      )}
      {error && <p className="mt-2 text-sm text-bad">{error}</p>}
    </div>
  );
}

/**
 * A copyable manual-setup instruction for a client this app could not locate
 * automatically. Uses `invocation.entry_name` so it names the entry exactly the
 * way an automatic "Configure automatically" would have -- never re-derives that
 * name itself. Presentational only, never executed by this app; the real argv
 * shape a CLI client would use lives in `mcp.rs`'s `build_add_args`, which this
 * mirrors for display purposes only.
 */
function buildManualSnippet(target: McpClientTarget, invocation: McpInvocation): string {
  const fullArgs = [...invocation.args, invocation.enable_review_notes_flag];
  if (target.method === 'config-file') {
    const where = target.default_location_hint ?? "that client's MCP config file";
    const json = JSON.stringify(
      { mcpServers: { [invocation.entry_name]: { command: invocation.command, args: fullArgs } } },
      null,
      2,
    );
    return `Add this to the "mcpServers" object in:\n${where}\n(create the file with just this content if it doesn't exist yet)\n\n${json}`;
  }
  const scopeArgs = target.id === 'claude-code' ? ' --scope user' : '';
  const cliName = target.id === 'codex' ? 'codex' : 'claude';
  return `${cliName} mcp add${scopeArgs} ${invocation.entry_name} -- ${invocation.command} ${fullArgs.join(' ')}`;
}

function McpPanel() {
  const invocation = useAsync(() => api.get_mcp_invocation(), []);
  const targets = useAsync(() => api.list_mcp_client_targets(), []);
  const [pending, setPending] = useState<{ target: McpClientTarget; force: boolean } | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Record<string, { text: string; ok: boolean }>>({});
  const [rescanning, setRescanning] = useState(false);
  const [snippetCopiedId, setSnippetCopiedId] = useState<string | null>(null);

  if (invocation.loading) return <LoadingBlock label="Loading MCP invocation…" />;
  if (invocation.error) {
    // Bundle-scoped and no bundle may be selected — that's an expected state here, not a failure to surface loudly.
    return <p className="mt-2 text-sm text-muted">Select a bundle to see its MCP invocation command.</p>;
  }
  if (!invocation.data) return null;
  const line = [invocation.data.command, ...invocation.data.args].join(' ');

  async function runConfigure(target: McpClientTarget, force: boolean) {
    setBusyId(target.id);
    try {
      const result = await api.configure_mcp_client(target.id, force);
      setMessages((m) => ({ ...m, [target.id]: { text: result.message, ok: result.outcome === 'configured' } }));
      // A same-named entry already exists -- escalate to a second, explicit
      // overwrite confirmation rather than silently doing nothing (same
      // "two full confirms for a collision" shape this app uses elsewhere).
      setPending(result.outcome === 'already_configured' ? { target, force: true } : null);
    } catch (err) {
      setMessages((m) => ({ ...m, [target.id]: { text: err instanceof Error ? err.message : String(err), ok: false } }));
      setPending(null);
    } finally {
      setBusyId(null);
    }
  }

  async function runRescan() {
    setRescanning(true);
    try {
      await api.rescan_mcp_client_targets();
      targets.reload();
    } finally {
      setRescanning(false);
    }
  }

  function copySnippet(target: McpClientTarget) {
    if (!invocation.data) return;
    void navigator.clipboard.writeText(buildManualSnippet(target, invocation.data));
    setSnippetCopiedId(target.id);
  }

  return (
    <div className="mt-2">
      <p className="text-xs text-muted">
        Run this from an external agent host to expose this bundle over MCP. Add{' '}
        <code className="rounded bg-panel-muted px-1 py-0.5 font-mono">{invocation.data.enable_review_notes_flag}</code> to allow
        that agent to write review notes.
      </p>
      <div className="mt-2 flex items-center gap-2">
        <code className="flex-1 overflow-x-auto rounded-lg bg-sidebar px-3 py-2 font-mono text-xs text-sidebar-ink">{line}</code>
        <Button variant="secondary" onClick={() => void navigator.clipboard.writeText(line)} className="!py-1 !text-xs">
          Copy
        </Button>
      </div>

      <div className="mt-4 flex items-center justify-between gap-3">
        <h3 className="text-xs font-semibold uppercase tracking-wide text-muted-soft">Configure automatically</h3>
        <Button variant="secondary" disabled={rescanning} className="!py-1 !text-xs shrink-0" onClick={() => void runRescan()}>
          {rescanning ? 'Rescanning…' : 'Rescan'}
        </Button>
      </div>

      {!targets.loading && targets.data === null && (
        <p className="mt-2 text-xs text-muted">Still detecting installed MCP clients — try Rescan in a moment.</p>
      )}

      {targets.data && targets.data.length > 0 && (
        <div className="mt-2 space-y-2">
          {targets.data.map((target) => {
            const message = messages[target.id];
            return (
              <div key={target.id} className="flex items-center justify-between gap-3 rounded-lg border border-line bg-panel-muted px-3 py-2 text-sm">
                <div className="min-w-0">
                  <div className="font-medium text-ink">{target.label}</div>
                  {target.located ? (
                    <div className="truncate text-xs text-muted-soft">{target.location}</div>
                  ) : (
                    <div className="text-xs text-warn">{target.note}</div>
                  )}
                  {message && <div className={`mt-1 text-xs ${message.ok ? 'text-ok' : 'text-bad'}`}>{message.text}</div>}
                </div>
                <div className="flex shrink-0 items-center gap-2">
                  {!target.located && (
                    <Button
                      variant="secondary"
                      className="!py-1 !text-xs shrink-0"
                      onClick={() => copySnippet(target)}
                    >
                      {snippetCopiedId === target.id ? 'Copied' : 'Copy setup snippet'}
                    </Button>
                  )}
                  <Button
                    variant="secondary"
                    disabled={!target.located || busyId === target.id}
                    className="!py-1 !text-xs shrink-0"
                    onClick={() => setPending({ target, force: false })}
                  >
                    {busyId === target.id ? 'Working…' : 'Configure automatically'}
                  </Button>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {pending && (
        <ConfirmDialog
          title={pending.force ? `Overwrite existing entry in ${pending.target.label}?` : `Configure ${pending.target.label}?`}
          message={
            pending.force
              ? `${messages[pending.target.id]?.text ?? 'An entry with this name already exists.'} Overwriting keeps a backup of the original file first, when one exists.`
              : `This writes "${line}" into ${pending.target.location ?? pending.target.label}` +
                (pending.target.method === 'config-file' ? ' (a backup is made first if the file already exists).' : '.')
          }
          confirmLabel={busyId === pending.target.id ? 'Working…' : pending.force ? 'Overwrite' : 'Configure'}
          danger={pending.force}
          disabled={busyId === pending.target.id}
          onConfirm={() => void runConfigure(pending.target, pending.force)}
          onCancel={() => setPending(null)}
        />
      )}
    </div>
  );
}
