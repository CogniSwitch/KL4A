import { useEffect, useRef, useState } from 'react';
import * as api from '../lib/api';
import { useAsync } from '../lib/hooks';
import { subscribe } from '../lib/events';
import { renderMarkdown } from '../lib/markdown';
import { EmptyState } from '../components/common/Feedback';
import { Button } from '../components/common/Button';
import { Select, Textarea } from '../components/common/Input';
import { ConfirmDialog } from '../components/common/ConfirmDialog';
import type { AgentEntry, AgentProvider } from '../types/commands';

/**
 * Groups entries lacking a `chat_id` at all -- written before round 6's per-chat
 * memory feature existed -- into one shared bucket rather than one bucket per
 * legacy entry. Never assigned to a NEW chat (`handleNewChat` always generates a
 * real `crypto.randomUUID()`), so this id only ever appears for old data.
 */
const LEGACY_CHAT_ID = '__legacy__';

export interface ChatSummary {
  chatId: string;
  /** First turn's scenario text, truncated -- or a fixed label for the legacy bucket. */
  title: string;
  turnCount: number;
  lastActivityAt: string;
}

/**
 * Pure/exported for direct unit testing (same pattern `GraphScreen.tsx` uses for
 * `computeLayout`). Groups the flat transcript `get_agent_transcript` already
 * returns by `chat_id`, client-side -- no new backend "list chats" endpoint needed,
 * every field a chat-list row needs is already present on each entry. Sorted
 * newest-active-first, matching how a real chat app's own sidebar orders threads.
 */
export function groupIntoChats(transcript: AgentEntry[]): ChatSummary[] {
  const order: string[] = [];
  const byId = new Map<string, AgentEntry[]>();
  for (const entry of transcript) {
    const id = entry.chat_id ?? LEGACY_CHAT_ID;
    if (!byId.has(id)) {
      byId.set(id, []);
      order.push(id);
    }
    byId.get(id)!.push(entry);
  }
  const summaries = order.map((chatId) => {
    const entries = byId.get(chatId)!;
    const firstScenario = entries[0].scenario;
    const title =
      chatId === LEGACY_CHAT_ID
        ? 'Earlier chats (before chat history)'
        : firstScenario.length > 60
          ? `${firstScenario.slice(0, 60)}…`
          : firstScenario || 'New chat';
    return { chatId, title, turnCount: entries.length, lastActivityAt: entries[entries.length - 1].created_at };
  });
  return summaries.sort((a, b) => (a.lastActivityAt < b.lastActivityAt ? 1 : a.lastActivityAt > b.lastActivityAt ? -1 : 0));
}

function chatRowClass(active: boolean): string {
  return `block w-full rounded-md px-2.5 py-2 text-left text-sm ${active ? 'bg-accent-soft text-ink' : 'text-ink/80 hover:bg-panel-muted'}`;
}

function taskPillClass(active: boolean): string {
  return `rounded-full border px-2.5 py-1 text-xs ${
    active ? 'border-accent bg-accent-soft text-ink' : 'border-line bg-panel text-muted hover:bg-panel-muted'
  }`;
}

/**
 * `provider: "context"` touches no settings and opens no socket (D 28) — the
 * only zero-configuration path, and the default here so a fresh install has
 * something to click that always works.
 *
 * Chats (round 6, item 15): each chat is a `chat_id` (generated client-side via
 * `crypto.randomUUID()`), sent on every turn within it. The Agent screen groups
 * the flat transcript by that id into a real chat list (`groupIntoChats` above) —
 * "add a new chat" (item 13) is just switching `activeChatId` to a fresh one. This
 * is also this session's own precedent for a browsable list+detail view backed by
 * a flat, client-grouped record (no new backend surface beyond `chat_id` itself
 * and one small `delete_agent_chat` command) — reuse this shape before inventing a
 * different one for the next screen that needs a similar list.
 */
