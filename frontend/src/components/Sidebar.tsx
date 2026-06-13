// Sidebar with session list and project management
import {
  MessageSquare, ChevronDown, FolderPlus, FolderOpen, X, Edit3, Download, Pin, Trash2, MoreHorizontal,
} from 'lucide-react';
import type { EngineSessionSummary, CodeProject } from '../types';
import { formatSessionLastAccessed } from '../lib';

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
}: SidebarProps) {
  return (
    <aside
      aria-hidden={!sidebarOpen}
      className={`shrink-0 overflow-hidden border-r transition-[width] duration-300 ease-out ${sidebarOpen ? 'w-64' : 'w-0 pointer-events-none'} ${isDark ? 'border-zinc-800 bg-zinc-950' : 'border-stone-300 bg-stone-50'}`}
    >
      <div className={`flex h-full w-64 shrink-0 flex-col py-4 pl-2 pr-4 transition-opacity duration-150 ease-out ${sidebarOpen ? 'opacity-100 delay-100' : 'opacity-0'}`}>
        <div className="mb-6">
          <div className="aegis-wordmark">AEGIS</div>
        </div>

        <button
          className="aegis-accent-solid relative mb-4 flex items-center justify-center rounded-lg px-3 py-2 text-xs font-semibold tracking-[0.14em] text-white disabled:opacity-60"
          disabled={isStreaming}
          onClick={onNewSession}
          type="button"
        >
          <MessageSquare className="absolute left-3" size={15} />
          <span>NEW CONVERSATION</span>
        </button>

        {/* Projects Section */}
        <div className="mb-3">
          <div className="mb-2 flex items-center justify-between">
            <button
              className={`flex min-w-0 items-center gap-1.5 text-[14px] font-semibold transition ${isDark ? 'text-zinc-400 hover:text-zinc-100' : 'text-slate-600 hover:text-slate-950'}`}
              onClick={onToggleProjects}
              type="button"
            >
              <ChevronDown className={`shrink-0 transition-transform ${projectsOpen ? '' : '-rotate-90'}`} size={15} />
              <span>Projects</span>
            </button>
            <button
              aria-label="Open project folder"
              className={`rounded-lg p-1.5 transition ${isDark ? 'text-zinc-500 hover:bg-zinc-900 hover:text-emerald-300' : 'text-slate-500 hover:bg-stone-200 hover:text-emerald-700'}`}
              disabled={scanningProject}
              onClick={onAddProject}
              title="Open project folder"
              type="button"
            >
              <FolderPlus size={16} />
            </button>
          </div>

          {projectsOpen && (
            <div className="space-y-1">
              {codeProjects.length === 0 ? (
                <button
                  className={`flex w-full items-center gap-2 rounded-lg border px-3 py-2 text-left text-sm transition ${isDark ? 'border-zinc-800 text-zinc-500 hover:bg-zinc-900' : 'border-stone-300 text-slate-500 hover:bg-stone-100'}`}
                  disabled={scanningProject}
                  onClick={onAddProject}
                  type="button"
                >
                  <FolderOpen size={15} />
                  {scanningProject ? 'Scanning folder...' : 'Open project folder'}
                </button>
              ) : (
                codeProjects.map((project) => {
                  const isActiveProject = activeProjectId === project.id;
                  return (
                    <div
                      className={`group flex items-center gap-2 rounded-lg px-2.5 py-2 transition ${isActiveProject
                        ? isDark ? 'bg-zinc-900 text-zinc-50 shadow-[0_3px_12px_rgba(255,255,255,0.10)]' : 'bg-white text-slate-950 shadow-[0_8px_20px_rgba(120,113,108,0.12)]'
                        : isDark ? 'text-zinc-400 hover:bg-zinc-900/70 hover:text-zinc-100' : 'text-slate-600 hover:bg-white hover:text-slate-950'}`}
                      key={project.id}
                    >
                      <button
                        className="flex min-w-0 flex-1 items-center gap-2 text-left"
                        onClick={() => onSelectProject(project.id)}
                        type="button"
                      >
                        <FolderOpen className={isActiveProject ? 'text-emerald-400' : ''} size={16} />
                        <span className="min-w-0">
                          <span className="block truncate text-sm">{project.name}</span>
                          <span className={`block truncate text-[11px] ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>
                            {project.fileCount} files &middot; {Math.ceil(project.totalBytes / 1024)} KB
                            {project.writable ? ' &middot; editable' : ' &middot; read-only'}
                          </span>
                        </span>
                      </button>
                      <button
                        aria-label={`Remove ${project.name}`}
                        className={`rounded-md p-1 opacity-0 transition group-hover:opacity-100 ${isDark ? 'text-zinc-500 hover:bg-zinc-800 hover:text-red-300' : 'text-slate-500 hover:bg-stone-100 hover:text-red-600'}`}
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
            className={`flex items-center gap-1.5 text-[14px] font-semibold transition ${isDark ? 'text-zinc-400 hover:text-zinc-100' : 'text-slate-600 hover:text-slate-950'}`}
            onClick={onToggleSessions}
            type="button"
          >
            <ChevronDown className={`transition-transform ${sessionsOpen ? '' : '-rotate-90'}`} size={15} />
            <span>Sessions</span>
          </button>
        </div>

        <div className={`sessions-scroll -ml-1.5 -mr-3 min-h-0 flex-1 space-y-1 overflow-y-auto py-1.5 pl-2 pr-3 ${sessionsOpen ? '' : 'hidden'}`}>
          {sessions.length === 0 ? (
            <div className={`rounded-lg border p-3 text-sm ${isDark ? 'border-zinc-800 text-zinc-500' : 'border-stone-300 text-slate-500'}`}>
              No saved sessions yet.
            </div>
          ) : (
            sortedSessions.map((session, sessionIndex) => {
              const isDeleting = deletingSessionIds.includes(session.session_id);
              const isNewSession = newSessionPulseId === session.session_id;
              const isActive = session.session_id === activeSessionId;
              const isPinned = pinnedSessionIdSet.has(session.session_id);
              const shouldOpenMenuUp = sessionIndex > sortedSessions.length - 4;
              const lastAccessedLabel = formatSessionLastAccessed(session.updated_at);

              const cardStateClasses = isDeleting
                ? isDark ? 'border-transparent bg-red-950/40 text-red-100 opacity-0 scale-95 -translate-x-2' : 'border-transparent bg-red-100 text-red-800 opacity-0 scale-95 -translate-x-2'
                : isActive && isPinned
                  ? isDark ? 'border-transparent bg-zinc-800/95 text-zinc-50 shadow-[0_3px_10px_rgba(255,255,255,0.16),inset_0_1px_0_rgba(255,255,255,0.16)] ring-1 ring-amber-500/20'
                    : 'border-transparent bg-white text-slate-950 shadow-[0_8px_22px_rgba(120,113,108,0.16)] ring-1 ring-amber-300/55'
                  : isActive
                    ? isDark ? 'border-transparent bg-zinc-800/95 text-zinc-50 shadow-[0_3px_10px_rgba(255,255,255,0.16),inset_0_1px_0_rgba(255,255,255,0.16)]'
                      : 'border-transparent bg-white text-slate-950 shadow-[0_8px_22px_rgba(120,113,108,0.16)]'
                    : isPinned
                      ? isDark ? 'border-transparent bg-zinc-900/75 text-zinc-100 shadow-[0_2px_8px_rgba(255,255,255,0.12)]'
                        : 'border-transparent bg-white/80 text-slate-900 shadow-[0_3px_14px_rgba(120,113,108,0.10)]'
                      : isDark ? 'border-transparent text-zinc-300 shadow-[0_1px_0_rgba(255,255,255,0.09)] hover:bg-zinc-900/85 hover:shadow-[0_3px_9px_rgba(255,255,255,0.14)]'
                        : 'border-transparent text-slate-700 shadow-[0_1px_0_rgba(120,113,108,0.12)] hover:bg-white/80 hover:shadow-[0_8px_20px_rgba(120,113,108,0.12)]';

              return (
                <div className={`relative w-full rounded-lg border px-2 py-2 text-left transition-all duration-200 ease-out ${isNewSession ? 'animate-[fadeInSession_520ms_ease-out]' : ''} ${cardStateClasses}`} key={session.session_id}>
                  <div className="flex items-center gap-1.5">
                    {editingSessionId === session.session_id ? (
                      <input
                        autoFocus
                        className={`session-title-text min-w-0 flex-1 rounded-lg border px-2 py-1.5 text-[13px] outline-none ${isDark ? 'border-emerald-700 bg-zinc-950 text-zinc-100' : 'border-emerald-500 bg-white text-slate-900'}`}
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
                            className="session-title-text truncate text-[13px] leading-5"
                            onDoubleClick={(e) => { e.stopPropagation(); onBeginRenaming(session); }}
                          >
                            {session.title}
                          </span>
                          <span className={`truncate text-[11px] leading-4 ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>
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
                      aria-label={`Open actions for ${session.title}`}
                      className={`rounded-lg p-1.5 transition disabled:opacity-50 ${isDark ? 'text-zinc-400 hover:bg-zinc-700/80 hover:text-zinc-100' : 'text-slate-500 hover:bg-stone-100 hover:text-slate-900'}`}
                      disabled={isStreaming || isDeleting}
                      onClick={(e) => { e.stopPropagation(); onSetSessionMenuOpen(sessionMenuOpenId === session.session_id ? null : session.session_id); }}
                      type="button"
                    >
                      <MoreHorizontal size={17} />
                    </button>
                  </div>

                  {sessionMenuOpenId === session.session_id && (
                    <div
                      className={`absolute right-2 z-30 w-40 rounded-xl border p-1 text-sm shadow-xl ${shouldOpenMenuUp ? 'bottom-10' : 'top-10'} ${isDark ? 'border-zinc-800 bg-zinc-950 text-zinc-100 shadow-white/5' : 'border-stone-200 bg-white text-slate-900 shadow-stone-300/50'}`}
                      onClick={(e) => e.stopPropagation()}
                    >
                      <button className={`flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left transition ${isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'}`} onClick={() => onBeginRenaming(session)} type="button">
                        <Edit3 size={14} /> Rename
                      </button>
                      <button className={`flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left transition ${isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'}`} onClick={() => onExportSession(session)} type="button">
                        <Download size={14} /> Export chat
                      </button>
                      <button className={`flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left transition ${isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'}`} onClick={() => onTogglePinned(session.session_id)} type="button">
                        <Pin fill={isPinned ? 'currentColor' : 'none'} size={14} /> {isPinned ? 'Unpin' : 'Pin'}
                      </button>
                      <button className={`flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left font-medium text-red-500 transition ${isDark ? 'hover:bg-red-950/30' : 'hover:bg-red-50'}`} onClick={() => onDeleteSession(session)} type="button">
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
