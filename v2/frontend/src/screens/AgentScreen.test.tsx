import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { AgentScreen, groupIntoChats } from './AgentScreen';
import { __resetStoreForTests, store } from '../mock/store';
import type { AgentEntry } from '../types/commands';

function renderAgent() {
  return render(<AgentScreen />);
}

function entry(overrides: Partial<AgentEntry>): AgentEntry {
  return {
    created_at: '2026-08-01T00:00:00Z',
    provider: 'context',
    task_id: 'auto',
    task_title: 'Auto',
    scenario: 's',
    allow_proposed: false,
    detected_concepts: [],
    context_summary: {
      usable_knowledge_count: 0,
      decision_rule_count: 0,
      relation_count: 0,
      evidence_count: 0,
      concept_count: 0,
      detected_concept_count: 0,
      excluded_knowledge_count: 0,
    },
    answer: 'a',
    ...overrides,
  };
}

describe('groupIntoChats', () => {
  it('groups entries by chat_id, oldest turn first within a chat', () => {
    const chats = groupIntoChats([
      entry({ chat_id: 'c1', scenario: 'q1', created_at: '2026-08-01T00:00:00Z' }),
      entry({ chat_id: 'c2', scenario: 'other', created_at: '2026-08-01T01:00:00Z' }),
      entry({ chat_id: 'c1', scenario: 'q2', created_at: '2026-08-01T02:00:00Z' }),
    ]);
    const c1 = chats.find((c) => c.chatId === 'c1')!;
    expect(c1.turnCount).toBe(2);
    expect(c1.title).toBe('q1');
    expect(c1.lastActivityAt).toBe('2026-08-01T02:00:00Z');
  });

  it('sorts chats newest-active-first', () => {
    const chats = groupIntoChats([
      entry({ chat_id: 'older', scenario: 'a', created_at: '2026-08-01T00:00:00Z' }),
      entry({ chat_id: 'newer', scenario: 'b', created_at: '2026-08-02T00:00:00Z' }),
    ]);
    expect(chats.map((c) => c.chatId)).toEqual(['newer', 'older']);
  });

  it('groups every entry with no chat_id into one shared legacy bucket', () => {
    const chats = groupIntoChats([entry({ scenario: 'a' }), entry({ scenario: 'b' })]);
    expect(chats).toHaveLength(1);
    expect(chats[0].turnCount).toBe(2);
    expect(chats[0].title).toBe('Earlier chats (before chat history)');
  });

  it('truncates a long first scenario for the chat title', () => {
    const long = 'x'.repeat(80);
    const chats = groupIntoChats([entry({ chat_id: 'c1', scenario: long })]);
    expect(chats[0].title).toBe(`${'x'.repeat(60)}…`);
  });
});

describe('AgentScreen azure-llm-tools provider', () => {
  beforeEach(() => {
    __resetStoreForTests();
    store.selectedBundle = 'glp1-healthcare';
  });

  it('offers the tool-using provider alongside context and azure-llm', async () => {
    renderAgent();
    const select = screen.getByRole('combobox', { name: /provider/i });
    expect(screen.getByRole('option', { name: /looks things up first/i })).toBeInTheDocument();
    await userEvent.selectOptions(select, 'azure-llm-tools');
    expect((select as HTMLSelectElement).value).toBe('azure-llm-tools');
  });

  it('requires a configured LLM profile for azure-llm-tools, same as azure-llm', async () => {
    store.settingsProfiles = [];
    renderAgent();
    await userEvent.selectOptions(screen.getByRole('combobox', { name: /provider/i }), 'azure-llm-tools');
    await userEvent.type(screen.getByPlaceholderText(/describe the situation/i), 'can staff proceed');
    await userEvent.click(screen.getByRole('button', { name: 'Ask' }));
    expect(await screen.findByText(/no llm profile is configured/i)).toBeInTheDocument();
  });

  it('shows a collapsible tool-call trace for an azure-llm-tools run, and not for a context run', async () => {
    renderAgent();

    await userEvent.type(screen.getByPlaceholderText(/describe the situation/i), 'can staff proceed');
    await userEvent.click(screen.getByRole('button', { name: 'Ask' }));
    await screen.findByText(/context provider/i);
    expect(screen.queryByText(/looked something up/i)).not.toBeInTheDocument();

    await userEvent.selectOptions(screen.getByRole('combobox', { name: /provider/i }), 'azure-llm-tools');
    // `handleRun` clears the textarea after a successful run -- re-type for round 2.
    await userEvent.type(screen.getByPlaceholderText(/describe the situation/i), 'can staff proceed');
    await userEvent.click(screen.getByRole('button', { name: 'Ask' }));

    const trace = await screen.findByText(/looked something up 1 time/i);
    expect(trace).toBeInTheDocument();
    await userEvent.click(trace);
    expect(screen.getByText(/knowledge\.search/)).toBeInTheDocument();
  });
});

