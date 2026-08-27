import type { ReactNode } from 'react';
import { NavLink } from 'react-router-dom';
import { useWorkbench } from '../../context/WorkbenchContext';

/**
 * Sidebar nav grouped by workflow phase, each item with a small stroke
 * icon. Grouping + icons are ported as a *pattern* (not code) from
 * `origin/ui/sopkb-web-redesign`'s `tools/sopkb/sopkb/web_app.py`
 * (`NAV_GROUPS`/`NAV_ICON_PATHS`, ~L598-635) — a restyle of a *different*,
 * older Python server-rendered companion UI, not this React app. Icon
 * paths for ingest/sources/knowledge/concepts/agent/reports/bundles/
 * settings are copied verbatim from that file's `NAV_ICON_PATHS` (same
 * 20x20 viewBox / 1.6 stroke-width conventions), since they're bare SVG
 * path data, not framework-specific code.
 *
 * `Review` and `Viewer` aren't top-level nav destinations here (drill-down
 * routes only, reached from Knowledge and Sources respectively), so
 * `Govern` only has Agent and `Build` only has Ingest/Sources. `Export` is
 * gone too — OKF generation already happens automatically on every ingest
 * run and review action (`sync_okf_bundle`), so there was nothing left for
 * a dedicated export step to do; "reveal the bundle folder" and "force a
 * resync" moved onto Sources as secondary actions instead of a whole page.
 * `Overview` (`reports`) is pinned above the phase groups, not inside one —
 * it's the bundle's dashboard/landing view, not a workflow step, so it
 * doesn't belong to "Build"/"Understand"/"Govern" the way the others do; a
 * lone one-item "Ship" group added a label without adding meaning.
 */
type NavName = 'ingest' | 'sources' | 'knowledge' | 'concepts' | 'agent' | 'reports';

const NAV_ITEMS: Record<NavName, { to: string; label: string }> = {
  ingest: { to: '/ingest', label: 'Ingest' },
  sources: { to: '/sources', label: 'Sources' },
  knowledge: { to: '/knowledge', label: 'Knowledge' },
  concepts: { to: '/concepts', label: 'Concepts' },
  agent: { to: '/agent', label: 'Agent' },
  reports: { to: '/reports', label: 'Overview' },
};

const NAV_GROUPS: { label: string; items: NavName[] }[] = [
  { label: 'Build', items: ['ingest', 'sources'] },
  { label: 'Understand', items: ['knowledge', 'concepts'] },
  { label: 'Govern', items: ['agent'] },
];

const ICON_PATHS: Record<NavName | 'bundles' | 'settings', ReactNode> = {
  ingest: (
    <>
      <path d="M10 3v10M10 3l-4 4M10 3l4 4" />
      <path d="M4 15h12" />
    </>
  ),
  sources: (
    <path d="M3 6.5A1.5 1.5 0 0 1 4.5 5h3l1.6 1.8H15A1.5 1.5 0 0 1 16.5 8.3V14a1.5 1.5 0 0 1-1.5 1.5H4.5A1.5 1.5 0 0 1 3 14z" />
  ),
  knowledge: <path d="M4.5 4.5h11M4.5 10h11M4.5 15.5h7" />,
  concepts: (
    <>
      <path d="M10.2 3H5.8A1.8 1.8 0 0 0 4 4.8v4.4c0 .4.2.9.5 1.2l6.1 6.1a1.8 1.8 0 0 0 2.6 0l3.7-3.7a1.8 1.8 0 0 0 0-2.6L11 4.5a1.8 1.8 0 0 0-1.2-.5z" />
      <circle cx="7.2" cy="7.2" r="1" />
    </>
  ),
  agent: <path d="M4 5.5h12v7.5H8.5L5 16v-3H4z" />,
  reports: (
    <>
      <path d="M5.5 3h6l3 3v11h-9z" />
      <path d="M11.5 3v3h3" />
      <path d="M7.5 10.5h5M7.5 13.5h5" />
    </>
  ),
  bundles: (
    <>
      <rect x="3" y="3" width="6" height="6" rx="1.2" />
      <rect x="11" y="3" width="6" height="6" rx="1.2" />
      <rect x="3" y="11" width="6" height="6" rx="1.2" />
      <rect x="11" y="11" width="6" height="6" rx="1.2" />
    </>
  ),
  settings: (
    <>
      <circle cx="10" cy="10" r="2.4" />
      <path d="M10 2.8v2.1M10 15.1v2.1M17.2 10h-2.1M4.9 10H2.8M15.3 4.7l-1.5 1.5M6.2 13.8l-1.5 1.5M15.3 15.3l-1.5-1.5M6.2 6.2 4.7 4.7" />
    </>
  ),
};

function NavIcon({ name }: { name: NavName | 'bundles' | 'settings' }) {
  return (
    <svg
      viewBox="0 0 20 20"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.6}
      strokeLinecap="round"
      strokeLinejoin="round"
      className="h-4 w-4 shrink-0"
      aria-hidden="true"
    >
      {ICON_PATHS[name]}
    </svg>
  );
}

