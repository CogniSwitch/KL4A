import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SettingsScreen } from './SettingsScreen';
import { __resetStoreForTests, store } from '../mock/store';

function renderSettings() {
  return render(<SettingsScreen />);
}

describe('SettingsScreen parallel-worker count', () => {
  beforeEach(() => {
    __resetStoreForTests();
  });

  it('shows the current default and persists a changed value', async () => {
    renderSettings();
    const input = await screen.findByDisplayValue('6');

    fireEvent.change(input, { target: { value: '3' } });
    const fieldContainer = input.closest('div')!;
    await userEvent.click(within(fieldContainer).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(store.maxParallelWorkers).toBe(3));
  });

  it('never lets the value drop below 1', async () => {
    renderSettings();
    const input = await screen.findByDisplayValue('6');
    fireEvent.change(input, { target: { value: '0' } });
    expect((input as HTMLInputElement).value).toBe('1');
  });
});

describe('SettingsScreen prompts reference', () => {
  beforeEach(() => {
    __resetStoreForTests();
  });

  it('shows all four step prompts, collapsed by default', async () => {
    renderSettings();
    const summary = await screen.findByText(/Normalize — heading index/);
    expect(screen.getByText(/Normalize — heading relevel/)).toBeInTheDocument();
    expect(screen.getByText(/Mine — default author prompt/)).toBeInTheDocument();
    expect(screen.getByText(/Agent — default chat prompt/)).toBeInTheDocument();

    const details = summary.closest('details')!;
    expect(details.open).toBe(false);
    await userEvent.click(summary);
    expect(details.open).toBe(true);
    expect(screen.getByText(/document indexing engine/i)).toBeInTheDocument();
  });
});

describe('SettingsScreen bundle prompt overrides', () => {
  beforeEach(() => {
    __resetStoreForTests();
  });

  it('starts blank, persists a typed override to the selected bundle, and survives a reload', async () => {
    renderSettings();
    const label = await screen.findByText('Mining prompt override');
    const textarea = label.closest('label')!.querySelector('textarea')!;
    expect(textarea).toHaveValue('');

    fireEvent.change(textarea, { target: { value: 'Custom mining prompt for this bundle.' } });
    const saveButton = screen.getByRole('button', { name: 'Save bundle overrides' });
    await userEvent.click(saveButton);

    await waitFor(() => expect(store.getBundlePromptOverrides(store.selectedBundle).mining_prompt).toBe('Custom mining prompt for this bundle.'));
  });

  it('shows a fallback message instead of the panel when no bundle is selected', async () => {
    store.deselectBundle();
    renderSettings();
    expect(await screen.findByText(/select a bundle to configure its prompt overrides/i)).toBeInTheDocument();
    expect(screen.queryByText('Mining prompt override')).not.toBeInTheDocument();
  });
});

describe('SettingsScreen diagnostics export', () => {
  beforeEach(() => {
    __resetStoreForTests();
  });

  it('shows the saved path after a successful export', async () => {
    renderSettings();
    const button = await screen.findByRole('button', { name: /export diagnostics bundle/i });
    await userEvent.click(button);
    expect(await screen.findByText(/Saved to/)).toBeInTheDocument();
    expect(screen.getByText((content) => content.includes('sopkb-diagnostics.zip'))).toBeInTheDocument();
  });
});

describe('SettingsScreen one-click MCP client configuration', () => {
  beforeEach(() => {
    __resetStoreForTests();
    // jsdom has no real clipboard -- stub it so the "Copy"/"Copy setup snippet"
    // buttons' navigator.clipboard.writeText call doesn't throw mid-test.
    Object.defineProperty(navigator, 'clipboard', { value: { writeText: vi.fn().mockResolvedValue(undefined) }, configurable: true });
  });

  function rowFor(label: string) {
    return screen.getByText(label).closest('div.flex') as HTMLElement;
  }

  it('lists every known client and disables the button for one that could not be located', async () => {
    renderSettings();
    await screen.findByText('Claude Desktop');
    expect(screen.getByText('Claude Code')).toBeInTheDocument();
    expect(screen.getByText('Codex (CLI & Desktop)')).toBeInTheDocument();

    expect(screen.getByText(/could not find the `codex` cli/i)).toBeInTheDocument();
    const codexButton = within(rowFor('Codex (CLI & Desktop)')).getByRole('button', { name: 'Configure automatically' });
    expect(codexButton).toBeDisabled();
  });

  it('configures a located client after one explicit confirmation', async () => {
    renderSettings();
    await screen.findByText('Claude Desktop');

    await userEvent.click(within(rowFor('Claude Desktop')).getByRole('button', { name: 'Configure automatically' }));
    expect(await screen.findByText(/Configure Claude Desktop\?/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Configure' }));

    expect(await screen.findByText(/Added "sopkb-/)).toBeInTheDocument();
    expect(store.configuredMcpClients.has(`claude-desktop:${store.selectedBundle}`)).toBe(true);
  });

  it('escalates a name collision to a second, explicit overwrite confirmation', async () => {
    renderSettings();
    await screen.findByText('Claude Code');

    // First configure succeeds.
    await userEvent.click(within(rowFor('Claude Code')).getByRole('button', { name: 'Configure automatically' }));
    await userEvent.click(await screen.findByRole('button', { name: 'Configure' }));
    await screen.findByText(/Added "sopkb-/);

    // Second attempt hits the same name and must escalate rather than silently no-op or re-succeed.
    await userEvent.click(within(rowFor('Claude Code')).getByRole('button', { name: 'Configure automatically' }));
    await userEvent.click(await screen.findByRole('button', { name: 'Configure' }));
    expect(await screen.findByText(/Overwrite existing entry in Claude Code\?/)).toBeInTheDocument();
    expect(within(screen.getByRole('alertdialog')).getByText(/already configured/i)).toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: 'Overwrite' }));
    expect(await screen.findByText(/Added "sopkb-/)).toBeInTheDocument();
  });

  it('rescans on demand and picks up a client that becomes located, without remounting', async () => {
    renderSettings();
    await screen.findByText('Claude Desktop');
    expect(within(rowFor('Codex (CLI & Desktop)')).getByRole('button', { name: 'Configure automatically' })).toBeDisabled();

    // Simulate "the user just installed Codex" -- the panel must reflect this
    // only after an explicit Rescan, never on its own (that's the whole point
    // of caching detection instead of re-probing on every mount).
    store.mcpCodexLocated = true;
    await userEvent.click(screen.getByRole('button', { name: 'Rescan' }));

    await waitFor(() =>
      expect(within(rowFor('Codex (CLI & Desktop)')).getByRole('button', { name: 'Configure automatically' })).toBeEnabled(),
    );
  });

  it('copies a manual setup snippet for a client that could not be located, naming the same entry an automatic configure would use', async () => {
    renderSettings();
    await screen.findByText('Claude Desktop');

    await userEvent.click(within(rowFor('Codex (CLI & Desktop)')).getByRole('button', { name: 'Copy setup snippet' }));

    expect(navigator.clipboard.writeText).toHaveBeenCalledWith(expect.stringContaining(`sopkb-${store.selectedBundle}`));
    expect(await within(rowFor('Codex (CLI & Desktop)')).findByRole('button', { name: 'Copied' })).toBeInTheDocument();
  });
});
