import { Component, type ErrorInfo, type ReactNode } from 'react';

interface AppErrorBoundaryProps {
  children: ReactNode;
}

interface AppErrorBoundaryState {
  failed: boolean;
}

export class AppErrorBoundary extends Component<AppErrorBoundaryProps, AppErrorBoundaryState> {
  state: AppErrorBoundaryState = { failed: false };

  static getDerivedStateFromError(): AppErrorBoundaryState {
    return { failed: true };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('AEGIS UI render failed', error, info.componentStack);
  }

  render() {
    if (!this.state.failed) return this.props.children;

    return (
      <main className="flex min-h-screen items-center justify-center bg-stone-100 px-6 text-stone-950">
        <section className="w-full max-w-md rounded-2xl border border-stone-300 bg-white p-6 shadow-xl shadow-stone-900/10">
          <p className="text-xs font-semibold uppercase tracking-[0.18em] text-emerald-700">AEGIS UI</p>
          <h1 className="mt-3 text-xl font-semibold">The interface hit an unexpected error</h1>
          <p className="mt-2 text-sm leading-6 text-stone-600">
            Your conversation remains stored locally. Reload the interface to reconnect without losing the session.
          </p>
          <button
            className="mt-5 rounded-xl bg-emerald-700 px-4 py-2.5 text-sm font-semibold text-white transition hover:bg-emerald-600"
            onClick={() => window.location.reload()}
            type="button"
          >
            Reload AEGIS
          </button>
        </section>
      </main>
    );
  }
}
