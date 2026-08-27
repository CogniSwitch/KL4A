import { render, screen, within } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it } from 'vitest';
import { KnowledgeScreen } from './KnowledgeScreen';
import { WorkbenchProvider } from '../context/WorkbenchContext';
import { __resetStoreForTests, store } from '../mock/store';

function renderKnowledge() {
  return render(
    <MemoryRouter>
      <WorkbenchProvider>
        <KnowledgeScreen />
      </WorkbenchProvider>
    </MemoryRouter>,
  );
}

/**
 * Section coverage / knowledge-gap visualization — ported from oss-launch's
 * EvidencePanel "Section coverage" block. jt-dev had no equivalent before
 * this (confirmed by grepping for `list_sections`/`get_section` usage
 * outside `lib/api.ts`/`mock/api.mock.ts`, which turned up nothing).
 *
 * The `glp1-healthcare` fixture bundle has 13 sections; every item covers
 * one of 9 distinct section ids — the 4 uncovered ones are each source's
 * top-level title section (`...-001`), which holds only the document
 * heading and never gets a knowledge item mined from it.
 */
describe('KnowledgeScreen section coverage', () => {
  beforeEach(() => {
    __resetStoreForTests();
    store.selectedBundle = 'glp1-healthcare';
  });

  it('shows the coverage percentage computed from sections vs. covered section ids', async () => {
    renderKnowledge();
    expect(await screen.findByText('Section coverage')).toBeInTheDocument();
    expect(await screen.findByText('69% of sections (9 / 13) have at least one knowledge item.')).toBeInTheDocument();
  });

  it('lists exactly the four uncovered sections under the gap heading', async () => {
    renderKnowledge();
    await screen.findByText('Section coverage');
    const gapHeading = screen.getByText(/Sections with no knowledge item \(4\)/);
    // Scoped to the gap list itself (its <ul> is the heading's next sibling) —
    // several gap headings are substrings of the source titles/knowledge-item
    // subjects rendered elsewhere on this screen, so an unscoped query would
    // be ambiguous.
    const gapList = gapHeading.nextElementSibling as HTMLElement;
    const gapItems = within(gapList).getAllByRole('listitem');
    expect(gapItems).toHaveLength(4);
    const gapText = gapItems.map((li) => li.textContent);
    expect(gapText.some((t) => t?.includes('GLP-1 Follow-up Monitoring Procedure'))).toBe(true);
    expect(gapText.some((t) => t?.includes('Primary Care GLP-1 Prescribing SOP'))).toBe(true);
    expect(gapText.some((t) => t?.includes('GLP-1 Prior Authorization Workflow'))).toBe(true);
    expect(gapText.some((t) => t?.includes('GLP-1 Safety Policy'))).toBe(true);
    // A covered section's heading must not appear in the gap list.
    expect(gapText.some((t) => t?.includes('Intake Requirements'))).toBe(false);
  });
});
