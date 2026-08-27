import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it } from 'vitest';
import { OverviewScreen } from './OverviewScreen';
import { WorkbenchProvider } from '../context/WorkbenchContext';
import { __resetStoreForTests, store } from '../mock/store';

function renderOverview() {
  return render(
    <MemoryRouter>
      <WorkbenchProvider>
        <OverviewScreen />
      </WorkbenchProvider>
    </MemoryRouter>,
  );
}

describe('OverviewScreen Markdown rendering', () => {
  beforeEach(() => {
    __resetStoreForTests();
    store.selectedBundle = 'glp1-healthcare';
  });

  it('renders a Markdown heading and list as real HTML elements, not literal # / - text', async () => {
    const bundle = store.requireBundle();
    bundle.reportsMarkdown.extraction_summary = '# Freshness Report\n\n- one item\n- another item\n';

    renderOverview();

    const heading = await screen.findByRole('heading', { level: 1, name: 'Freshness Report' });
    expect(heading).toBeInTheDocument();
    expect(screen.getAllByRole('listitem')).toHaveLength(2);
    // The raw Markdown syntax must not appear as literal text anywhere.
    expect(screen.queryByText(/^#\s*Freshness Report/)).not.toBeInTheDocument();
  });

  it('sanitizes embedded HTML/script content rather than injecting it verbatim', async () => {
    const bundle = store.requireBundle();
    bundle.reportsMarkdown.extraction_summary =
      '# Report\n\n<img src=x onerror="window.__pwned = true">\n<script>window.__pwned = true</script>\n';

    renderOverview();

    await screen.findByRole('heading', { level: 1, name: 'Report' });
    expect(document.querySelector('script')).not.toBeInTheDocument();
    expect(document.querySelector('img[onerror]')).not.toBeInTheDocument();
    expect((window as unknown as { __pwned?: boolean }).__pwned).toBeUndefined();
  });
});

// Round 7, item 17: the new landing-dashboard half of this screen, aggregating
// commands other screens already call on their own (get_source_stats,
// list_knowledge_items, get_concept_index, get_validation_summary) -- no new
// backend command, so these tests are really about correct client-side
// aggregation and the tiles' links/actions, not new data plumbing.
describe('OverviewScreen stat tiles', () => {
  beforeEach(() => {
    __resetStoreForTests();
    store.selectedBundle = 'glp1-healthcare';
  });

  it('shows real counts for sources, knowledge items, and concepts, each linking to its own screen', async () => {
    renderOverview();
    const bundle = store.requireBundle();

    const sourcesTile = await screen.findByRole('link', { name: /sources/i });
    expect(sourcesTile).toHaveAttribute('href', '/sources');
    expect(within(sourcesTile).getByText(String(bundle.sources.length))).toBeInTheDocument();

    const knowledgeTile = screen.getByRole('link', { name: /knowledge items/i });
    expect(knowledgeTile).toHaveAttribute('href', '/knowledge');
    expect(within(knowledgeTile).getByText(String(bundle.items.length))).toBeInTheDocument();

    const conceptsTile = screen.getByRole('link', { name: /^concepts/i });
    expect(conceptsTile).toHaveAttribute('href', '/concepts');
  });

  it('clicking the Validation tile switches the report viewer to the Validation tab', async () => {
    const bundle = store.requireBundle();
    bundle.reportsMarkdown.validation = '# Validation Report\n';
    renderOverview();

    const validationTile = await screen.findByRole('button', { name: /overview stat: validation/i });
    await userEvent.click(validationTile);

    expect(await screen.findByRole('heading', { level: 1, name: 'Validation Report' })).toBeInTheDocument();
  });
});
