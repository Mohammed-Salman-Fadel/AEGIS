import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { FormEvent } from 'react';
import { Bot, MessageSquare, Moon, Plus, RefreshCw, Send, Sun, Trash2, Upload, User } from 'lucide-react';

type Role = 'user' | 'assistant';
type ThemeMode = 'dark' | 'light';

interface Message {
  role: Role;
  content: string;
}

interface EngineSessionSummary {
  session_id: string;
  title: string;
  turn_count: number;
  updated_at: string;
}

interface EngineSessionsResponse {
  sessions: EngineSessionSummary[];
}

interface EngineTurn {
  query: string;
  response: string;
}

interface EngineSession {
  session_id: string;
  title: string;
  history: {
    turns: EngineTurn[];
  };
}

const API_BASE = '/api';
const THEME_STORAGE_KEY = 'aegis-ui-theme';

function sessionDescription(session: EngineSessionSummary) {
  const turnLabel = session.turn_count === 1 ? 'turn' : 'turns';
  return `${session.turn_count} ${turnLabel}`;
}

function turnsToMessages(turns: EngineTurn[]): Message[] {
  return turns.flatMap((turn) => [
    { role: 'user' as const, content: turn.query },
    { role: 'assistant' as const, content: turn.response },
  ]);
}

export default function App() {
  const [sessions, setSessions] = useState<EngineSessionSummary[]>([]);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState('');
  const [theme, setTheme] = useState<ThemeMode>(() => {
    if (typeof window === 'undefined') {
      return 'dark';
    }

    const storedTheme = window.localStorage.getItem(THEME_STORAGE_KEY);
    return storedTheme === 'light' ? 'light' : 'dark';
  });
  const [isStreaming, setIsStreaming] = useState(false);
  const [isUploading, setIsUploading] = useState(false);
  const [status, setStatus] = useState('Ready');
  const [error, setError] = useState<string | null>(null);
  const [editingSessionId, setEditingSessionId] = useState<string | null>(null);
  const [editingTitle, setEditingTitle] = useState('');
  const [deletingSessionIds, setDeletingSessionIds] = useState<string[]>([]);
  const scrollRef = useRef<HTMLDivElement>(null);
  const isDark = theme === 'dark';

  const activeSession = useMemo(
    () => sessions.find((session) => session.session_id === activeSessionId),
    [activeSessionId, sessions],
  );

  const loadSessions = useCallback(async () => {
    setError(null);
    const response = await fetch(`${API_BASE}/sessions`);

    if (!response.ok) {
      throw new Error(`Engine returned HTTP ${response.status} while loading sessions.`);
    }

    const data = (await response.json()) as EngineSessionsResponse;
    setSessions(data.sessions);
  }, []);

  const createSession = useCallback(async () => {
    setError(null);
    const response = await fetch(`${API_BASE}/sessions`, {
      method: 'POST',
    });

    if (!response.ok) {
      throw new Error(`Engine returned HTTP ${response.status} while creating a session.`);
    }

    const session = (await response.json()) as EngineSession;
    setActiveSessionId(session.session_id);
    return session;
  }, []);

  const loadSession = useCallback(async (sessionId: string) => {
    setError(null);
    setStatus('Loading session');
    const response = await fetch(`${API_BASE}/sessions/${encodeURIComponent(sessionId)}`);

    if (!response.ok) {
      throw new Error(`Engine returned HTTP ${response.status} while loading the session.`);
    }

    const session = (await response.json()) as EngineSession;
    setActiveSessionId(session.session_id);
    setMessages(turnsToMessages(session.history.turns));
    setStatus('Ready');
  }, []);

  useEffect(() => {
    loadSessions().catch((loadError: unknown) => {
      setError(loadError instanceof Error ? loadError.message : 'Could not load sessions.');
      setStatus('Engine unavailable');
    });
  }, [loadSessions]);

  useEffect(() => {
    if (typeof window === 'undefined') {
      return;
    }

    window.localStorage.setItem(THEME_STORAGE_KEY, theme);
  }, [theme]);

  useEffect(() => {
    scrollRef.current?.scrollTo({
      top: scrollRef.current.scrollHeight,
      behavior: 'smooth',
    });
  }, [messages, isStreaming]);

  async function handleSessionSelect(sessionId: string) {
    if (isStreaming) {
      return;
    }

    try {
      await loadSession(sessionId);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : 'Could not load the session.');
      setStatus('Session load failed');
    }
  }

  async function handleNewSession() {
    if (isStreaming) {
      return;
    }

    setStatus('Creating session');
    setMessages([]);
    setError(null);

    try {
      await createSession();
      await loadSessions();
      setStatus('Ready');
    } catch (createError) {
      setError(createError instanceof Error ? createError.message : 'Could not create a new session.');
      setStatus('Session creation failed');
    }
  }

  async function handleDeleteSession(session: EngineSessionSummary) {
    if (isStreaming || deletingSessionIds.includes(session.session_id)) {
      return;
    }

    const confirmed = globalThis.confirm(
      `Delete session "${session.title}"?\n\nThis will permanently remove the saved conversation.`,
    );

    if (!confirmed) {
      return;
    }

    setError(null);
    setStatus('Deleting session');

    try {
      const response = await fetch(`${API_BASE}/sessions/${encodeURIComponent(session.session_id)}`, {
        method: 'DELETE',
      });

      if (!response.ok) {
        throw new Error(`Engine returned HTTP ${response.status} while deleting the session.`);
      }

      setDeletingSessionIds((current) => [...current, session.session_id]);

      if (session.session_id === activeSessionId) {
        setActiveSessionId(null);
        setMessages([]);
      }

      await new Promise((resolve) => {
        window.setTimeout(resolve, 320);
      });
      await loadSessions();
      setDeletingSessionIds((current) =>
        current.filter((sessionId) => sessionId !== session.session_id),
      );
      setStatus('Ready');
    } catch (deleteError) {
      setDeletingSessionIds((current) =>
        current.filter((sessionId) => sessionId !== session.session_id),
      );
      setError(
        deleteError instanceof Error ? deleteError.message : 'Could not delete the session.',
      );
      setStatus('Session deletion failed');
    }
  }

  function beginRenamingSession(session: EngineSessionSummary) {
    if (isStreaming) {
      return;
    }

    setEditingSessionId(session.session_id);
    setEditingTitle(session.title);
    setError(null);
  }

  function cancelRenamingSession() {
    setEditingSessionId(null);
    setEditingTitle('');
  }

  async function submitRenamingSession(session: EngineSessionSummary) {
    const nextTitle = editingTitle.trim();
    if (!nextTitle) {
      cancelRenamingSession();
      return;
    }

    if (nextTitle === session.title) {
      cancelRenamingSession();
      return;
    }

    setError(null);
    setStatus('Renaming session');

    try {
      const response = await fetch(`${API_BASE}/sessions/${encodeURIComponent(session.session_id)}`, {
        method: 'PATCH',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ title: nextTitle }),
      });

      if (!response.ok) {
        throw new Error(`Engine returned HTTP ${response.status} while renaming the session.`);
      }

      await loadSessions();
      setStatus('Ready');
      cancelRenamingSession();
    } catch (renameError) {
      setError(
        renameError instanceof Error ? renameError.message : 'Could not rename the session.',
      );
      setStatus('Session rename failed');
    }
  }

  async function handleFileUpload(event: React.ChangeEvent<HTMLInputElement>) {
    const files = event.target.files;
    if (!files || files.length === 0 || isUploading) {
      return;
    }

    const validExtensions = ['.pdf', '.txt'];
    const unsupportedFiles = Array.from(files).filter(
      (file) => !validExtensions.some((ext) => file.name.toLowerCase().endsWith(ext)),
    );

    if (unsupportedFiles.length > 0) {
      setError(`Unsupported file types: ${unsupportedFiles.map((f) => f.name).join(', ')}. Only PDF and TXT are supported.`);
      event.target.value = '';
      return;
    }

    setIsUploading(true);
    setStatus('Indexing documents');
    setError(null);

    try {
      const formData = new FormData();
      for (let i = 0; i < files.length; i++) {
        formData.append('file', files[i]);
      }

      const response = await fetch(`${API_BASE}/ingest`, {
        method: 'POST',
        body: formData,
      });

      if (!response.ok) {
        throw new Error(`Engine returned HTTP ${response.status} while uploading.`);
      }

      setStatus('Indexed successfully');
    } catch (uploadError) {
      setError(uploadError instanceof Error ? uploadError.message : 'Upload failed');
      setStatus('Upload failed');
    } finally {
      setIsUploading(false);
      event.target.value = '';
    }
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const prompt = input.trim();
    if (!prompt || isStreaming) {
      return;
    }

    setInput('');
    setError(null);
    setStatus('Inference');
    setIsStreaming(true);
    setMessages((current) => [
      ...current,
      { role: 'user', content: prompt },
      { role: 'assistant', content: '' },
    ]);

    try {
      const sessionId = activeSessionId ?? (await createSession()).session_id;
      const response = await fetch(`${API_BASE}/chat`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          session_id: sessionId,
          message: prompt,
        }),
      });

      if (!response.ok || !response.body) {
        throw new Error(`Engine returned HTTP ${response.status} while sending chat.`);
      }

      const reader = response.body.getReader();
      const decoder = new TextDecoder();
      let pending = '';

      while (true) {
        const { done, value } = await reader.read();
        if (done) {
          break;
        }

        pending += decoder.decode(value, { stream: true });
        const lines = pending.split('\n');
        pending = lines.pop() ?? '';

        for (const line of lines) {
          const trimmed = line.trim();
          if (!trimmed.startsWith('data:')) {
            continue;
          }

          const data = trimmed.replace(/^data:\s?/, '');
          if (data === '[DONE]') {
            setStatus('Complete');
            continue;
          }

          if (data.startsWith('[ERROR]')) {
            throw new Error(data);
          }

          setMessages((current) => {
            const next = [...current];
            const last = next[next.length - 1];

            if (last?.role === 'assistant') {
              next[next.length - 1] = {
                ...last,
                content: `${last.content}${data}`,
              };
            }

            return next;
          });
        }
      }

      setStatus('Complete');
      await loadSessions();
    } catch (sendError) {
      setError(sendError instanceof Error ? sendError.message : 'Could not send chat request.');
      setStatus('Chat failed');
      setMessages((current) => current.filter((message) => message.content.length > 0));
    } finally {
      setIsStreaming(false);
    }
  }

  return (
    <div
      className={`flex h-screen overflow-hidden ${
        isDark ? 'bg-zinc-950 text-zinc-100' : 'bg-stone-100 text-slate-900'
      }`}
    >
      <aside
        className={`flex w-72 shrink-0 flex-col border-r p-4 ${
          isDark ? 'border-zinc-800 bg-zinc-950' : 'border-stone-300 bg-stone-50'
        }`}
      >
        <div className="mb-6">
          <div className="text-xl font-semibold tracking-wide">AEGIS</div>
          <div className={`mt-1 text-xs ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>
            Rust engine client
          </div>
        </div>

        <button
          className="mb-4 flex items-center justify-center gap-2 rounded-lg bg-emerald-600 px-3 py-2 text-sm font-medium text-white hover:bg-emerald-500 disabled:opacity-60"
          disabled={isStreaming}
          onClick={() => {
            void handleNewSession();
          }}
          type="button"
        >
          <Plus size={16} />
          New Chat
        </button>

        <button
          className={`mb-4 flex items-center justify-center gap-2 rounded-lg border px-3 py-2 text-sm disabled:opacity-60 ${
            isDark
              ? 'border-zinc-800 text-zinc-300 hover:bg-zinc-900'
              : 'border-stone-300 text-slate-700 hover:bg-stone-200'
          }`}
          disabled={isStreaming}
          onClick={() => {
            loadSessions().catch((loadError: unknown) => {
              setError(loadError instanceof Error ? loadError.message : 'Could not refresh sessions.');
            });
          }}
          type="button"
        >
          <RefreshCw size={15} />
          Refresh
        </button>

        <div
          className={`mb-2 text-xs font-semibold uppercase tracking-wide ${
            isDark ? 'text-zinc-500' : 'text-slate-500'
          }`}
        >
          Sessions
        </div>

        <div className="min-h-0 flex-1 space-y-2 overflow-y-auto">
          {sessions.length === 0 ? (
            <div
              className={`rounded-lg border p-3 text-sm ${
                isDark ? 'border-zinc-800 text-zinc-500' : 'border-stone-300 text-slate-500'
              }`}
            >
              No saved sessions yet.
            </div>
          ) : (
            sessions.map((session) => {
              const isDeleting = deletingSessionIds.includes(session.session_id);
              const cardStateClasses = isDeleting
                ? isDark
                  ? 'border-red-500 bg-red-950/40 opacity-0 scale-95 -translate-x-2'
                  : 'border-red-300 bg-red-100 opacity-0 scale-95 -translate-x-2'
                : session.session_id === activeSessionId
                  ? isDark
                    ? 'border-emerald-600 bg-emerald-950/30'
                    : 'border-emerald-500 bg-emerald-100'
                  : isDark
                    ? 'border-zinc-800 hover:bg-zinc-900'
                    : 'border-stone-300 hover:bg-stone-200';

              return (
              <div
                className={`w-full rounded-lg border p-3 text-left transition-all duration-300 ease-out ${cardStateClasses}`}
                key={session.session_id}
              >
                <div className="flex items-start gap-2">
                  {editingSessionId === session.session_id ? (
                    <div className="min-w-0 flex-1 text-left">
                      <div className="flex items-center gap-2 text-sm font-medium">
                        <MessageSquare size={15} />
                        <input
                          autoFocus
                          className={`min-w-0 flex-1 rounded border px-2 py-1 text-sm outline-none ${
                            isDark
                              ? 'border-emerald-700 bg-zinc-950 text-zinc-100'
                              : 'border-emerald-500 bg-white text-slate-900'
                          }`}
                          onBlur={() => {
                            void submitRenamingSession(session);
                          }}
                          onChange={(event) => setEditingTitle(event.target.value)}
                          onKeyDown={(event) => {
                            if (event.key === 'Enter') {
                              event.preventDefault();
                              void submitRenamingSession(session);
                            }
                            if (event.key === 'Escape') {
                              event.preventDefault();
                              cancelRenamingSession();
                            }
                          }}
                          value={editingTitle}
                        />
                      </div>
                      <div className={`mt-1 text-xs ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>
                        {sessionDescription(session)}
                      </div>
                    </div>
                  ) : (
                    <button
                      className="min-w-0 flex-1 text-left"
                      disabled={isStreaming || isDeleting}
                      onClick={() => {
                        void handleSessionSelect(session.session_id);
                      }}
                      type="button"
                    >
                      <div className="flex items-center gap-2 text-sm font-medium">
                        <MessageSquare size={15} />
                        <span
                          className="truncate"
                          onDoubleClick={(event) => {
                            event.stopPropagation();
                            beginRenamingSession(session);
                          }}
                        >
                          {session.title}
                        </span>
                      </div>
                      <div className={`mt-1 text-xs ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>
                        {sessionDescription(session)}
                      </div>
                    </button>
                  )}
                  <button
                    aria-label={`Delete session ${session.title}`}
                    className={`rounded-md p-2 transition disabled:opacity-60 ${
                      isDark
                        ? 'text-zinc-500 hover:bg-red-950/40 hover:text-red-300'
                        : 'text-slate-500 hover:bg-red-100 hover:text-red-600'
                    }`}
                    disabled={isStreaming || isDeleting}
                    onClick={() => {
                      void handleDeleteSession(session);
                    }}
                    type="button"
                  >
                    <Trash2 size={15} />
                  </button>
                </div>
              </div>
            )})
          )}
        </div>
      </aside>

      <main className="flex min-w-0 flex-1 flex-col">
        <header
          className={`flex h-16 shrink-0 items-center justify-between border-b px-6 ${
            isDark ? 'border-zinc-800' : 'border-stone-300'
          }`}
        >
          <div>
            <div className="text-sm font-medium">{activeSession?.title ?? 'New chat'}</div>
            <div className={`text-xs ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>
              Session: {activeSessionId ?? 'Not started yet'}
            </div>
          </div>
          <div className="flex items-center gap-3">
            <button
              className={`inline-flex items-center gap-2 rounded-lg border px-3 py-2 text-xs font-medium transition ${
                isDark
                  ? 'border-zinc-800 text-zinc-300 hover:bg-zinc-900'
                  : 'border-stone-300 bg-white text-slate-700 hover:bg-stone-100'
              }`}
              onClick={() => setTheme((current) => (current === 'dark' ? 'light' : 'dark'))}
              type="button"
            >
              {isDark ? <Sun size={14} /> : <Moon size={14} />}
              {isDark ? 'Light mode' : 'Dark mode'}
            </button>
            <div
              className={`rounded-lg border px-3 py-1 text-xs ${
                isDark
                  ? 'border-zinc-800 text-zinc-400'
                  : 'border-stone-300 bg-white text-slate-500'
              }`}
            >
              {status}
            </div>
          </div>
        </header>

        {error && (
          <div
            className={`border-b px-6 py-3 text-sm ${
              isDark
                ? 'border-red-900/60 bg-red-950/30 text-red-200'
                : 'border-red-200 bg-red-50 text-red-700'
            }`}
          >
            {error}
          </div>
        )}

        <div ref={scrollRef} className="min-h-0 flex-1 overflow-y-auto px-6 py-6">
          <div className="mx-auto flex max-w-3xl flex-col gap-4">
            {messages.length === 0 ? (
              <div
                className={`rounded-lg border p-6 ${
                  isDark
                    ? 'border-zinc-800 bg-zinc-900/40 text-zinc-400'
                    : 'border-stone-300 bg-white text-slate-500'
                }`}
              >
                Ask a question to start a session. The response streams from the Rust engine through
                the same `/chat` endpoint used by the CLI.
              </div>
            ) : (
              messages.map((message, index) => (
                <div
                  className={`flex gap-3 ${message.role === 'user' ? 'justify-end' : 'justify-start'}`}
                  key={`${message.role}-${index}`}
                >
                  {message.role === 'assistant' && (
                    <div
                      className={`mt-1 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg ${
                        isDark ? 'bg-zinc-800' : 'bg-stone-200 text-slate-700'
                      }`}
                    >
                      <Bot size={16} />
                    </div>
                  )}
                  <div
                    className={`max-w-[78%] whitespace-pre-wrap rounded-lg px-4 py-3 text-sm leading-6 ${
                      message.role === 'user'
                        ? 'bg-emerald-600 text-white'
                        : isDark
                          ? 'border border-zinc-800 bg-zinc-900 text-zinc-200'
                          : 'border border-stone-300 bg-white text-slate-800'
                    }`}
                  >
                    {message.content || '...'}
                  </div>
                  {message.role === 'user' && (
                    <div className="mt-1 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-emerald-700">
                      <User size={16} />
                    </div>
                  )}
                </div>
              ))
            )}
          </div>
        </div>

        <footer
          className={`shrink-0 border-t p-4 ${isDark ? 'border-zinc-800' : 'border-stone-300'}`}
        >
          <form className="mx-auto flex max-w-3xl gap-3" onSubmit={handleSubmit}>
            <input
              className={`min-w-0 flex-1 rounded-lg border px-4 py-3 text-sm outline-none focus:border-emerald-600 ${
                isDark
                  ? 'border-zinc-800 bg-zinc-900 text-zinc-100 placeholder:text-zinc-500'
                  : 'border-stone-300 bg-white text-slate-900 placeholder:text-slate-400'
              }`}
              disabled={isStreaming}
              onChange={(event) => setInput(event.target.value)}
              placeholder="Message AEGIS"
              value={input}
            />
            <label
              className={`flex cursor-pointer items-center justify-center rounded-lg border px-4 py-3 text-sm transition ${
                isStreaming || isUploading ? 'cursor-not-allowed opacity-60' : ''
              } ${
                isDark
                  ? 'border-zinc-800 bg-zinc-900 text-zinc-300 hover:bg-zinc-800'
                  : 'border-stone-300 bg-white text-slate-700 hover:bg-stone-50'
              }`}
              title="Supported: PDF, TXT"
            >
              <Upload size={16} />
              <span className="ml-2">Import</span>
              <input
                accept=".pdf,.txt"
                className="hidden"
                disabled={isStreaming || isUploading}
                multiple
                onChange={(event) => void handleFileUpload(event)}
                title="Supported files: PDF, TXT"
                type="file"
              />
            </label>
            <button
              className="flex items-center gap-2 rounded-lg bg-emerald-600 px-4 py-3 text-sm font-medium text-white hover:bg-emerald-500 disabled:opacity-60"
              disabled={isStreaming || !input.trim() || isUploading}
              type="submit"
            >
              <Send size={16} />
              Send
            </button>
          </form>
        </footer>
      </main>
    </div>
  );
}
