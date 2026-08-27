import { Navigate, Route, Routes, useLocation } from 'react-router-dom';
import { WorkbenchProvider, useWorkbench } from './context/WorkbenchContext';
import { IngestRunProvider } from './context/IngestRunContext';
import { AppShell } from './components/layout/AppShell';
import { LoadingBlock } from './components/common/Feedback';
import { ErrorBoundary } from './components/common/ErrorBoundary';
import { DegradedScreen } from './screens/DegradedScreen';
import { SettingsScreen } from './screens/SettingsScreen';
import { BundlePickerScreen } from './screens/BundlePickerScreen';
import { SourcesScreen } from './screens/SourcesScreen';
import { ViewerScreen } from './screens/ViewerScreen';
import { KnowledgeScreen } from './screens/KnowledgeScreen';
import { ConceptsScreen } from './screens/ConceptsScreen';
import { ConceptDetailScreen } from './screens/ConceptDetailScreen';
import { OverviewScreen } from './screens/OverviewScreen';
import { AgentScreen } from './screens/AgentScreen';
import { ReviewScreen } from './screens/ReviewScreen';
import { IngestScreen } from './screens/IngestScreen';

/** Redirects bundle-scoped routes back to the picker when nothing is selected. */
function RequireBundle({ children }: { children: React.ReactNode }) {
  const { context } = useWorkbench();
  if (!context?.selected_bundle) return <Navigate to="/" replace />;
  return <>{children}</>;
}

function AppRoutes() {
  return (
    <Routes>
      <Route path="/" element={<BundlePickerScreen />} />
      <Route path="/settings" element={<SettingsScreen />} />
      <Route path="/sources" element={<RequireBundle><SourcesScreen /></RequireBundle>} />
      <Route path="/sources/:sourceId" element={<RequireBundle><ViewerScreen /></RequireBundle>} />
      <Route path="/knowledge" element={<RequireBundle><KnowledgeScreen /></RequireBundle>} />
      <Route path="/review/:itemId" element={<RequireBundle><ReviewScreen /></RequireBundle>} />
      <Route path="/concepts" element={<RequireBundle><ConceptsScreen /></RequireBundle>} />
      <Route path="/concepts/:conceptId" element={<RequireBundle><ConceptDetailScreen /></RequireBundle>} />
      <Route path="/reports" element={<RequireBundle><OverviewScreen /></RequireBundle>} />
      <Route path="/agent" element={<RequireBundle><AgentScreen /></RequireBundle>} />
      <Route path="/ingest" element={<RequireBundle><IngestScreen /></RequireBundle>} />
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  );
}

function Shell() {
  const { context, loading, loadError } = useWorkbench();
  const location = useLocation();

  if (loading) {
    return (
      <div className="flex h-screen w-screen items-center justify-center">
        <LoadingBlock label="Opening workbench…" />
      </div>
    );
  }

  if (loadError || context?.mode === 'Degraded') {
    return <DegradedScreen />;
  }

  return (
    <AppShell>
      {/* `key={location.pathname}` remounts the boundary (clearing any caught
          error) on navigation, so leaving a broken screen via the sidebar --
          the recovery path this boundary exists to offer -- actually recovers
          instead of the fallback panel following the user to the next route. */}
      <ErrorBoundary key={location.pathname}>
        <AppRoutes />
      </ErrorBoundary>
    </AppShell>
  );
}

function App() {
  return (
    <WorkbenchProvider>
      <IngestRunProvider>
        <Shell />
      </IngestRunProvider>
    </WorkbenchProvider>
  );
}

export default App;
