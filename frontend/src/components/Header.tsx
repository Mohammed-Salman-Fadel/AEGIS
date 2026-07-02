// Top header bar with session info, mode selector, status, and metrics toggle
import { Activity, Bot, Cpu, GraduationCap } from 'lucide-react';
import type { ChatMode } from '../types';
import { useT } from '../lib/i18n';

interface HeaderProps {
  isDark: boolean;
  activeSessionTitle?: string;
  activeSessionId: string | null;
  chatMode: ChatMode;
  isMetricsOpen: boolean;
  status: string;
  onSetChatMode: (mode: ChatMode) => void;
  onToggleMetrics: () => void;
}

export function Header({
  isDark, activeSessionTitle, activeSessionId, chatMode,
  isMetricsOpen, status, onSetChatMode,   onToggleMetrics,
}: HeaderProps) {
  const t = useT();
  return (
    <header className={`grid h-16 shrink-0 grid-cols-3 items-center border-b px-6 ${isDark ? 'border-zinc-800' : 'border-stone-300'}`}>
      <div className="flex min-w-0 items-center gap-3">
        <div className="min-w-0">
          <div className="truncate text-sm font-medium">{activeSessionTitle ?? t('header.new_chat')}</div>
          <div className={`truncate text-xs ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>
            Session: {activeSessionId ?? t('header.not_started')}
          </div>
        </div>
      </div>

      <div className="flex items-center justify-center gap-1 rounded-xl border border-zinc-800/50 bg-zinc-900/30 p-1 shadow-inner backdrop-blur-sm justify-self-center">
        {(['general', 'coder', 'academic'] as ChatMode[]).map((mode) => {
          const Icon = mode === 'general' ? Bot : mode === 'coder' ? Cpu : GraduationCap;
          return (
            <button
              key={mode}
              className={`flex items-center gap-2 rounded-lg px-3 py-1.5 text-xs font-medium transition-all ${chatMode === mode ? 'aegis-accent-chip-active text-white' : 'text-zinc-400 hover:bg-zinc-800/50 hover:text-zinc-200'}`}
              onClick={() => onSetChatMode(mode)}
              type="button"
            >
              <Icon size={14} />
              {mode.charAt(0).toUpperCase() + mode.slice(1)}
            </button>
          );
        })}
      </div>

      <div className="flex items-center gap-3 justify-self-end">
        <button
          className={`aegis-accent-ghost inline-flex items-center gap-2 rounded-lg border px-3 py-2 text-xs font-medium transition ${isMetricsOpen ? 'aegis-accent-subtle' : isDark ? 'border-zinc-800 text-zinc-300 hover:bg-zinc-900' : 'border-stone-300 bg-white text-slate-700 hover:bg-stone-100'}`}
          onClick={onToggleMetrics}
          type="button"
        >
          <Activity size={14} />
          {t('metrics.live_stats')}
        </button>
        <div className={`rounded-lg border px-3 py-1 text-xs ${isDark ? 'border-zinc-800 text-zinc-400' : 'border-stone-300 bg-white text-slate-500'}`}>
          {status}
        </div>
      </div>
    </header>
  );
}
