import { render, screen, within } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { beforeEach, describe, expect, it } from 'vitest';
import { ViewerScreen } from './ViewerScreen';
import { WorkbenchProvider } from '../context/WorkbenchContext';
import { __resetStoreForTests, store } from '../mock/store';

function renderViewer(sourceId: string) {
  return render(
    <MemoryRouter initialEntries={[`/sources/${sourceId}`]}>
      <WorkbenchProvider>
        <Routes>
          <Route path="/sources/:sourceId" element={<ViewerScreen />} />
        </Routes>
      </WorkbenchProvider>
    </MemoryRouter>,
  );
}

const SOURCE_ID = 'follow-up-monitoring-procedure-38ef67176baf';

/** Scopes to the `<section>` under the "Sections (n)"/"Evidence (n)" heading — several section headings and item subjects/`Section` cells share literal text (e.g. "Dose Titration" is both a section heading and a subject), so document-wide queries are ambiguous. */
function panelFor(headingText: string): HTMLElement {
  return screen.getByText(headingText).closest('section')!;
}

/**
 * Section TOC and per-source evidence list — see CATCHUP_PLAN.md's
 * 2026-08-22 sections-view research. This source in the `glp1-healthcare`
 * fixture has 3 sections (40/194/178 chars) and 4 mined knowledge items
 * split across the latter two.
 */
describe('ViewerScreen sections and evidence', () => {
  beforeEach(() => {
    __resetStoreForTests();
    store.selectedBundle = 'glp1-healthcare';
  });

  it('lists this source\'s sections with heading, role badge, and char span', async () => {
    renderViewer(SOURCE_ID);
    await screen.findByText('Sections (3)');
    const toc = panelFor('Sections (3)');

    const titleItem = within(toc).getByText('GLP-1 Follow-up Monitoring Procedure').closest('li')!;
    expect(within(titleItem).getByText('procedure')).toBeInTheDocument();
    expect(within(titleItem).getByText('40 chars')).toBeInTheDocument();

    const doseTitrationItem = within(toc).getByText('Dose Titration').closest('li')!;
    expect(within(doseTitrationItem).getByText('178 chars')).toBeInTheDocument();
  });

  it('does not show sections belonging to a different source', async () => {
    renderViewer(SOURCE_ID);
    await screen.findByText('Sections (3)');
    expect(screen.queryByText('Intake Requirements')).not.toBeInTheDocument();
  });

  it('lists every knowledge item mined from this source, with a Section column cross-referencing section_id', async () => {
    renderViewer(SOURCE_ID);
    await screen.findByText('Evidence (4)');
    const evidence = panelFor('Evidence (4)');

    const rows = within(evidence)
      .getAllByRole('row')
      .filter((r) => within(r).queryAllByRole('cell').length > 0);
    expect(rows).toHaveLength(4);

    const doseRow = within(evidence)
      .getByText('Clinicians must confirm patient tolerance before dose escalation.')
      .closest('tr')!;
    const cells = within(doseRow).getAllByRole('cell');
    expect(cells[0].textContent).toBe('Dose Titration'); // Subject
    expect(cells[1].textContent).toBe('Dose Titration'); // Section (cross-referenced via section_id)
    expect(within(doseRow).getByText('proposed')).toBeInTheDocument();
  });

  it('does not show knowledge items mined from a different source', async () => {
    renderViewer(SOURCE_ID);
    await screen.findByText('Evidence (4)');
    // "Intake Requirements" items belong to the primary-care source, not this one.
    expect(
      screen.queryByText('Clinicians must confirm patient identity before reviewing GLP-1 therapy eligibility.'),
    ).not.toBeInTheDocument();
  });
});
