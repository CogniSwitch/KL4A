import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it } from 'vitest';
import { AppShell } from './AppShell';
import { WorkbenchProvider } from '../../context/WorkbenchContext';
import { __resetStoreForTests, store } from '../../mock/store';

function renderShell() {
  return render(
    <MemoryRouter>
      <WorkbenchProvider>
        <AppShell>
          <div>content</div>
        </AppShell>
      </WorkbenchProvider>
    </MemoryRouter>,
  );
}

// Bundles/Settings live in a fixed footer below the scrollable, bundle-scoped
// nav (round 7, item 18) -- this is this file's first-ever test, added
// alongside that reorg since there was previously no safety net at all.
describe('AppShell sidebar', () => {
  beforeEach(() => {
    __resetStoreForTests();
  });

  it('always shows Bundles and Settings, even with no bundle selected', async () => {
    renderShell();
    expect(await screen.findByRole('link', { name: /bundles/i })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: /settings/i })).toBeInTheDocument();
  });

  it('hides the phase-grouped nav when no bundle is selected', async () => {
    renderShell();
    await screen.findByRole('link', { name: /bundles/i });
    expect(screen.queryByRole('link', { name: /ingest/i })).not.toBeInTheDocument();
  });

  it('shows the phase-grouped nav once a bundle is selected', async () => {
    store.selectedBundle = 'glp1-healthcare';
    renderShell();
    expect(await screen.findByRole('link', { name: /ingest/i })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: /sources/i })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: /knowledge/i })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: /bundles/i })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: /settings/i })).toBeInTheDocument();
  });
});
