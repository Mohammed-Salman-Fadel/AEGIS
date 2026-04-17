import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { FormEvent } from 'react';
import { Bot, MessageSquare, Plus, RefreshCw, Send, User } from 'lucide-react';

type Role = 'user' | 'assistant';

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

function createSessionId() {
  return globalThis.crypto?.randomUUID?.() ?? Math.random().toString(36).slice(2);
}

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
  const [activeSessionId, setActiveSessionId] = useState<string>(() => createSessionId());
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState('');
  const [isStreaming, setIsStreaming] = useState(false);
  const [status, setStatus] = useState('Ready');
  const [error, setError] = useState<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

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

  function handleNewSession() {
    if (isStreaming) {
      return;
    }

    setActiveSessionId(createSessionId());
    setMessages([]);
    setError(null);
    setStatus('Ready');
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
      const response = await fetch(`${API_BASE}/chat`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          session_id: activeSessionId,
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
    <div className="flex h-screen overflow-hidden bg-zinc-950 text-zinc-100">
      <aside className="flex w-72 shrink-0 flex-col border-r border-zinc-800 bg-zinc-950 p-4">
        <div className="mb-6">
          <div className="text-xl font-semibold tracking-wide">AEGIS</div>
          <div className="mt-1 text-xs text-zinc-500">Rust engine client</div>
        </div>

        <button
          className="mb-4 flex items-center justify-center gap-2 rounded-lg bg-emerald-600 px-3 py-2 text-sm font-medium text-white hover:bg-emerald-500 disabled:opacity-60"
          disabled={isStreaming}
          onClick={handleNewSession}
          type="button"
        >
          <Plus size={16} />
          New Chat
        </button>

        <button
          className="mb-4 flex items-center justify-center gap-2 rounded-lg border border-zinc-800 px-3 py-2 text-sm text-zinc-300 hover:bg-zinc-900 disabled:opacity-60"
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

        <div className="mb-2 text-xs font-semibold uppercase tracking-wide text-zinc-500">
          Sessions
        </div>

        <div className="min-h-0 flex-1 space-y-2 overflow-y-auto">
          {sessions.length === 0 ? (
            <div className="rounded-lg border border-zinc-800 p-3 text-sm text-zinc-500">
              No saved sessions yet.
            </div>
          ) : (
            sessions.map((session) => (
              <button
                className={`w-full rounded-lg border p-3 text-left transition ${
                  session.session_id === activeSessionId
                    ? 'border-emerald-600 bg-emerald-950/30'
                    : 'border-zinc-800 hover:bg-zinc-900'
                }`}
                disabled={isStreaming}
                key={session.session_id}
                onClick={() => {
                  void handleSessionSelect(session.session_id);
                }}
                type="button"
              >
                <div className="flex items-center gap-2 text-sm font-medium">
                  <MessageSquare size={15} />
                  <span className="truncate">{session.title}</span>
                </div>
                <div className="mt-1 text-xs text-zinc-500">{sessionDescription(session)}</div>
              </button>
            ))
          )}
        </div>
      </aside>

      <main className="flex min-w-0 flex-1 flex-col">
        <header className="flex h-16 shrink-0 items-center justify-between border-b border-zinc-800 px-6">
          <div>
            <div className="text-sm font-medium">{activeSession?.title ?? 'New chat'}</div>
            <div className="text-xs text-zinc-500">Session: {activeSessionId}</div>
          </div>
          <div className="rounded-lg border border-zinc-800 px-3 py-1 text-xs text-zinc-400">
            {status}
          </div>
        </header>

        {error && (
          <div className="border-b border-red-900/60 bg-red-950/30 px-6 py-3 text-sm text-red-200">
            {error}
          </div>
        )}

        <div ref={scrollRef} className="min-h-0 flex-1 overflow-y-auto px-6 py-6">
          <div className="mx-auto flex max-w-3xl flex-col gap-4">
            {messages.length === 0 ? (
              <div className="rounded-lg border border-zinc-800 bg-zinc-900/40 p-6 text-zinc-400">
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
                    <div className="mt-1 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-zinc-800">
                      <Bot size={16} />
                    </div>
                  )}
                  <div
                    className={`max-w-[78%] whitespace-pre-wrap rounded-lg px-4 py-3 text-sm leading-6 ${
                      message.role === 'user'
                        ? 'bg-emerald-600 text-white'
                        : 'border border-zinc-800 bg-zinc-900 text-zinc-200'
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

        <footer className="shrink-0 border-t border-zinc-800 p-4">
          <form className="mx-auto flex max-w-3xl gap-3" onSubmit={handleSubmit}>
            <input
              className="min-w-0 flex-1 rounded-lg border border-zinc-800 bg-zinc-900 px-4 py-3 text-sm text-zinc-100 outline-none placeholder:text-zinc-500 focus:border-emerald-600"
              disabled={isStreaming}
              onChange={(event) => setInput(event.target.value)}
              placeholder="Message AEGIS"
              value={input}
            />
            <button
              className="flex items-center gap-2 rounded-lg bg-emerald-600 px-4 py-3 text-sm font-medium text-white hover:bg-emerald-500 disabled:opacity-60"
              disabled={isStreaming || !input.trim()}
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
