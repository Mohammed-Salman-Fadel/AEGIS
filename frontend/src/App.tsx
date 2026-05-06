import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { FormEvent } from 'react';
import { 
  Bot, Moon, Plus, RefreshCw, Send, Sun, Trash2, Upload, 
  User, Edit3, Download, Cpu, HardDrive
} from 'lucide-react';
import { useChatStore } from './store/useChatStore';

type Role = 'user' | 'assistant';
type ThemeMode = 'dark' | 'light';

interface Message {
  role: Role;
  content: string;
  sources?: string[];
}

interface EngineSessionSummary {
  session_id: string;
  title: string;
  turn_count: number;
  updated_at: string;
}

interface EngineTurn {
  query: string;
  response: string;
  sources?: string[];
}

const API_BASE = '/api';
const THEME_STORAGE_KEY = 'aegis-ui-theme';

function turnsToMessages(turns: EngineTurn[]): Message[] {
  return turns.flatMap((turn) => [
    { role: 'user' as const, content: turn.query },
    { role: 'assistant' as const, content: turn.response, sources: turn.sources },
  ]);
}

export default function App() {
  const { resources, currentTrace, updateResources, setTrace } = useChatStore(); 
  const [sessions, setSessions] = useState<EngineSessionSummary[]>([]);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState('');
  const [theme, setTheme] = useState<ThemeMode>(() => (window.localStorage.getItem(THEME_STORAGE_KEY) as ThemeMode) || 'dark');
  const [isStreaming, setIsStreaming] = useState(false);
  const [isUploading, setIsUploading] = useState(false);
  const [editingMessageIndex, setEditingMessageIndex] = useState<number | null>(null);
  const [deletingSessionIds, setDeletingSessionIds] = useState<string[]>([]);
  const [sessionToDelete, setSessionToDelete] = useState<EngineSessionSummary | null>(null);
  
  const scrollRef = useRef<HTMLDivElement>(null);
  const isDark = theme === 'dark';

  const loadSessions = useCallback(async () => {
    try {
      const response = await fetch(`${API_BASE}/sessions`);
      if (response.ok) {
        const data = await response.json();
        setSessions(data.sessions);
      }
    } catch (e) { console.error('Sessions load failed'); }
  }, []);

  useEffect(() => {
    loadSessions();
    const interval = setInterval(() => {
      updateResources(Math.floor(Math.random() * 8) + 5, Math.floor(Math.random() * 10) + 30);
    }, 5000);
    return () => clearInterval(interval);
  }, [loadSessions, updateResources]);

  useEffect(() => {
    window.localStorage.setItem(THEME_STORAGE_KEY, theme);
    document.documentElement.classList.toggle('dark', isDark);
  }, [theme, isDark]);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages, isStreaming]);

  const loadSession = useCallback(async (sessionId: string) => {
    const response = await fetch(`${API_BASE}/sessions/${encodeURIComponent(sessionId)}`);
    if (response.ok) {
      const session = await response.json();
      setActiveSessionId(session.session_id);
      setMessages(turnsToMessages(session.history.turns));
    }
  }, []);

  const createSession = useCallback(async () => {
    const response = await fetch(`${API_BASE}/sessions`, { method: 'POST' });
    if (response.ok) {
      const session = await response.json();
      setActiveSessionId(session.session_id);
      setMessages([]);
      await loadSessions();
      return session;
    }
  }, [loadSessions]);

  const handleExport = () => {
    const content = messages.map(m => `### ${m.role.toUpperCase()}\n${m.content}${m.sources ? `\nSources: ${m.sources.join(', ')}` : ''}`).join('\n\n---\n\n');
    const blob = new Blob([content], { type: 'text/markdown' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url; a.download = 'aegis-session.md'; a.click();
  };

  async function handleFileUpload(event: React.ChangeEvent<HTMLInputElement>) {
    const files = event.target.files;
    if (!files?.length || isUploading) return;
    setIsUploading(true); setTrace('RAG');
    const formData = new FormData();
    for (let i = 0; i < files.length; i++) formData.append('file', files[i]);
    try {
      await fetch(`${API_BASE}/ingest`, { method: 'POST', body: formData });
    } finally { setIsUploading(false); setTrace('Idle'); }
  }

  async function confirmDelete() {
    if (!sessionToDelete) return;
    const session = sessionToDelete;
    setDeletingSessionIds(prev => [...prev, session.session_id]);
    setSessionToDelete(null);
    try {
      const response = await fetch(`${API_BASE}/sessions/${encodeURIComponent(session.session_id)}`, { method: 'DELETE' });
      if (response.ok) {
        if (session.session_id === activeSessionId) { setActiveSessionId(null); setMessages([]); }
        await loadSessions();
      }
    } finally { setDeletingSessionIds(prev => prev.filter(id => id !== session.session_id)); }
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const prompt = input.trim();
    if (!prompt || isStreaming) return;

    setInput('');
    setIsStreaming(true);
    setTrace('Routing');

    let history = editingMessageIndex !== null ? messages.slice(0, editingMessageIndex) : [...messages];
    setEditingMessageIndex(null);
    setMessages([...history, { role: 'user', content: prompt }, { role: 'assistant', content: '' }]);

    try {
      let currentId = activeSessionId;
      if (!currentId) {
        const session = await createSession();
        currentId = session.session_id;
      }

      const response = await fetch(`${API_BASE}/chat`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ session_id: currentId, message: prompt }),
      });

      const reader = response.body?.getReader();
      const decoder = new TextDecoder();
      setTrace('Inference');

      while (reader) {
        const { done, value } = await reader.read();
        if (done) break;
        
        const chunk = decoder.decode(value, { stream: true });
        const lines = chunk.split('\n');
        
        for (const line of lines) {
          if (!line.startsWith('data:')) continue;
          
          const rawData = line.slice(5); 
          
          if (rawData.trim() === '[DONE]') continue;
          
          if (rawData.trim().startsWith('[SOURCES]')) {
            const sources = JSON.parse(rawData.trim().replace('[SOURCES]', ''));
            setMessages(prev => {
              const next = [...prev];
              next[next.length - 1].sources = sources;
              return next;
            });
            continue;
          }

          setMessages(prev => {
            const next = [...prev];
            const lastIdx = next.length - 1;
            if (next[lastIdx] && next[lastIdx].role === 'assistant') {
                const lastContent = next[lastIdx].content;
                next[lastIdx] = { ...next[lastIdx], content: lastContent + rawData };
            }
            return next;
          });
        }
      }
    } finally { setIsStreaming(false); setTrace('Idle'); loadSessions(); }
  }

  return (
    <div className={`flex h-screen overflow-hidden ${isDark ? 'bg-[#0f111a] text-zinc-100' : 'bg-slate-50 text-slate-900'}`}>
      <aside className={`flex w-72 shrink-0 flex-col border-r p-4 ${isDark ? 'border-zinc-800 bg-[#161925]' : 'border-slate-200 bg-white'}`}>
        <div className="mb-6 flex justify-between items-start px-2">
          <div>
            <div className="text-xl font-bold text-violet-500 tracking-tight">AEGIS</div>
            <div className="text-[10px] opacity-50 uppercase tracking-widest font-bold text-violet-400">Neural Engine</div>
          </div>
          <div className="flex flex-col items-end gap-1 font-mono text-[10px] opacity-60 text-violet-400">
            <div className="flex items-center gap-1"><Cpu size={10} /> {resources.cpu}%</div>
            <div className="flex items-center gap-1"><HardDrive size={10} /> {resources.ram}%</div>
          </div>
        </div>

        <button onClick={createSession} className="mb-4 flex items-center justify-center gap-2 rounded-xl bg-violet-600 px-3 py-2.5 text-sm font-bold text-white hover:bg-violet-500 transition-all shadow-lg shadow-violet-900/20 active:scale-95">
          <Plus size={18} /> New Chat
        </button>

        <div className="flex-1 overflow-y-auto space-y-2 custom-scrollbar px-1">
          {sessions.map((s) => (
            <div key={s.session_id} onClick={() => !isStreaming && loadSession(s.session_id)} className={`group cursor-pointer rounded-xl border p-3 transition-all ${activeSessionId === s.session_id ? 'border-violet-600 bg-violet-500/10 shadow-[0_0_15px_rgba(139,92,246,0.1)]' : 'border-zinc-800/50 hover:bg-zinc-900/50'}`}>
              <div className="flex items-center justify-between">
                <div className="truncate text-xs font-semibold max-w-[160px]">{s.title || "Untitled Session"}</div>
                <button onClick={(e) => { e.stopPropagation(); setSessionToDelete(s); }} className="opacity-0 group-hover:opacity-100 text-zinc-500 hover:text-red-500 transition-colors"><Trash2 size={14} /></button>
              </div>
              <div className="mt-1 text-[10px] opacity-40 uppercase tracking-tighter">{s.turn_count} turns</div>
            </div>
          ))}
        </div>

        <div className="mt-4 pt-4 border-t border-zinc-800/50">
          <button onClick={handleExport} disabled={messages.length === 0} className="flex w-full items-center gap-2 rounded-lg p-2 text-xs font-medium text-zinc-500 hover:bg-zinc-900 hover:text-violet-400 transition-colors disabled:opacity-20"><Download size={14} /> Export session (.md)</button>
        </div>
      </aside>

      <main className="flex min-w-0 flex-1 flex-col relative">
        {isUploading && <div className="absolute top-0 left-0 w-full h-1 bg-violet-500 animate-pulse z-50 shadow-[0_0_15px_rgba(139,92,246,0.5)]" />}
        <header className={`flex h-16 shrink-0 items-center justify-between border-b px-6 ${isDark ? 'border-zinc-800 bg-[#0f111a]/50 backdrop-blur-md' : 'border-slate-200 bg-white'}`}>
          <div className={`flex items-center gap-2 rounded-full px-3 py-1 text-[10px] font-bold border ${isDark ? 'bg-zinc-900 border-zinc-800 text-violet-400 shadow-[0_0_10px_rgba(139,92,246,0.1)]' : 'bg-violet-50 border-violet-100 text-violet-700'}`}>
            <div className={`h-1.5 w-1.5 rounded-full bg-violet-500 ${isStreaming ? 'animate-ping' : ''}`} />
            TRACE: {currentTrace}
          </div>
          <button onClick={() => setTheme(t => t === 'dark' ? 'light' : 'dark')} className="p-2 rounded-xl border border-zinc-800/50 hover:bg-zinc-900 transition-all hover:text-violet-400">{isDark ? <Sun size={16} /> : <Moon size={16} />}</button>
        </header>

        <div ref={scrollRef} className="flex-1 overflow-y-auto px-6 py-8 custom-scrollbar scroll-smooth">
          <div className="mx-auto max-w-3xl space-y-6">
            {messages.length === 0 ? (
              <div className="h-full flex flex-col items-center justify-center opacity-10 mt-20"><Bot size={80} /><p className="mt-4 font-bold tracking-widest uppercase text-xs text-center">Neural Link Active<br/>Waiting for synthesis request...</p></div>
            ) : (
              messages.map((m, i) => (
                <div key={i} className={`flex gap-4 ${m.role === 'user' ? 'flex-row-reverse' : ''}`}>
                  <div className={`h-9 w-9 shrink-0 rounded-xl flex items-center justify-center shadow-lg transition-transform hover:scale-105 ${m.role === 'assistant' ? 'bg-violet-500/10 text-violet-500 border border-violet-500/20' : 'bg-violet-600 text-white'}`}><Bot size={18} /></div>
                  <div className="group relative max-w-[85%]">
                    <div className={`rounded-2xl px-5 py-3.5 text-sm leading-relaxed shadow-sm break-words whitespace-pre-wrap ${m.role === 'user' ? 'bg-violet-600 text-white rounded-tr-none' : isDark ? 'bg-[#161925] border border-zinc-800 text-zinc-200 rounded-tl-none' : 'bg-white border border-slate-200 rounded-tl-none'}`}>
                      {m.content || (isStreaming && i === messages.length - 1 ? 'Synthesizing...' : '')}
                      {m.sources && m.sources.length > 0 && (
                        <div className="mt-4 flex flex-wrap gap-2 border-t border-zinc-800/50 pt-3">
                          <span className="w-full text-[9px] font-black opacity-30 tracking-widest mb-1 uppercase text-violet-400">Verified Sources</span>
                          {m.sources.map((s, idx) => (<span key={idx} className="text-[10px] bg-violet-500/10 text-violet-400 px-2 py-0.5 rounded border border-violet-500/20 font-semibold">{s}</span>))}
                        </div>
                      )}
                    </div>
                    {m.role === 'user' && !isStreaming && (
                      <button onClick={() => { setInput(m.content); setEditingMessageIndex(i); }} className="absolute -left-10 top-2 p-2 opacity-0 group-hover:opacity-100 text-zinc-500 hover:text-violet-400 transition-all"><Edit3 size={16} /></button>
                    )}
                  </div>
                </div>
              ))
            )}
          </div>
        </div>

        <footer className={`p-6 border-t ${isDark ? 'border-zinc-800 bg-[#0f111a]/95' : 'border-slate-200 bg-white/95'} backdrop-blur-lg`}>
          <form className="mx-auto max-w-3xl flex gap-3" onSubmit={handleSubmit}>
            <div className="relative flex-1 group">
              <input value={input} onChange={e => setInput(e.target.value)} placeholder={editingMessageIndex !== null ? "Edit your synthesis query..." : "Ask AEGIS..." } disabled={isStreaming} className={`w-full rounded-2xl border px-6 py-4 text-sm outline-none transition-all ${isDark ? 'bg-zinc-900 border-zinc-800 focus:border-violet-600/50 text-zinc-100 placeholder:text-zinc-600' : 'bg-slate-50 border-slate-300 focus:border-violet-500 text-slate-900'}`} />
              {editingMessageIndex !== null && <button onClick={() => setEditingMessageIndex(null)} className="absolute right-4 top-1/2 -translate-y-1/2 text-[10px] bg-zinc-800 px-2 py-1 rounded text-zinc-400 hover:text-white font-bold uppercase tracking-tighter">Cancel Edit</button>}
            </div>
            <label className={`flex items-center px-5 rounded-2xl border border-zinc-800/50 cursor-pointer transition-all ${isUploading ? 'opacity-30' : 'hover:bg-zinc-900 active:scale-95'}`}>
              <Upload size={20} className="text-violet-500" /><input accept=".pdf,.txt" className="hidden" disabled={isStreaming || isUploading} multiple onChange={handleFileUpload} type="file" />
            </label>
            <button type="submit" disabled={isStreaming || !input.trim()} className="bg-violet-600 text-white px-8 rounded-2xl font-bold hover:bg-violet-500 disabled:opacity-50 transition-all shadow-xl shadow-violet-900/30 active:scale-95">
              {isStreaming ? <RefreshCw size={20} className="animate-spin" /> : <Send size={20} />}
            </button>
          </form>
        </footer>
      </main>

      {sessionToDelete && (
        <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/80 backdrop-blur-sm p-4 animate-in fade-in duration-300">
          <div className={`w-full max-w-sm rounded-[2rem] border p-8 shadow-2xl ${isDark ? 'bg-[#161925] border-zinc-800' : 'bg-white border-slate-200'}`}>
            <div className="h-14 w-14 bg-red-500/10 text-red-500 rounded-full flex items-center justify-center mb-6 mx-auto animate-bounce"><Trash2 size={28} /></div>
            <h3 className="text-xl font-bold text-center mb-2 uppercase tracking-tight text-white">Delete Session?</h3>
            <p className="text-sm opacity-60 text-center mb-8 italic">"{sessionToDelete.title || "New Session"}" will be permanently removed.</p>
            <div className="flex gap-4">
              <button onClick={() => setSessionToDelete(null)} className="flex-1 rounded-2xl py-3 text-sm font-bold bg-zinc-800 hover:bg-zinc-700 transition-all text-white">Cancel</button>
              <button onClick={() => void confirmDelete()} className="flex-1 rounded-2xl py-3 text-sm font-bold bg-red-600 hover:bg-red-500 transition-all shadow-lg shadow-red-900/40 text-white">Purge</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}