import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { beforeEach, describe, expect, it } from 'vitest';
import { ReviewScreen } from './ReviewScreen';
import { WorkbenchProvider } from '../context/WorkbenchContext';
import { __resetStoreForTests, store } from '../mock/store';

function renderReview(itemId: string) {
  return render(
    <MemoryRouter initialEntries={[`/review/${itemId}`]}>
      <WorkbenchProvider>
        <Routes>
          <Route path="/review/:itemId" element={<ReviewScreen />} />
        </Routes>
      </WorkbenchProvider>
    </MemoryRouter>,
  );
}

/**
 * Renders the real screen (not just the API layer) to prove the
 * `allowed_actions` gating actually reaches the buttons a user clicks —
 * the mock-API tests in `src/mock/api.mock.review.test.ts` cover the data
 * layer, this covers the component wiring on top of it.
 */
describe('ReviewScreen action gating', () => {
  beforeEach(() => {
    __resetStoreForTests();
    store.selectedBundle = 'glp1-healthcare';
  });

  it('enables approve/reject/defer/edit for a proposed item, and comment once rationale is typed', async () => {
    renderReview('ki-follow-up-monitoring-procedure-38ef67176baf-000001');
    const approve = await screen.findByRole('button', { name: 'Approve' });
    expect(approve).toBeEnabled();
    expect(screen.getByRole('button', { name: 'Reject' })).toBeEnabled();
    expect(screen.getByRole('button', { name: 'Defer' })).toBeEnabled();
    // Comment requires real text — it must not be clickable with a blank rationale
    // (a blank click used to silently post the literal placeholder "comment via UI").
    const comment = screen.getByRole('button', { name: 'Comment' });
    expect(comment).toBeDisabled();
    await userEvent.type(screen.getByPlaceholderText('Rationale…'), 'looks fine');
    expect(comment).toBeEnabled();
    // Editable — at least one inline "Edit" affordance is present on the detail card.
    expect(screen.getAllByRole('button', { name: 'Edit' }).length).toBeGreaterThan(0);
  });

  it('disables approve/reject/defer/edit for an already-approved item, keeping only comment reachable', async () => {
    store.selectedBundle = 'escalation-runbook';
    renderReview('ki-escalation-runbook-096d9a2e502e-000001');
    await waitFor(() => expect(screen.getByText('approved')).toBeInTheDocument());
    expect(screen.getByRole('button', { name: 'Approve' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Reject' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Defer' })).toBeDisabled();
    // No inline "Edit" affordance anywhere on the detail card for a terminal item.
    expect(screen.queryAllByRole('button', { name: 'Edit' })).toHaveLength(0);
    await userEvent.type(screen.getByPlaceholderText('Rationale…'), 'still worth noting');
    expect(screen.getByRole('button', { name: 'Comment' })).toBeEnabled();
  });

  it('edits a field inline from the detail card', async () => {
    renderReview('ki-follow-up-monitoring-procedure-38ef67176baf-000001');
    await screen.findByRole('button', { name: 'Approve' });

    // Inline edit: click "Edit" next to Subject specifically (there are several
    // Edit affordances on the card, one per editable field).
    // `dt`'s immediate parent is the label+Edit-button flex row; its own parent is
    // the whole field wrapper (label row + display/edit value) — that's the scope
    // we need so "Save"/the input are both reachable from `subjectRow`.
    const subjectRow = screen.getByText('Subject').closest('div')!.parentElement!;
    await userEvent.click(within(subjectRow).getByRole('button', { name: 'Edit' }));
    const subjectInput = within(subjectRow).getByRole('textbox');
    await userEvent.clear(subjectInput);
    await userEvent.type(subjectInput, 'A corrected subject');
    await userEvent.click(within(subjectRow).getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(screen.getByText('Saved.')).toBeInTheDocument());
    expect(screen.queryByRole('button', { name: 'Save' })).not.toBeInTheDocument();
  });

  it('shows a success message once an action completes', async () => {
    renderReview('ki-follow-up-monitoring-procedure-38ef67176baf-000001');
    const approveButton = await screen.findByRole('button', { name: 'Approve' });
    await userEvent.click(approveButton);
    await waitFor(() => expect(screen.getByText('Approved.')).toBeInTheDocument());
  });
});