export function AgentScreen() {
  const tasks = useAsync(() => api.list_agent_tasks(), []);
  const transcript = useAsync(() => api.get_agent_transcript(), []);

  const [scenario, setScenario] = useState('');
  const [taskId, setTaskId] = useState('auto');
  const [provider, setProvider] = useState<AgentProvider>('context');
  const [allowProposed, setAllowProposed] = useState(false);
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<string | null>(null);
  const [runError, setRunError] = useState<string | null>(null);

  const [activeChatId, setActiveChatId] = useState<string>(() => crypto.randomUUID());
  const [deletingChat, setDeletingChat] = useState(false);
  const [pendingClearAll, setPendingClearAll] = useState(false);
  const [clearingAll, setClearingAll] = useState(false);

  useEffect(() => subscribe('agent://progress', (p) => setProgress(`${p.phase}: ${p.detail}`)), []);

  // On first load, if real chat history exists, land on the most recently active
  // chat instead of leaving the user on a fresh empty one they never asked for.
  // Only runs once -- after that, `activeChatId` is fully user-driven (New chat /
  // switching in the list), never silently reassigned out from under them.
  const initializedRef = useRef(false);
  useEffect(() => {
    if (initializedRef.current || transcript.loading) return;
    initializedRef.current = true;
    const chats = groupIntoChats(transcript.data ?? []);
    if (chats.length > 0) setActiveChatId(chats[0].chatId);
  }, [transcript.loading, transcript.data]);

  const chats = groupIntoChats(transcript.data ?? []);
  const activeSummary = chats.find((c) => c.chatId === activeChatId);
  const activeEntries = (transcript.data ?? []).filter((e) => (e.chat_id ?? LEGACY_CHAT_ID) === activeChatId);

  // Auto-scroll to the newest entry, same as any real chat window -- otherwise
  // a long transcript would silently leave a fresh answer below the fold.
  const bottomRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    // jsdom (this app's test environment) has no scrollIntoView implementation
    // at all -- guard rather than crash every test that renders this screen.
    bottomRef.current?.scrollIntoView?.({ block: 'nearest' });
  }, [activeEntries.length]);

  async function handleRun() {
    if (!scenario.trim()) return;
    setRunning(true);
    setRunError(null);
    setProgress(null);
    try {
      await api.run_agent_chat({
        scenario: scenario.trim(),
        task_id: taskId,
        provider,
        allow_proposed: allowProposed,
        chat_id: activeChatId,
      });
      await transcript.reload();
      setScenario('');
    } catch (err) {
      setRunError(err instanceof Error ? err.message : String(err));
    } finally {
      setRunning(false);
      setProgress(null);
    }
  }

  function handleNewChat() {
    setActiveChatId(crypto.randomUUID());
    setScenario('');
    setRunError(null);
  }

  async function handleDeleteActiveChat() {
    setDeletingChat(true);
    try {
      await api.delete_agent_chat(activeChatId);
      await transcript.reload();
      setActiveChatId(crypto.randomUUID());
    } finally {
      setDeletingChat(false);
    }
  }

  async function handleConfirmClearAll() {
    setClearingAll(true);
    try {
      await api.clear_agent_transcript();
      await transcript.reload();
      setActiveChatId(crypto.randomUUID());
    } finally {
      setClearingAll(false);
      setPendingClearAll(false);
    }
  }

  return (
    <div className="space-y-6">
      <h1 className="text-xl font-semibold text-ink">Agent</h1>

      <div className="flex gap-4">
        <aside className="w-56 shrink-0 space-y-2">
          <Button variant="secondary" className="w-full" onClick={handleNewChat}>
            + New chat
          </Button>
          <div aria-label="Chats" className="max-h-[420px] space-y-1 overflow-y-auto rounded-lg border border-line bg-panel p-1.5">
            {chats.length === 0 && <p className="p-2 text-xs text-muted-soft">No chats yet — ask something to start one.</p>}
            {chats.map((chat) => (
              <button key={chat.chatId} type="button" onClick={() => setActiveChatId(chat.chatId)} className={chatRowClass(chat.chatId === activeChatId)}>
                <p className="truncate font-medium">{chat.title}</p>
                <p className="mt-0.5 text-xs text-muted-soft">
                  {chat.turnCount} turn{chat.turnCount === 1 ? '' : 's'} · {new Date(chat.lastActivityAt).toLocaleDateString()}
                </p>
              </button>
            ))}
          </div>
          {chats.length > 0 && (
            <button type="button" onClick={() => setPendingClearAll(true)} className="text-xs text-bad underline decoration-bad/40">
              Clear all chat history
            </button>
          )}
        </aside>

        <section className="flex min-h-0 flex-1 flex-col rounded-lg border border-line bg-panel">
          <div className="flex items-center justify-between border-b border-line px-4 py-2.5">
            <div>
              <h2 className="truncate text-sm font-semibold text-ink">{activeSummary?.title ?? 'New chat'}</h2>
              <p className="text-xs text-muted-soft">
                azure-llm and azure-llm-tools answers build on this chat's earlier turns — context has no model to remember anything.
              </p>
            </div>
            {activeSummary && activeChatId !== LEGACY_CHAT_ID && (
              <Button variant="ghost" disabled={deletingChat} onClick={() => void handleDeleteActiveChat()} className="!text-bad decoration-bad/40">
                {deletingChat ? 'Deleting…' : 'Delete chat'}
              </Button>
            )}
          </div>

          <div role="log" aria-label="Chat messages" className="max-h-[420px] flex-1 space-y-4 overflow-y-auto p-4">
            {activeEntries.length === 0 && <EmptyState>No messages in this chat yet — ask something below.</EmptyState>}
            {activeEntries.map((entry, i) => (
              <div key={i} className="space-y-1.5">
                {/* User turn: right-aligned bubble, chat-convention. */}
                <div className="flex justify-end">
                  <div className="max-w-[85%] rounded-2xl rounded-br-sm bg-accent px-3.5 py-2 text-sm text-white">{entry.scenario}</div>
                </div>
                {/* Agent turn: left-aligned bubble. */}
                <div className="flex justify-start">
                  <div className="max-w-[85%] rounded-2xl rounded-bl-sm border border-line bg-panel-muted px-3.5 py-2 text-sm">
                    <div className="flex items-center justify-between gap-3 text-xs text-muted-soft">
                      <span>{entry.provider} · {entry.task_title}</span>
                      <span>{new Date(entry.created_at).toLocaleString()}</span>
                    </div>
                    {/* `entry.answer` is free-text from an LLM provider and often comes back as a GFM
                        table (rule-evaluation answers) — rendered as Markdown rather than a plain <p>
                        so those tables/lists/headings are actually readable. */}
                    <div className="mt-1 space-y-1.5">{renderMarkdown(entry.answer)}</div>
                    <p className="mt-2 text-xs text-muted-soft">
                      {entry.context_summary.usable_knowledge_count} knowledge items ·{' '}
                      {entry.context_summary.concept_count} concepts · {entry.detected_concepts.length} detected
                    </p>
                    {entry.trace && entry.trace.length > 0 && (
                      <details className="mt-2 rounded-lg border border-line bg-panel px-3 py-2 text-xs">
                        <summary className="cursor-pointer text-muted">
                          Looked something up {entry.trace.length} time{entry.trace.length === 1 ? '' : 's'} before answering
                        </summary>
                        <ol className="mt-2 space-y-2">
                          {entry.trace.map((call, callIndex) => (
                            <li key={callIndex} className="rounded border border-line bg-panel-muted p-2">
                              <p className="font-mono text-ink">
                                {call.tool}({JSON.stringify(call.args)})
                              </p>
                              <p className="mt-1 truncate text-muted-soft" title={JSON.stringify(call.result)}>
                                → {JSON.stringify(call.result)}
                              </p>
                            </li>
                          ))}
                        </ol>
                      </details>
                    )}
                  </div>
                </div>
              </div>
            ))}
            <div ref={bottomRef} />
          </div>

          {/*
           * Task context selector -- moved here (right above the composer that
           * actually consumes `taskId`) from its own disconnected full-width
           * section above the transcript, with a one-line explanation of what it
           * actually does: it pins which predefined context the NEXT question
           * draws from, it is not a to-do list or chat history of its own.
           */}
          <div className="border-t border-line px-4 py-2.5">
            <p className="text-xs text-muted-soft">
              Task context — pins which predefined scenario context the next question draws from. Leave on Auto to match automatically.
            </p>
            {tasks.loading && <p className="mt-1.5 text-xs text-muted-soft">Loading tasks…</p>}
            {tasks.error && <p className="mt-1.5 text-xs text-bad">Could not load tasks: {tasks.error}</p>}
            {tasks.data && (
              <div className="mt-1.5 flex flex-wrap gap-1.5">
                <button type="button" onClick={() => setTaskId('auto')} className={taskPillClass(taskId === 'auto')}>
                  Auto
                </button>
                {tasks.data.map((task) => (
                  <button key={task.id} type="button" onClick={() => setTaskId(task.id)} title={task.description} className={taskPillClass(taskId === task.id)}>
                    {task.title}
                  </button>
                ))}
              </div>
            )}
          </div>

          {/* Composer, pinned below the transcript like a real chat window's input bar. */}
          <div className="border-t border-line p-4">
            <Textarea
              value={scenario}
              onChange={(e) => setScenario(e.target.value)}
              placeholder="Describe the situation you need guidance on…"
              className="w-full"
              rows={3}
            />
            <div className="mt-3 flex flex-wrap items-center gap-4 text-sm text-ink">
              <label className="flex items-center gap-1.5">
                Provider
                <Select value={provider} onChange={(e) => setProvider(e.target.value as AgentProvider)} className="!py-1">
                  <option value="context">context (no LLM call)</option>
                  <option value="azure-llm">azure-llm</option>
                  <option value="azure-llm-tools">azure-llm (looks things up first)</option>
                </Select>
              </label>
              <label className="flex items-center gap-1.5">
                <input type="checkbox" checked={allowProposed} onChange={(e) => setAllowProposed(e.target.checked)} className="accent-accent" />
                Allow proposed/draft knowledge in the answer (advisory)
              </label>
            </div>
            <p className="mt-1 text-xs text-muted-soft">
              This checkbox is advisory only — it is embedded in the prompt payload and echoed back, but does not filter
              or exclude anything from retrieval on the backend.
            </p>

            {runError && <p className="mt-2 text-sm text-bad">{runError}</p>}
            {running && progress && <p className="mt-2 text-sm text-muted">{progress}</p>}

            <Button variant="primary" disabled={running || !scenario.trim()} onClick={() => void handleRun()} className="mt-3">
              {running ? 'Running…' : 'Ask'}
            </Button>
          </div>
        </section>
      </div>

      {pendingClearAll && (
        <ConfirmDialog
          title="Clear all chat history?"
          message="Deletes every chat's turns, not just the one you're viewing. Non-recoverable."
          confirmLabel={clearingAll ? 'Clearing…' : 'Clear all'}
          danger
          disabled={clearingAll}
          onConfirm={() => void handleConfirmClearAll()}
          onCancel={() => setPendingClearAll(false)}
        />
      )}
    </div>
  );
}
