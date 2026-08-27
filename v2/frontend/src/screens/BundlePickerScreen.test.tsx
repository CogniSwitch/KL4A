import { render, screen, waitForElementToBeRemoved, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it } from 'vitest';
import { BundlePickerScreen } from './BundlePickerScreen';
import { WorkbenchProvider } from '../context/WorkbenchContext';
import { __resetStoreForTests, store } from '../mock/store';

function renderPicker() {
  return render(
    <MemoryRouter>
      <WorkbenchProvider>
        <BundlePickerScreen />
      </WorkbenchProvider>
    </MemoryRouter>,
  );
}

/** Cards render their directory key (always unique, always visible) as a `<p>` — use that to locate the card. */
function cardForKey(key: string): HTMLElement {
  return screen.getByText(key).closest('li')!;
}

async function openDeleteDialogFor(key: string) {
  const card = cardForKey(key);
  await userEvent.click(within(card).getByRole('button', { name: /delete/i }));
  return screen.findByRole('alertdialog');
}

/**
 * Three fixture cards exist: `glp1-healthcare` ("GLP-1 Healthcare Reference",
 * created 2026-01-01), `escalation-runbook` ("Reviewed Bundle Case", created
 * 2026-01-15), and the always-broken `legacy-intake-notes` (title equals its
 * key, created 2026-01-10) — enough real spread to exercise both sort orders.
 */
describe('BundlePickerScreen sort modes', () => {
  beforeEach(() => {
    __resetStoreForTests();
  });

  it('shows a sort control once bundles have loaded', async () => {
    renderPicker();
    await screen.findByText('glp1-healthcare');
    expect(screen.getByRole('combobox', { name: /sort/i })).toBeInTheDocument();
  });

  it('sorts alphabetically by title when selected', async () => {
    renderPicker();
    await screen.findByText('glp1-healthcare');
    await userEvent.selectOptions(screen.getByRole('combobox', { name: /sort/i }), 'Alphabetical (title)');

    const order = screen.getAllByRole('listitem').map((li) => li.textContent ?? '');
    // "GLP-1 Healthcare Reference" < "legacy-intake-notes" < "Reviewed Bundle Case".
    const posGlp1 = order.findIndex((t) => t.includes('GLP-1 Healthcare Reference'));
    const posLegacy = order.findIndex((t) => t.includes('legacy-intake-notes'));
    const posEscalation = order.findIndex((t) => t.includes('Reviewed Bundle Case'));
    expect(posGlp1).toBeLessThan(posLegacy);
    expect(posLegacy).toBeLessThan(posEscalation);
  });

  it('sorts by last created, most recent first', async () => {
    renderPicker();
    await screen.findByText('glp1-healthcare');
    await userEvent.selectOptions(screen.getByRole('combobox', { name: /sort/i }), 'Last created');

    const order = screen.getAllByRole('listitem').map((li) => li.textContent ?? '');
    // escalation-runbook (2026-01-15) newest, legacy-intake-notes (2026-01-10)
    // middle, glp1-healthcare (2026-01-01) oldest.
    const posEscalation = order.findIndex((t) => t.includes('escalation-runbook'));
    const posLegacy = order.findIndex((t) => t.includes('legacy-intake-notes'));
    const posGlp1 = order.findIndex((t) => t.includes('glp1-healthcare'));
    expect(posEscalation).toBeLessThan(posLegacy);
    expect(posLegacy).toBeLessThan(posGlp1);
  });

  it('reverses alphabetical order when the direction toggle is clicked, without affecting last-created', async () => {
    renderPicker();
    await screen.findByText('glp1-healthcare');
    await userEvent.selectOptions(screen.getByRole('combobox', { name: /sort/i }), 'Alphabetical (title)');
    await userEvent.click(screen.getByRole('button', { name: /sort ascending/i }));

    const order = screen.getAllByRole('listitem').map((li) => li.textContent ?? '');
    const posGlp1 = order.findIndex((t) => t.includes('GLP-1 Healthcare Reference'));
    const posEscalation = order.findIndex((t) => t.includes('Reviewed Bundle Case'));
    // Reversed from the ascending test above: "Reviewed Bundle Case" now precedes "GLP-1 Healthcare Reference".
    expect(posEscalation).toBeLessThan(posGlp1);

    // Switching to Last created must NOT carry over the "descending" direction just
    // set for Alphabetical -- it has its own remembered direction (defaults to
    // newest-first, i.e. descending, on its own terms).
    await userEvent.selectOptions(screen.getByRole('combobox', { name: /sort/i }), 'Last created');
    const orderByDate = screen.getAllByRole('listitem').map((li) => li.textContent ?? '');
    const posEscalationByDate = orderByDate.findIndex((t) => t.includes('escalation-runbook'));
    const posGlp1ByDate = orderByDate.findIndex((t) => t.includes('glp1-healthcare'));
    expect(posEscalationByDate).toBeLessThan(posGlp1ByDate);
  });

  it('toggles between grid and list view', async () => {
    renderPicker();
    await screen.findByText('glp1-healthcare');
    const list = screen.getAllByRole('listitem')[0]!.closest('ul')!;
    expect(list.className).toContain('grid');

    await userEvent.click(screen.getByRole('button', { name: 'List' }));
    expect(list.className).not.toContain('grid');

    await userEvent.click(screen.getByRole('button', { name: 'Grid' }));
    expect(list.className).toContain('grid');
  });
});

