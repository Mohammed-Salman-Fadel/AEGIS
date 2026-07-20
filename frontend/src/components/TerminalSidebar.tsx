import { PanelRightClose, TerminalSquare } from 'lucide-react';

interface TerminalSidebarProps {
  isDark: boolean;
  isOpen: boolean;
  provider?: string;
  model?: string;
  sessionId: string | null;
  status: string;
  onClose: () => void;
}

export function TerminalSidebar({
  isDark,
  isOpen,
  provider,
  model,
  sessionId,
  status,
  onClose,
}: TerminalSidebarProps) {
  return (
    <aside
      aria-label="AEGIS CLI"
      className={`flex shrink-0 flex-col overflow-hidden border-l transition-[width,opacity,transform] duration-300 ease-in-out max-md:fixed max-md:inset-y-0 max-md:right-0 max-md:z-40 ${
        isOpen ? 'w-[min(26rem,92vw)] opacity-100' : 'w-0 border-transparent opacity-0 max-md:translate-x-full'
      } aegis-bg-surface aegis-border-subtle`}
    >
      <div className={`flex min-w-[min(26rem,92vw)] flex-1 flex-col ${isOpen ? '' : 'pointer-events-none'}`}>
        <div className="flex h-16 shrink-0 items-center justify-between border-b px-5 aegis-border-subtle">
          <div className="flex items-center gap-2">
            <TerminalSquare className="text-emerald-500" size={16} />
            <span className="aegis-display text-[12px] font-medium uppercase tracking-[0.08em] aegis-text-muted">AEGIS CLI</span>
          </div>
          <button
            aria-label="Close terminal"
            className={`rounded-md p-1 transition ${isDark ? 'text-zinc-500 hover:bg-zinc-900 hover:text-zinc-300' : 'text-stone-500 hover:bg-[rgba(94,76,55,0.1)] hover:text-stone-800'}`}
            onClick={onClose}
            type="button"
          >
            <PanelRightClose size={16} />
          </button>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto p-5">
          <div className={`rounded-xl border p-4 font-mono text-[12px] leading-6 ${isDark ? 'border-zinc-800 bg-zinc-950/70 text-zinc-300' : 'border-[rgba(94,76,55,0.24)] bg-[rgba(255,250,240,0.78)] text-stone-700'}`}>
            <div className="text-emerald-500">$ aegis status</div>
            <div className={isDark ? 'text-zinc-500' : 'text-stone-500'}>AEGIS CLI v0.1.0</div>
            <div>engine: <span className="text-emerald-500">{status}</span></div>
            <div>provider: {provider ?? 'detecting'}</div>
            <div className="break-all">model: {model ?? 'not selected'}</div>
            <div className="break-all">session: {sessionId ?? 'not started'}</div>
          </div>
          <p className={`mt-4 text-xs leading-5 ${isDark ? 'text-zinc-500' : 'text-stone-500'}`}>
            This panel mirrors the local AEGIS CLI context. Use your system terminal to run interactive CLI commands.
          </p>
        </div>
      </div>
    </aside>
  );
}