function navClass(isActive: boolean) {
  return `flex items-center gap-2 rounded-lg border-l-2 px-3 py-1.5 text-sm transition-colors ${
    isActive
      ? 'border-accent bg-white/10 font-medium text-white'
      : 'border-transparent text-sidebar-muted hover:bg-white/5 hover:text-sidebar-ink'
  }`;
}

export function AppShell({ children }: { children: ReactNode }) {
  const { context } = useWorkbench();
  const hasBundle = Boolean(context?.selected_bundle);

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-bg text-ink">
      <aside className="flex w-56 shrink-0 flex-col bg-sidebar text-sidebar-ink">
        <div className="border-b border-white/10 px-4 py-4">
          {/*
           * "Anchor Stitch" mark, P5 Deep Indigo palette — reuses the P2 lockup's
           * exact geometry (circle-dashed line-circle+dot-short line), recolored
           * per the brand package's own guidance that a plate-free, single-ink
           * mark (cream `#E8DEC0`, gold `#D4A857` accent) is the intended way to
           * drop this directly onto an already-dark surface like this sidebar,
           * without its own background chip.
           */}
          <div className="flex items-center gap-2.5">
            <svg viewBox="0 0 64 64" width="28" height="28" aria-hidden="true" className="shrink-0">
              <circle cx="32" cy="12" r="5" fill="none" stroke="#E8DEC0" strokeWidth="2.6" />
              <line x1="32" y1="17" x2="32" y2="42" stroke="#E8DEC0" strokeWidth="2.8" strokeLinecap="round" strokeDasharray="6 5" />
              <circle cx="32" cy="48" r="6" fill="none" stroke="#E8DEC0" strokeWidth="3" />
              <circle cx="32" cy="48" r="1.8" fill="#D4A857" />
              <line x1="26" y1="58" x2="38" y2="58" stroke="#D4A857" strokeWidth="2.8" strokeLinecap="round" />
            </svg>
            <p className="text-sm font-semibold text-white">
              <span className="font-display">
                KL<span style={{ color: '#D4A857' }}>4</span>A
              </span>
              <span className="ml-1.5 font-mono text-xs uppercase tracking-wide text-sidebar-muted">Workbench</span>
            </p>
          </div>
          {context && (
            <p className="mt-0.5 truncate text-xs text-sidebar-muted" title={context.root}>
              {context.mode === 'SingleBundle' ? 'Single bundle' : 'All bundles'}
            </p>
          )}
        </div>
        <nav className="flex-1 space-y-4 overflow-y-auto px-3 py-4">
          {hasBundle && (
            <div className="space-y-4">
              <div className="space-y-1">
                <NavLink to={NAV_ITEMS.reports.to} className={({ isActive }) => navClass(isActive)}>
                  <NavIcon name="reports" />
                  <span>{NAV_ITEMS.reports.label}</span>
                </NavLink>
              </div>
              <p className="px-3 text-xs font-semibold uppercase tracking-wide text-sidebar-muted">
                {context?.selected_bundle}
              </p>
              {NAV_GROUPS.map((group) => (
                <div key={group.label}>
                  <p className="px-3 text-[10px] font-semibold uppercase tracking-wider text-sidebar-muted/70">
                    {group.label}
                  </p>
                  <div className="mt-1 space-y-1">
                    {group.items.map((name) => {
                      const item = NAV_ITEMS[name];
                      return (
                        <NavLink key={item.to} to={item.to} className={({ isActive }) => navClass(isActive)}>
                          <NavIcon name={name} />
                          <span>{item.label}</span>
                        </NavLink>
                      );
                    })}
                  </div>
                </div>
              ))}
            </div>
          )}
        </nav>
        {/*
         * Bundles/Settings pinned to the bottom of the sidebar (not part of
         * NAV_GROUPS) -- a common bottom-left utility-nav convention (VS Code,
         * Slack, Notion), and separated from the scrollable phase-grouped nav
         * above by a divider matching the header's own `border-b`. Bundles
         * above Settings: Bundles answers "where am I" (context), Settings
         * answers "configure the app" (an action on that context), with
         * Settings as the very last item matching the usual bottom-most-gear
         * placement. Always visible regardless of `hasBundle`, unchanged from
         * before.
         */}
        <div className="shrink-0 space-y-1 border-t border-white/10 px-3 py-3">
          <NavLink to="/" end className={({ isActive }) => navClass(isActive)}>
            <NavIcon name="bundles" />
            <span>Bundles</span>
          </NavLink>
          <NavLink to="/settings" className={({ isActive }) => navClass(isActive)}>
            <NavIcon name="settings" />
            <span>Settings</span>
          </NavLink>
        </div>
      </aside>
      <main className="flex-1 overflow-y-auto bg-bg">
        <div className="mx-auto max-w-5xl px-8 py-8">{children}</div>
      </main>
    </div>
  );
}