describe('BundlePickerScreen delete with confirmation', () => {
  beforeEach(() => {
    __resetStoreForTests();
  });

  it('does not delete on a single click — the confirm button stays disabled until the title is typed', async () => {
    renderPicker();
    await screen.findByText('glp1-healthcare');
    const dialog = await openDeleteDialogFor('glp1-healthcare');
    const confirmButton = within(dialog).getByRole('button', { name: /delete permanently/i });
    expect(confirmButton).toBeDisabled();

    await userEvent.click(confirmButton);
    expect(screen.getByText('glp1-healthcare')).toBeInTheDocument();
    expect(screen.getByRole('alertdialog')).toBeInTheDocument();
  });

  it('a mistyped confirmation still leaves the confirm button disabled', async () => {
    renderPicker();
    await screen.findByText('glp1-healthcare');
    const dialog = await openDeleteDialogFor('glp1-healthcare');
    await userEvent.type(within(dialog).getByRole('textbox'), 'wrong name');
    expect(within(dialog).getByRole('button', { name: /delete permanently/i })).toBeDisabled();
  });

  it('deletes the bundle once the exact title is typed and confirmed', async () => {
    renderPicker();
    await screen.findByText('glp1-healthcare');
    const dialog = await openDeleteDialogFor('glp1-healthcare');
    await userEvent.type(within(dialog).getByRole('textbox'), 'GLP-1 Healthcare Reference');
    await userEvent.click(within(dialog).getByRole('button', { name: /delete permanently/i }));

    // The dialog closes as soon as the delete call itself resolves; the list
    // reload it triggers is a separate, later-resolving fetch (`useAsync`'s
    // `reload()` is fire-and-forget, not awaitable), so the card's removal
    // must be waited for independently rather than assumed by this point.
    await waitForElementToBeRemoved(() => screen.queryByRole('alertdialog'));
    await waitForElementToBeRemoved(() => screen.queryByText('glp1-healthcare'));
  });

  it('cancel leaves the bundle untouched', async () => {
    renderPicker();
    await screen.findByText('glp1-healthcare');
    const dialog = await openDeleteDialogFor('glp1-healthcare');
    await userEvent.click(within(dialog).getByRole('button', { name: /cancel/i }));

    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
    expect(screen.getByText('glp1-healthcare')).toBeInTheDocument();
  });

  it('deselects the workbench selection when the currently-selected bundle is deleted', async () => {
    store.selectedBundle = 'glp1-healthcare';
    renderPicker();
    await screen.findByText('glp1-healthcare');
    const dialog = await openDeleteDialogFor('glp1-healthcare');
    await userEvent.type(within(dialog).getByRole('textbox'), 'GLP-1 Healthcare Reference');
    await userEvent.click(within(dialog).getByRole('button', { name: /delete permanently/i }));

    await waitForElementToBeRemoved(() => screen.queryByRole('alertdialog'));
    expect(store.selectedBundle).toBeUndefined();
  });

  it('surfaces an error and keeps the dialog open if delete fails', async () => {
    renderPicker();
    await screen.findByText('glp1-healthcare');
    const dialog = await openDeleteDialogFor('glp1-healthcare');
    await userEvent.type(within(dialog).getByRole('textbox'), 'GLP-1 Healthcare Reference');
    // Delete the bundle out from under the dialog to force the API call to
    // 404 on confirm, exercising the error path without mocking fetch.
    store.bundles.delete('glp1-healthcare');
    await userEvent.click(within(dialog).getByRole('button', { name: /delete permanently/i }));

    await screen.findByText(/could not delete bundle/i);
    expect(screen.getByRole('alertdialog')).toBeInTheDocument();
  });
});
