import React, { useState, useEffect, useRef } from 'react';
import { 
  ShieldCheck, Cpu, Database, Send, Activity, FileText, 
  Trash2, Upload, X, CheckCircle2, Loader2, Sparkles,
  Lock, MessageSquare, Plus, Edit2, Zap
} from 'lucide-react';
import { useChatStore } from './store/useChatStore';

interface ChatSession {
  id: string;
  name: string;
  files: string[];
}

const apiPath = (path: string) => path;

const websocketPath = (path: string) => {
  const protocol = window.location.protocol === 'https:' ? 'wss' : 'ws';
  return `${protocol}://${window.location.host}${path}`;
};

const AegisApp: React.FC = () => {
  const { 
    messages, currentTrace, resources, isStreaming,
    addMessage, updateStreamingMessage, setTrace, 
    updateResources, resetChat 
  } = useChatStore();
  
  const [input, setInput] = useState('');
  const [selectedFiles, setSelectedFiles] = useState<File[]>([]);
  const [isIndexing, setIsIndexing] = useState(false);
  const [indexProgress, setIndexProgress] = useState(0);
  const [showSuccess, setShowSuccess] = useState(false);
  
  const [sessions, setSessions] = useState<ChatSession[]>([]);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [tempName, setTempName] = useState('');

  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (sessions.length === 0) {
      const newId = Math.random().toString(36).substring(7);
      setSessions([{ id: newId, name: 'Initial Research', files: [] }]);
      setActiveSessionId(newId);
    }
  }, [sessions.length]);

  useEffect(() => {
    const fetchStats = async () => {
      try {
        const res = await fetch(apiPath('/system/stats'));
        const data = await res.json();
        updateResources(data.cpu, data.ram);
      } catch (e) { console.log("Engine offline"); }
    };
    const interval = setInterval(fetchStats, 2000);
    return () => clearInterval(interval);
  }, [updateResources]);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTo({ top: scrollRef.current.scrollHeight, behavior: 'smooth' });
    }
  }, [messages, isStreaming]);

  const handleNewChat = () => {
    const newId = Math.random().toString(36).substring(7);
    setSessions(prev => [{ id: newId, name: `New Strategy ${prev.length + 1}`, files: [] }, ...prev]);
    setActiveSessionId(newId);
    resetChat();
  };

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files) {
      setSelectedFiles(Array.from(e.target.files));
      setShowSuccess(false);
    }
  };

  const handleStartIndexing = async () => {
    if (selectedFiles.length === 0) return;
    setIsIndexing(true);
    setIndexProgress(20);
    const formData = new FormData();
    selectedFiles.forEach(file => formData.append('files', file));
    const fileNames = selectedFiles.map(f => f.name);

    try {
      const response = await fetch(apiPath('/ingest'), { method: 'POST', body: formData });
      if (response.ok) {
        setIndexProgress(100);
        setSessions(prev => prev.map(s => s.id === activeSessionId ? { ...s, files: [...s.files, ...fileNames] } : s));
        setShowSuccess(true);
        setIsIndexing(false);
        setSelectedFiles([]);
        setTimeout(() => setShowSuccess(false), 3000);
      }
    } catch (e) { setIsIndexing(false); }
  };

  const handleSendMessage = () => {
    if (!input.trim() || isStreaming) return;
    const userQuery = input;
    addMessage(userQuery, 'user');
    setInput('');
    setTrace('Routing');

    const socket = new WebSocket(websocketPath('/chat/stream'));
    socket.onopen = () => socket.send(JSON.stringify({ query: userQuery, session_id: activeSessionId }));
    socket.onmessage = (event) => {
      const data = JSON.parse(event.data);
      if (data.type === 'token') {
        if (messages.length === 0 || messages[messages.length - 1].role !== 'assistant') addMessage('', 'assistant');
        updateStreamingMessage(data.content);
      }
    };
    socket.onclose = () => setTrace('Complete');
  };

  const saveName = (id: string) => {
    setSessions(prev => prev.map(s => s.id === id ? { ...s, name: tempName } : s));
    setEditingId(null);
  };

  const activeSession = sessions.find(s => s.id === activeSessionId);

  return (
    <div className="flex h-screen bg-[#09090B] text-zinc-100 font-sans overflow-hidden">
      <aside className="w-72 border-r border-zinc-800 bg-[#0C0C0E] flex flex-col p-5 shrink-0 z-30">
        <div className="flex items-center gap-3 mb-10 px-2">
          <div className="p-2 bg-indigo-600 rounded-xl"><ShieldCheck size={22} className="text-white" /></div>
          <span className="font-black text-xl tracking-tighter italic">AEGIS</span>
        </div>

        <button onClick={handleNewChat} className="flex items-center gap-3 w-full p-4 mb-8 rounded-2xl bg-indigo-600 hover:bg-indigo-500 text-white transition-all shadow-lg active:scale-95">
          <Plus size={20} /> <span className="text-xs font-black uppercase tracking-widest">New Session</span>
        </button>

        <div className="flex-1 overflow-y-auto space-y-3">
          <span className="text-[10px] font-black text-zinc-600 uppercase tracking-[0.3em] px-2">History</span>
          {sessions.map(s => (
            <div key={s.id} onClick={() => setActiveSessionId(s.id)} className={`group flex items-center justify-between p-4 rounded-2xl cursor-pointer border ${activeSessionId === s.id ? 'bg-zinc-800 border-zinc-700' : 'hover:bg-zinc-900/50 border-transparent'}`}>
              <div className="flex items-center gap-3 overflow-hidden">
                <MessageSquare size={16} className={activeSessionId === s.id ? 'text-indigo-400' : 'text-zinc-600'} />
                {editingId === s.id ? (
                  <input autoFocus className="bg-transparent outline-none text-xs font-bold w-full" value={tempName} onChange={e => setTempName(e.target.value)} onBlur={() => saveName(s.id)} onKeyDown={e => e.key === 'Enter' && saveName(s.id)} />
                ) : (
                  <span className={`text-xs font-bold truncate ${activeSessionId === s.id ? 'text-zinc-100' : 'text-zinc-500'}`}>{s.name}</span>
                )}
              </div>
              <Edit2 size={12} className="opacity-0 group-hover:opacity-100 text-zinc-600" onClick={(e) => { e.stopPropagation(); setEditingId(s.id); setTempName(s.name); }} />
            </div>
          ))}
        </div>
        <button onClick={resetChat} className="mt-4 p-3 flex items-center gap-3 text-zinc-500 hover:text-red-400 transition-colors">
          <Trash2 size={18} /> <span className="text-xs font-bold uppercase tracking-wider">Clear Memory</span>
        </button>
      </aside>

      <div className="flex-1 flex flex-col bg-[#0F0F12]">
        <header className="h-16 border-b border-zinc-800/50 bg-[#09090B]/50 backdrop-blur-xl flex items-center justify-between px-10">
          <div className="flex items-center gap-2 px-3 py-1 rounded-full bg-emerald-500/5 border border-emerald-500/20">
            <Lock size={10} className="text-emerald-500" />
            <span className="text-[9px] font-black text-emerald-500 uppercase tracking-widest leading-none">Secure Tunnel</span>
          </div>
          <div className="flex gap-8">
            <div className="w-28 flex flex-col justify-center">
              <div className="flex justify-between text-[9px] font-black mb-1"><div className="flex items-center gap-1"><Database size={10} className="text-zinc-500"/><span>RAM</span></div><span className="text-indigo-400">{resources.ram}%</span></div>
              <div className="h-1 bg-zinc-800 rounded-full overflow-hidden"><div className="h-full bg-indigo-500 transition-all duration-1000" style={{ width: `${resources.ram}%` }} /></div>
            </div>
            <div className="w-28 flex flex-col justify-center">
              <div className="flex justify-between text-[9px] font-black mb-1"><div className="flex items-center gap-1"><Cpu size={10} className="text-zinc-500"/><span>CPU</span></div><span className="text-purple-400">{resources.cpu}%</span></div>
              <div className="h-1 bg-zinc-800 rounded-full overflow-hidden"><div className="h-full bg-purple-500 transition-all duration-1000" style={{ width: `${resources.cpu}%` }} /></div>
            </div>
          </div>
        </header>

        <div className="bg-[#0C0C0E] border-b border-zinc-800/50 p-6 px-10">
          <div className="max-w-4xl mx-auto flex items-center justify-between">
            <div className="flex items-center gap-4">
              <div className="p-2.5 bg-indigo-500/10 rounded-xl border border-indigo-500/20 text-indigo-400"><Sparkles size={18} /></div>
              <div>
                <h2 className="text-[10px] font-black text-zinc-500 uppercase tracking-[0.2em]">Session Assets</h2>
                <div className="flex flex-wrap gap-2 mt-1">
                  {activeSession?.files.map((name, i) => (
                    <div key={i} className="flex items-center gap-1 bg-indigo-500/5 px-2 py-1 rounded border border-indigo-500/20 text-[9px] font-bold text-indigo-400">
                      <FileText size={10} /> {name}
                    </div>
                  ))}
                </div>
              </div>
            </div>
            <div className="flex gap-3">
              <input type="file" id="file-ingest" multiple onChange={handleFileChange} className="hidden" />
              <label htmlFor="file-ingest" className="cursor-pointer bg-zinc-800 hover:bg-zinc-700 px-4 py-2 rounded-xl text-xs font-bold border border-zinc-700 flex items-center gap-2">
                <Upload size={14} /> IMPORT
              </label>
              {selectedFiles.length > 0 && (
                <button onClick={handleStartIndexing} className="bg-indigo-600 px-5 py-2 rounded-xl text-xs font-black uppercase tracking-widest flex items-center gap-2 shadow-lg">
                  {isIndexing ? <Loader2 size={14} className="animate-spin" /> : <Zap size={14} />} 
                  {isIndexing ? `${indexProgress}%` : 'Sync'}
                </button>
              )}
            </div>
          </div>
          {showSuccess && <div className="max-w-4xl mx-auto mt-2 text-[9px] font-bold text-emerald-400 flex items-center gap-2 animate-pulse"><CheckCircle2 size={12}/> INDEXED SUCCESSFULLY</div>}
        </div>

        <main ref={scrollRef} className="flex-1 overflow-y-auto p-10 space-y-10 max-w-4xl mx-auto w-full">
          {messages.map((m, i) => (
            <div key={i} className={`flex ${m.role === 'user' ? 'justify-end' : 'justify-start'} animate-in fade-in duration-500`}>
              <div className={`p-6 rounded-[2rem] text-sm leading-relaxed max-w-[80%] ${m.role === 'user' ? 'bg-indigo-600 text-white shadow-xl shadow-indigo-900/20' : 'bg-zinc-900 border border-zinc-800 text-zinc-300'}`}>
                {m.content}
              </div>
            </div>
          ))}
        </main>

        <footer className="p-10">
          <div className="max-w-3xl mx-auto flex flex-col gap-6">
            <div className="flex items-center gap-3 px-4 py-2 bg-zinc-900/50 border border-zinc-800 rounded-2xl w-fit mx-auto backdrop-blur-md shadow-sm">
              <Activity size={12} className="text-indigo-400" />
              <span className="text-[9px] font-black text-zinc-500 uppercase tracking-widest">Status: <span className="text-indigo-400">{currentTrace}</span></span>
            </div>
            <div className="relative group flex items-center">
              <input value={input} onChange={(e) => setInput(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && handleSendMessage()} placeholder="Search local documents or ask anything..." className="w-full p-6 pr-16 bg-zinc-900 border border-zinc-800 rounded-[2rem] outline-none focus:border-indigo-500 transition-all text-zinc-100 placeholder:text-zinc-700 shadow-2xl" />
              <div className="absolute right-3 flex items-center gap-2">
                {selectedFiles.length > 0 && <X size={16} className="text-zinc-500 cursor-pointer hover:text-red-400" onClick={() => setSelectedFiles([])} />}
                <button onClick={handleSendMessage} className="p-4 rounded-2xl bg-indigo-600 text-white shadow-xl hover:bg-indigo-500 transition-all active:scale-90"><Send size={20} /></button>
              </div>
            </div>
          </div>
        </footer>
      </div>
    </div>
  );
};

export default AegisApp;