describe('AgentScreen chat-window layout', () => {
  beforeEach(() => {
    __resetStoreForTests();
    store.selectedBundle = 'glp1-healthcare';
  });

  it('discloses that azure-llm answers remember this chat, unlike context', async () => {
    renderAgent();
    expect(await screen.findByText(/build on this chat's earlier turns/i)).toBeInTheDocument();
  });

  it('orders the transcript oldest first, like a real chat thread', async () => {
    renderAgent();
    // Scoped to the transcript panel only -- once a chat exists, its sidebar
    // row title is the SAME scenario text as its first bubble, so an
    // unscoped query would (correctly) find both.
    const chatLog = screen.getByRole('log', { name: 'Chat messages' });

    const box = screen.getByPlaceholderText(/describe the situation/i) as HTMLTextAreaElement;
    await userEvent.type(box, 'first question');
    await userEvent.click(screen.getByRole('button', { name: 'Ask' }));
    await within(chatLog).findByText('first question');
    // `handleRun`'s own `setScenario('')` (fire-and-forget from the onClick
    // handler, not awaited by `userEvent.click`) can still be in flight once
    // the transcript itself has updated -- wait for the textarea to actually
    // read back empty before typing again, so this never races that reset
    // and either loses keystrokes or types onto a not-yet-cleared value.
    await vi.waitFor(() => expect(box.value).toBe(''));
    await userEvent.type(box, 'second question');
    await userEvent.click(screen.getByRole('button', { name: 'Ask' }));
    await within(chatLog).findByText('second question');

    // Exact-match only -- the agent's own answer bubble quotes the scenario text
    // back inside a longer sentence ('...for scenario "first question": ...'),
    // which a substring match would also (wrongly) pick up.
    const questions = within(chatLog).getAllByText(/^(first|second) question$/);
    expect(questions.map((el) => el.textContent)).toEqual(['first question', 'second question']);
  });
});

describe('AgentScreen chats list', () => {
  beforeEach(() => {
    __resetStoreForTests();
    store.selectedBundle = 'glp1-healthcare';
  });

  it('starts on an empty new chat with no prior history', async () => {
    renderAgent();
    expect(await screen.findByText(/no messages in this chat yet/i)).toBeInTheDocument();
    expect(screen.getByText(/no chats yet/i)).toBeInTheDocument();
  });

  it('a sent question creates a chat that appears in the list, and a second question stays in the same chat', async () => {
    renderAgent();
    const chatLog = screen.getByRole('log', { name: 'Chat messages' });
    const chatsList = screen.getByLabelText('Chats');
    const box = screen.getByPlaceholderText(/describe the situation/i) as HTMLTextAreaElement;

    await userEvent.type(box, 'first question');
    await userEvent.click(screen.getByRole('button', { name: 'Ask' }));
    await within(chatLog).findByText('first question');
    expect(within(chatsList).getByText('first question')).toBeInTheDocument();

    await vi.waitFor(() => expect(box.value).toBe(''));
    await userEvent.type(box, 'second question');
    await userEvent.click(screen.getByRole('button', { name: 'Ask' }));
    await within(chatLog).findByText('second question');

    // Still exactly one chat in the sidebar -- both turns landed in the same chat_id.
    // The row's own text is "2 turns · <date>" in one paragraph, hence the regex.
    expect(within(chatsList).getAllByText(/^2 turns/)).toHaveLength(1);
  });

  it('"+ New chat" switches to a fresh empty chat without touching the previous one', async () => {
    renderAgent();
    const chatLog = screen.getByRole('log', { name: 'Chat messages' });
    const chatsList = screen.getByLabelText('Chats');
    const box = screen.getByPlaceholderText(/describe the situation/i) as HTMLTextAreaElement;
    await userEvent.type(box, 'first chat question');
    await userEvent.click(screen.getByRole('button', { name: 'Ask' }));
    await within(chatLog).findByText('first chat question');

    await userEvent.click(screen.getByRole('button', { name: '+ New chat' }));
    expect(screen.getByText(/no messages in this chat yet/i)).toBeInTheDocument();
    expect(within(chatLog).queryByText('first chat question')).not.toBeInTheDocument();

    // The first chat is still listed in the sidebar, just not the active one.
    expect(within(chatsList).getByText(/first chat question/)).toBeInTheDocument();
  });

  it('switching back to an earlier chat in the list re-shows its own messages, and the second chat is memoryless', async () => {
    renderAgent();
    const chatLog = screen.getByRole('log', { name: 'Chat messages' });
    const chatsList = screen.getByLabelText('Chats');
    const box = screen.getByPlaceholderText(/describe the situation/i) as HTMLTextAreaElement;
    await userEvent.type(box, 'chat one question');
    await userEvent.click(screen.getByRole('button', { name: 'Ask' }));
    await within(chatLog).findByText('chat one question');

    await userEvent.click(screen.getByRole('button', { name: '+ New chat' }));
    await userEvent.type(box, 'chat two question');
    await userEvent.click(screen.getByRole('button', { name: 'Ask' }));
    await within(chatLog).findByText('chat two question');
    expect(within(chatLog).queryByText('chat one question')).not.toBeInTheDocument();

    await userEvent.click(within(chatsList).getByText(/chat one question/));
    expect(await within(chatLog).findByText('chat one question')).toBeInTheDocument();
    expect(within(chatLog).queryByText('chat two question')).not.toBeInTheDocument();
  });

  it('"Delete chat" removes only the active chat and starts a fresh one', async () => {
    renderAgent();
    const chatLog = screen.getByRole('log', { name: 'Chat messages' });
    const box = screen.getByPlaceholderText(/describe the situation/i) as HTMLTextAreaElement;
    await userEvent.type(box, 'a chat to delete');
    await userEvent.click(screen.getByRole('button', { name: 'Ask' }));
    await within(chatLog).findByText('a chat to delete');

    await userEvent.click(screen.getByRole('button', { name: 'Delete chat' }));
    await vi.waitFor(() => expect(screen.queryByText(/no chats yet/i)).toBeInTheDocument());
  });
});
