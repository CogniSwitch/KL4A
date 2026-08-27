import { Component, type ReactNode } from 'react';
import { Button } from './Button';

interface Props {
  children: ReactNode;
}

interface State {
  error: Error | null;
}

/**
 * Catches a render-time exception in whatever screen is mounted below it so it
 * degrades to this panel instead of unmounting the entire app to a blank white
 * window (React's default for an uncaught error during render). Scoped inside
 * `AppShell`'s `<main>` in `App.tsx`, not around the whole app, so the sidebar
 * nav stays mounted and clickable -- the user can navigate away from whatever
 * broke without restarting the app. `App.tsx` remounts this with a
 * `key={pathname}` so navigating to a different route clears the error state
 * instead of it sticking around for the life of the process.
 */
export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: { componentStack: string }) {
    console.error('Unhandled error in screen render:', error, info.componentStack);
  }

  render() {
    if (!this.state.error) return this.props.children;
    return (
      <section className="rounded-lg border border-bad/30 bg-bad-soft p-6 text-sm text-bad">
        <h1 className="text-base font-semibold">This screen hit an unexpected error</h1>
        <p className="mt-2 opacity-90">
          Nothing else was affected -- use the sidebar to go to a different screen, or try this one again.
        </p>
        <pre className="mt-3 overflow-x-auto rounded-lg bg-bad/10 px-3 py-2 text-xs">{this.state.error.message}</pre>
        <div className="mt-4">
          <Button variant="secondary" onClick={() => this.setState({ error: null })}>
            Try this screen again
          </Button>
        </div>
      </section>
    );
  }
}
