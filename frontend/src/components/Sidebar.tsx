// Sidebar with session list and project management
import {
  MessageSquare, ChevronDown, FolderPlus, FolderOpen, X, Edit3, Download, Pin, Trash2, MoreHorizontal, Search,
} from 'lucide-react';
import { useDeferredValue, useState } from 'react';
import type { EngineSessionSummary, CodeProject } from '../types';
import { formatSessionLastAccessed } from '../lib';
import { useT } from '../lib/i18n';

interface SidebarProps {
  isDark: boolean;
  isStreaming: boolean;
  scanningProject: boolean;
  sidebarOpen: boolean;
  projectsOpen: boolean;
  sessionsOpen: boolean;
  codeProjects: CodeProject[];
  activeProjectId: string | null;
  sessions: EngineSessionSummary[];
  sortedSessions: EngineSessionSummary[];
  activeSessionId: string | null;
  editingSessionId: string | null;
  editingTitle: string;
  deletingSessionIds: string[];
  newSessionPulseId: string | null;
  pinnedSessionIdSet: Set<string>;
  sessionMenuOpenId: string | null;
  onToggleSidebar: () => void;
  onToggleProjects: () => void;
  onToggleSessions: () => void;
  onNewSession: () => void;
  onAddProject: () => void;
  onSelectProject: (id: string) => void;
  onRemoveProject: (id: string) => void;
  onSelectSession: (id: string) => void;
  onBeginRenaming: (session: EngineSessionSummary) => void;
  onSubmitRenaming: (session: EngineSessionSummary) => void;
  onCancelRenaming: () => void;
  onEditingTitleChange: (value: string) => void;
  onExportSession: (session: EngineSessionSummary) => void;
  onTogglePinned: (id: string) => void;
  onDeleteSession: (session: EngineSessionSummary) => void;
  onSetSessionMenuOpen: (id: string | null) => void;
  staleProjectIds: Set<string>;
}

export function Sidebar({
  isDark, isStreaming, scanningProject, sidebarOpen, projectsOpen, sessionsOpen,
  codeProjects, activeProjectId, sessions, sortedSessions, activeSessionId,
  editingSessionId, editingTitle, deletingSessionIds, newSessionPulseId,
  pinnedSessionIdSet, sessionMenuOpenId,
 onToggleSidebar, onToggleProjects, onToggleSessions, onNewSession, onAddProject,
 onSelectProject, onRemoveProject, onSelectSession, onBeginRenaming, onSubmitRenaming,
 onCancelRenaming, onEditingTitleChange, onExportSession, onTogglePinned, onDeleteSession,
 onSetSessionMenuOpen,
 staleProjectIds,
 }: SidebarProps) {
  const t = useT();
  const [sessionQuery, setSessionQuery] = useState('');
  const deferredSessionQuery = useDeferredValue(sessionQuery.trim().toLocaleLowerCase());
  const visibleSessions = deferredSessionQuery
    ? sortedSessions.filter((session) => session.title.toLocaleLowerCase().includes(deferredSessionQuery))
    : sortedSessions;
  return (
    <aside
      aria-hidden={!sidebarOpen}
      className={`shrink-0 overflow-hidden border-r transition-[width] duration-300 ease-out ${sidebarOpen ? 'w-64' : 'w-0 pointer-events-none'} aegis-bg-surface aegis-border-subtle ${isDark ? '' : 'shadow-[12px_0_34px_rgba(80,62,39,0.10)]'}`}
    >
      <div className={`flex h-full w-64 shrink-0 flex-col py-5 pl-3 pr-4 transition-opacity duration-150 ease-out ${sidebarOpen ? 'opacity-100 delay-100' : 'opacity-0'}`}>
        <div className="mb-7">
          <div className="aegis-wordmark">AEGIS</div>
        </div>

        <button
          className="aegis-accent-solid relative mb-5 flex items-center justify-center rounded-xl px-3.5 py-2.5 text-[12px] font-semibold tracking-[0.08em] text-white disabled:opacity-60"
          disabled={isStreaming}
          onClick={onNewSession}
          type="button"
        >
          <MessageSquare className="absolute left-3" size={15} />
            <span>{t('sidebar.new_conversation')}</span>
        </button>

        {/* Projects Section */}
        <div className="mb-4">
          <div className="mb-2 flex items-center justify-between">
            <button
              className={`aegis-display flex min-w-0 items-center gap-1.5 text-[12px] font-medium uppercase tracking-[0.08em] transition ${isDark ? 'text-zinc-400 hover:text-zinc-100' : 'text-stone-600 hover:text-stone-950'}`}
              onClick={onToggleProjects}
              type="button"
            >
              <ChevronDown className={`shrink-0 transition-transform ${projectsOpen ? '' : '-rotate-90'}`} size={15} />
              <span>{t('sidebar.projects')}</span>
            </button>
            <button
              aria-label="Open project folder"
              className={`rounded-lg p-1.5 transition ${isDark ? 'text-zinc-500 hover:bg-zinc-900 hover:text-emerald-300' : 'text-stone-500 hover:bg-[rgba(94,76,55,0.1)] hover:text-emerald-800'}`}
              disabled={scanningProject}
              onClick={onAddProject}
              title="Open project folder"
              type="button"
            >
              <FolderPlus size={16} />
            </button>
          </div>

          {projectsOpen && (
            <div className="space-y-1.5">
              {codeProjects.length === 0 ? (
                <button
                  className={`flex w-full items-center gap-2 rounded-xl border px-3 py-2.5 text-left text-sm transition ${isDark ? 'border-zinc-800 text-zinc-500 hover:bg-zinc-900' : 'border-[rgba(94,76,55,0.26)] bg-[rgba(255,250,240,0.42)] text-stone-600 hover:bg-[rgba(255,250,240,0.76)] hover:text-stone-950'}`}
                    disabled={scanningProject}
                    onClick={onAddProject}
                    type="button"
                  >
                    <FolderOpen size={15} />
                    {scanningProject ? t('sidebar.scanning') : t('sidebar.open_project')}
                  </button>
              ) : (
                codeProjects.map((project) => {
                  const isActiveProject = activeProjectId === project.id;
                  return (
                    <div
                      className={`group flex items-center gap-2 rounded-xl px-2.5 py-2.5 transition ${isActiveProject
                        ? isDark ? 'bg-zinc-900 text-zinc-50 shadow-[0_3px_12px_rgba(255,255,255,0.10)]' : 'bg-[rgba(255,250,240,0.9)] text-stone-950 shadow-[0_10px_24px_rgba(80,62,39,0.14)] ring-1 ring-[rgba(94,76,55,0.18)]'
                        : isDark ? 'text-zinc-400 hover:bg-zinc-900/70 hover:text-zinc-100' : 'text-stone-600 hover:bg-[rgba(255,250,240,0.62)] hover:text-stone-950'}`}
                      key={project.id}
                    >
                      <button
                        className="flex min-w-0 flex-1 items-center gap-2 text-left"
                        onClick={() => onSelectProject(project.id)}
                        type="button"
                      >
                        <FolderOpen className={isActiveProject ? 'text-emerald-400' : ''} size={16} />
                        {staleProjectIds.has(project.id) && (
                          <span className="flex h-4 w-4 shrink-0 items-center justify-center rounded-full bg-amber-500/20 text-[7px] font-bold text-amber-400" title="Project access lost — re-add to enable editing">!</span>
                        )}
                        <span className="min-w-0">
                          <span className="block truncate text-sm">{project.name}</span>
                          <span className={`block truncate text-[11px] font-normal ${isDark ? 'text-zinc-500' : 'text-stone-500'}`}>
                            {project.fileCount} files &middot; {Math.ceil(project.totalBytes / 1024)} KB
                            {project.writable ? ' &middot; editable' : ' &middot; read-only'}
                          </span>
                        </span>
                      </button>
                      <button
                        aria-label={`Remove ${project.name}`}
                        className={`rounded-md p-1 opacity-0 transition group-hover:opacity-100 ${isDark ? 'text-zinc-500 hover:bg-zinc-800 hover:text-red-300' : 'text-stone-500 hover:bg-red-100/70 hover:text-red-700'}`}
                        onClick={() => onRemoveProject(project.id)}
                        type="button"
                      >
                        <X size={14} />
                      </button>
                    </div>
                  );
                })
              )}
            </div>
          )}
        </div>

        {/* Sessions Section */}
        <div className="mb-2 flex items-center justify-between">
          <button
            className={`aegis-display flex items-center gap-1.5 text-[12px] font-medium uppercase tracking-[0.08em] transition ${isDark ? 'text-zinc-400 hover:text-zinc-100' : 'text-stone-600 hover:text-stone-950'}`}
            onClick={onToggleSessions}
            type="button"
          >
            <ChevronDown className={`transition-transform ${sessionsOpen ? '' : '-rotate-90'}`} size={15} />
            <span>{t('sidebar.sessions')}</span>
          </button>
        </div>

        {sessionsOpen && sessions.length > 0 && (
          <label className={`mb-1 flex items-center gap-2 rounded-xl border px-2.5 py-2 transition focus-within:ring-1 ${isDark ? 'border-zinc-800 bg-zinc-950/55 text-zinc-500 focus-within:border-zinc-700 focus-within:ring-emerald-500/30' : 'border-[rgba(94,76,55,0.22)] bg-[rgba(255,250,240,0.52)] text-stone-500 focus-within:border-emerald-700/50 focus-within:ring-emerald-700/15'}`}>
            <Search size={14} />
            <input
              aria-label="Search conversations"
              className="min-w-0 flex-1 bg-transparent text-[12px] text-inherit outline-none placeholder:text-inherit"
              onChange={(event) => setSessionQuery(event.target.value)}
              placeholder="Find a conversation"
              type="search"
              value={sessionQuery}
            />
          </label>
        )}

        <div className={`sessions-scroll -ml-1.5 -mr-3 min-h-0 flex-1 space-y-1.5 overflow-y-auto py-2 pl-2 pr-3 ${sessionsOpen ? '' : 'hidden'}`}>
          {sessions.length === 0 ? (
              <button className={`w-full rounded-xl border p-3 text-left text-sm transition ${isDark ? 'border-zinc-800 text-zinc-500 hover:bg-zinc-900 hover:text-zinc-300' : 'border-[rgba(94,76,55,0.26)] bg-[rgba(255,250,240,0.42)] text-stone-600 hover:bg-[rgba(255,250,240,0.75)]'}`} onClick={onNewSession} type="button">
                <span className="block font-medium">No conversations yet</span>
                <span className="mt-1 block text-[11px] opacity-75">Start one and it will stay available here.</span>
              </button>
          ) : visibleSessions.length === 0 ? (
              <div className={`rounded-xl border p-3 text-sm ${isDark ? 'border-zinc-800 text-zinc-500' : 'border-[rgba(94,76,55,0.26)] bg-[rgba(255,250,240,0.42)] text-stone-600'}`}>
                <span className="block font-medium">No matching conversations</span>
                <button className="mt-1 text-[11px] underline underline-offset-2 opacity-75" onClick={() => setSessionQuery('')} type="button">Clear search</button>
              </div>
          ) : (
            visibleSessions.map((session, sessionIndex) => {
              const isDeleting = deletingSessionIds.includes(session.session_id);
              const isNewSession = newSessionPulseId === session.session_id;
              const isActive = session.session_id === activeSessionId;
              const isPinned = pinnedSessionIdSet.has(session.session_id);
              const shouldOpenMenuUp = sessionIndex > visibleSessions.length - 4;
              const lastAccessedLabel = formatSessionLastAccessed(session.updated_at);

              const cardStateClasses = isDeleting
                ? isDark ? 'border-transparent bg-red-950/40 text-red-100 opacity-0 scale-95 -translate-x-2' : 'border-transparent bg-red-100 text-red-800 opacity-0 scale-95 -translate-x-2'
                : isActive && isPinned
                  ? isDark ? 'border-transparent bg-zinc-800/95 text-zinc-50 shadow-[0_3px_10px_rgba(255,255,255,0.16),inset_0_1px_0_rgba(255,255,255,0.16)] ring-1 ring-amber-500/20'
                    : 'border-transparent bg-[rgba(255,250,240,0.94)] text-stone-950 shadow-[0_10px_26px_rgba(80,62,39,0.16)] ring-1 ring-amber-500/35'
                  : isActive
                    ? isDark ? 'border-transparent bg-zinc-800/95 text-zinc-50 shadow-[0_3px_10px_rgba(255,255,255,0.16),inset_0_1px_0_rgba(255,255,255,0.16)]'
                      : 'border-transparent bg-[rgba(255,250,240,0.94)] text-stone-950 shadow-[0_10px_26px_rgba(80,62,39,0.16)] ring-1 ring-[rgba(94,76,55,0.14)]'
                    : isPinned
                      ? isDark ? 'border-transparent bg-zinc-900/75 text-zinc-100 shadow-[0_2px_8px_rgba(255,255,255,0.12)]'
                        : 'border-transparent bg-[rgba(255,250,240,0.72)] text-stone-900 shadow-[0_5px_16px_rgba(80,62,39,0.10)] ring-1 ring-[rgba(94,76,55,0.08)]'
                      : isDark ? 'border-transparent text-zinc-300 shadow-[0_1px_0_rgba(255,255,255,0.09)] hover:bg-zinc-900/85 hover:shadow-[0_3px_9px_rgba(255,255,255,0.14)]'
                        : 'border-transparent text-stone-700 shadow-[0_1px_0_rgba(94,76,55,0.14)] hover:bg-[rgba(255,250,240,0.68)] hover:shadow-[0_8px_20px_rgba(80,62,39,0.12)]';

              return (
                <div className={`relative w-full rounded-xl border px-2.5 py-2.5 text-left transition-all duration-200 ease-out ${isNewSession ? 'animate-[fadeInSession_520ms_ease-out]' : ''} ${cardStateClasses}`} key={session.session_id}>
                  <div className="flex items-center gap-1.5">
                    {editingSessionId === session.session_id ? (
                      <input
                        autoFocus
                        className={`session-title-text min-w-0 flex-1 rounded-lg border px-2 py-1.5 text-[13px] outline-none ${isDark ? 'border-emerald-700 bg-zinc-950 text-zinc-100' : 'border-emerald-600 bg-[rgba(255,250,240,0.96)] text-stone-950'}`}
                        onBlur={() => onSubmitRenaming(session)}
                        onChange={(e) => onEditingTitleChange(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter') { e.preventDefault(); onSubmitRenaming(session); }
                          if (e.key === 'Escape') { e.preventDefault(); onCancelRenaming(); }
                        }}
                        value={editingTitle}
                      />
                    ) : (
                      <button
                        className="min-w-0 flex-1 py-1 text-left"
                        disabled={isDeleting}
                        onClick={() => onSelectSession(session.session_id)}
                        type="button"
                      >
                        <span className="flex min-w-0 flex-col gap-0.5">
                          <span
                            className="session-title-text truncate text-[13px] font-medium leading-5"
                            onDoubleClick={(e) => { e.stopPropagation(); onBeginRenaming(session); }}
                          >
                            {session.title}
                          </span>
                          <span className={`truncate text-[11px] font-normal leading-4 ${isDark ? 'text-zinc-500' : 'text-stone-500'}`}>
                            {lastAccessedLabel}
                          </span>
                        </span>
                      </button>
                    )}

                    {isPinned && (
                      <span className={`inline-flex shrink-0 items-center justify-center rounded-lg p-1 ${isDark ? 'text-amber-300' : 'text-amber-600'}`} title="Pinned session">
                        <Pin fill="currentColor" size={14} />
                      </span>
                    )}

                    <button
                      aria-expanded={sessionMenuOpenId === session.session_id}
                      aria-controls={`session-actions-${session.session_id}`}
                      aria-haspopup="menu"
                      aria-label={`Open actions for ${session.title}`}
                      className={`rounded-lg p-1.5 transition disabled:opacity-50 ${isDark ? 'text-zinc-400 hover:bg-zinc-700/80 hover:text-zinc-100' : 'text-stone-500 hover:bg-[rgba(94,76,55,0.1)] hover:text-stone-950'}`}
                      disabled={isStreaming || isDeleting}
                      onClick={(e) => { e.stopPropagation(); onSetSessionMenuOpen(sessionMenuOpenId === session.session_id ? null : session.session_id); }}
                      type="button"
                    >
                      <MoreHorizontal size={17} />
                    </button>
                  </div>

                  {sessionMenuOpenId === session.session_id && (
                    <div
                      className={`absolute right-2 z-30 w-40 rounded-xl border p-1 text-sm shadow-xl ${shouldOpenMenuUp ? 'bottom-10' : 'top-10'} ${isDark ? 'border-zinc-800 bg-zinc-950 text-zinc-100 shadow-white/5' : 'border-[rgba(94,76,55,0.26)] bg-[rgba(255,250,240,0.96)] text-stone-950 shadow-[0_18px_44px_rgba(80,62,39,0.18)]'}`}
                      id={`session-actions-${session.session_id}`}
                      onClick={(e) => e.stopPropagation()}
                      role="menu"
                    >
                      <button className={`flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left transition ${isDark ? 'hover:bg-zinc-900' : 'hover:bg-[rgba(94,76,55,0.08)]'}`} onClick={() => onBeginRenaming(session)} role="menuitem" type="button">
                        <Edit3 size={14} /> Rename
                      </button>
                      <button className={`flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left transition ${isDark ? 'hover:bg-zinc-900' : 'hover:bg-[rgba(94,76,55,0.08)]'}`} onClick={() => onExportSession(session)} role="menuitem" type="button">
                        <Download size={14} /> Export chat
                      </button>
                      <button className={`flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left transition ${isDark ? 'hover:bg-zinc-900' : 'hover:bg-[rgba(94,76,55,0.08)]'}`} onClick={() => onTogglePinned(session.session_id)} role="menuitem" type="button">
                        <Pin fill={isPinned ? 'currentColor' : 'none'} size={14} /> {isPinned ? 'Unpin' : 'Pin'}
                      </button>
                      <button className={`flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left font-medium text-red-500 transition ${isDark ? 'hover:bg-red-950/30' : 'hover:bg-red-50'}`} onClick={() => onDeleteSession(session)} role="menuitem" type="button">
                        <Trash2 size={14} /> Delete
                      </button>
                    </div>
                  )}
                </div>
              );
            })
          )}
        </div>
      </div>
    </aside>
  );
}
