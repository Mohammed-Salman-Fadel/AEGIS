// Obsidian vault interaction modal — search, read, create, list, and graph notes
import { useState, useRef, useCallback, useEffect } from 'react';
import { X, Search, FileText, Plus, TreePine, BookOpen, Loader, Share2 } from 'lucide-react';
import { API_BASE } from '../constants';
import { AssistantMarkdown } from './AssistantMarkdown';

type ObsidianTab = 'search' | 'read' | 'create' | 'tree' | 'graph';

interface TreeNode {
  name: string;
  children: Map<string, TreeNode>;
  isFile: boolean;
}

function buildTree(paths: string[]): TreeNode {
  const root: TreeNode = { name: '', children: new Map(), isFile: false };
  for (const p of paths) {
    const parts = p.replace(/\\/g, '/').split('/');
    let node = root;
    for (let i = 0; i < parts.length; i++) {
      const part = parts[i];
      if (!node.children.has(part)) {
        node.children.set(part, { name: part, children: new Map(), isFile: i === parts.length - 1 });
      }
      node = node.children.get(part)!;
    }
  }
  return root;
}

function sortEntries(node: TreeNode): [string, TreeNode][] {
  return [...node.children.entries()].sort((a, b) => {
    if (a[1].isFile !== b[1].isFile) return a[1].isFile ? 1 : -1;
    return a[0].localeCompare(b[0]);
  });
}

function renderTree(node: TreeNode, prefix = '', isLast = true): string {
  let result = '';
  const entries = sortEntries(node);
  for (let i = 0; i < entries.length; i++) {
    const [name, child] = entries[i];
    const last = i === entries.length - 1;
    const connector = last ? '└── ' : '├── ';
    result += prefix + connector + (child.isFile ? name.replace(/\.md$/i, '') : '📁 ' + name) + '\n';
    if (!child.isFile) {
      const childPrefix = prefix + (last ? '    ' : '│   ');
      result += renderTree(child, childPrefix, last);
    }
  }
  return result;
}

interface ObsidianModalProps {
  isDark: boolean;
  isOpen: boolean;
  onClose: () => void;
  vaultPath?: string;
}

export function ObsidianModal({ isDark, isOpen, onClose, vaultPath }: ObsidianModalProps) {
  const [tab, setTab] = useState<ObsidianTab>('search');
  const [query, setQuery] = useState('');
  const [notePath, setNotePath] = useState('');
  const [newNotePath, setNewNotePath] = useState('');
  const [newNoteName, setNewNoteName] = useState('');
  const [newNoteContent, setNewNoteContent] = useState('');
  const [results, setResults] = useState('');
  const [loading, setLoading] = useState(false);
  const [graphData, setGraphData] = useState<any>(null);
  const [graphLoading, setGraphLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [readTree, setReadTree] = useState<TreeNode | null>(null);
  const [treeData, setTreeData] = useState<TreeNode | null>(null);
  const [treeLoading, setTreeLoading] = useState(false);
  const [searchResults, setSearchResults] = useState<any[] | null>(null);
  const [searchLoading, setSearchLoading] = useState(false);
  const [selectedNoteContent, setSelectedNoteContent] = useState<string | null>(null);
  const [selectedNotePath, setSelectedNotePath] = useState<string | null>(null);
  const searchTimer = useRef<any>(null);
  const [showPreview, setShowPreview] = useState(false);
  const [editingNote, setEditingNote] = useState(false);
  const [createSuccess, setCreateSuccess] = useState('');

  if (!isOpen) return null;

  async function callObsidian(action: string, body: Record<string, unknown>) {
    setLoading(true);
    setError(null);
    const bodyWithPath = { ...body };
    if (vaultPath) bodyWithPath.vault_path = vaultPath;
    try {
      const controller = new AbortController();
      const timeout = setTimeout(() => controller.abort(), 90000);
      const res = await fetch(`${API_BASE}/mcp/obsidian/${action}`, {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(bodyWithPath),
        signal: controller.signal,
      });
      clearTimeout(timeout);
      if (!res.ok) { const text = await res.text(); throw new Error(text || `Request failed`); }
      const data = await res.json();
      setResults(data.result || '(empty)');
    } catch (e) {
      if (e instanceof DOMException && e.name === 'AbortError') {
        setError('Request timed out. The MCP subprocess may not be responding.');
      } else {
        setError(e instanceof Error ? e.message : 'An error occurred');
      }
      setResults('');
    } finally { setLoading(false); }
  }

  async function handleSearch() { if (!query.trim()) return; await callObsidian('search-vault', { query: query.trim() }); }
  async function handleRead() {
    if (!notePath.trim() || !vaultPath?.trim()) return;
    setLoading(true); setError(null);
    try {
      const res = await fetch(`${API_BASE}/mcp/obsidian/read`, {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ vault_path: vaultPath.trim(), path: notePath.trim() }),
      });
      if (!res.ok) { const text = await res.text(); throw new Error(text || 'Failed to read note'); }
      const data = await res.json();
      setResults(data.content || '(empty)');
    } catch (e: any) {
      setError(e instanceof Error ? e.message : 'An error occurred');
      setResults('');
    } finally { setLoading(false); }
  }
  async function handleCreate() {
    const name = newNoteName.trim().replace(/\.md$/i, '') + '.md';
    const fullPath = newNotePath.trim() ? `${newNotePath.trim()}/${name}` : name;
    if (!newNoteName.trim() || !newNoteContent.trim() || !vaultPath?.trim()) return;
    try {
      setLoading(true);
      const res = await fetch(`${API_BASE}/mcp/obsidian/write`, {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ vault_path: vaultPath.trim(), path: fullPath, content: newNoteContent.trim() }),
      });
      if (!res.ok) { const text = await res.text(); throw new Error(text || 'Failed to create note'); }
      setCreateSuccess(`Note created successfully.`);
      setTimeout(() => setCreateSuccess(''), 5000);
      // Reload tree views
      setTreeData(null);
      setReadTree(null);
      setTimeout(() => { handleList(); loadReadTree(); }, 300);
    } catch (e: any) {
      setError(e instanceof Error ? e.message : 'An error occurred');
    } finally { setLoading(false); }
  }
  async function loadReadTree() {
    if (!vaultPath?.trim()) return;
    setReadTree(null);
    try {
      const res = await fetch(`${API_BASE}/mcp/obsidian/list-notes`, {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ vault_path: vaultPath.trim(), max_notes: 5000 }),
      });
      if (!res.ok) return;
      const data = await res.json();
      if (data.notes?.length > 0) {
        setReadTree(buildTree(data.notes.map((n: any) => n.id)));
      }
    } catch (_) {}
  }

  async function handleList() {
    if (!vaultPath?.trim() || treeData) return;
    setTreeLoading(true); setError(null);
    try {
      const controller = new AbortController();
      const timeout = setTimeout(() => controller.abort(), 15000);
      const res = await fetch(`${API_BASE}/mcp/obsidian/list-notes`, {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ vault_path: vaultPath.trim(), max_notes: 5000 }),
        signal: controller.signal,
      });
      clearTimeout(timeout);
      if (!res.ok) { const text = await res.text(); throw new Error(text || 'Failed to list notes'); }
      const data = await res.json();
      if (data.notes?.length === 0) {
        setTreeData(buildTree(['(empty vault)']));
      } else {
        setTreeData(buildTree(data.notes.map((n: any) => n.id)));
      }
    } catch (e: any) {
      if (e.name === 'AbortError') setError('Request timed out.');
      else setError(e instanceof Error ? e.message : 'An error occurred');
    } finally { setTreeLoading(false); }
  }

  function handleSearchInput(val: string) {
    setQuery(val);
    setSelectedNoteContent(null);
    setSelectedNotePath(null);
    if (searchTimer.current) clearTimeout(searchTimer.current);
    if (!val.trim()) { setSearchResults(null); return; }
    setSearchLoading(true);
    searchTimer.current = setTimeout(async () => {
      try {
        const controller = new AbortController();
        const timeout = setTimeout(() => controller.abort(), 8000);
        const res = await fetch(`${API_BASE}/mcp/obsidian/search`, {
          method: 'POST', headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ vault_path: vaultPath?.trim(), query: val.trim(), max_results: 30 }),
          signal: controller.signal,
        });
        clearTimeout(timeout);
        if (!res.ok) return;
        const data = await res.json();
        setSearchResults(data.results || []);
      } catch (_) {
        setSearchResults([]);
      } finally { setSearchLoading(false); }
    }, 200);
  }

  async function loadSearchNote(path: string) {
    if (!vaultPath?.trim()) return;
    setEditingNote(false);
    try {
      const res = await fetch(`${API_BASE}/mcp/obsidian/read`, {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ vault_path: vaultPath.trim(), path }),
      });
      if (!res.ok) return;
      const data = await res.json();
      setSelectedNoteContent(data.content || '');
      setSelectedNotePath(path);
      setSearchResults(null);
    } catch (_) {}
  }

  async function saveSearchNote() {
    if (!vaultPath?.trim() || !selectedNotePath) return;
    try {
      const res = await fetch(`${API_BASE}/mcp/obsidian/write`, {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ vault_path: vaultPath.trim(), path: selectedNotePath, content: selectedNoteContent }),
      });
      if (!res.ok) return;
      setEditingNote(false);
    } catch (_) {}
  }

  async function handleLoadGraph() {
    if (!vaultPath?.trim()) return;
    setGraphLoading(true); setError(null);
    try {
      const controller = new AbortController();
      const timeout = setTimeout(() => controller.abort(), 15000);
      const res = await fetch(`${API_BASE}/mcp/obsidian/graph`, {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ vault_path: vaultPath.trim(), max_notes: 1000 }),
        signal: controller.signal,
      });
      clearTimeout(timeout);
      if (!res.ok) { const text = await res.text(); throw new Error(text || 'Failed to load graph'); }
      const data = await res.json();
      if (data.nodes?.length === 0 && data.edges?.length === 0) {
        setError('No notes found in this vault. Make sure it contains .md files with [[wikilinks]].');
      } else {
        setGraphData(data);
      }
    } catch (e: any) {
      if (e.name === 'AbortError') setError('Graph request timed out. Try a smaller vault or check the path.');
      else setError(e instanceof Error ? e.message : 'Failed to load graph');
    }
    finally { setGraphLoading(false); }
  }

  const inputClass = `w-full rounded-lg border px-3 py-2 text-sm outline-none focus:border-emerald-600 ${isDark ? 'border-zinc-800 bg-zinc-900 text-zinc-100 placeholder:text-zinc-500' : 'border-stone-300 bg-white text-slate-900 placeholder:text-slate-400'}`;
  const resultClass = `mt-3 max-h-48 overflow-y-auto rounded-lg border p-3 text-sm leading-6 whitespace-pre-wrap ${isDark ? 'border-zinc-800 bg-zinc-900/60 text-zinc-300' : 'border-stone-200 bg-stone-50 text-slate-700'}`;

  const tabBtn = (t: ObsidianTab, label: string, Icon: any) => (
    <button
      className={`flex-1 px-3 py-2.5 text-xs font-semibold uppercase tracking-wider transition ${tab === t ? 'border-b-2 border-emerald-500 text-emerald-500' : isDark ? 'text-zinc-500 hover:text-zinc-300' : 'text-slate-500 hover:text-slate-700'}`}
      onClick={() => { setTab(t); if (t === 'read' && !readTree) loadReadTree(); if (t === 'tree' && !treeData) handleList(); if (t === 'graph' && !graphData) handleLoadGraph(); }}
      type="button"
    ><Icon size={14} className="inline mr-1" />{label}</button>
  );

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4" onClick={onClose}>
      <div className={`flex w-[85vw] h-[85vh] flex-col rounded-2xl border shadow-2xl ${isDark ? 'border-zinc-800 bg-zinc-950 text-zinc-100' : 'border-stone-300 bg-white text-slate-900'}`} onClick={(e) => e.stopPropagation()}>
        <div className={`flex items-center justify-between px-6 py-4 border-b shrink-0 ${isDark ? 'border-zinc-800' : 'border-stone-200'}`}>
          <div className="flex items-center gap-2 text-base font-semibold"><BookOpen size={18} />Obsidian Vault</div>
          <button className={`rounded-md p-1 transition ${isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'}`} onClick={onClose} type="button"><X size={18} /></button>
        </div>

        <div className={`flex border-b shrink-0 ${isDark ? 'border-zinc-800' : 'border-stone-200'}`}>
          {tabBtn('search', 'Search', Search)}{tabBtn('read', 'Read', FileText)}{tabBtn('create', 'Create', Plus)}{tabBtn('tree', 'Tree', TreePine)}{tabBtn('graph', 'Graph', Share2)}
        </div>

        <div className="flex-1 overflow-y-auto space-y-4 px-6 py-4">
          {error && <div className={`rounded-lg border px-3 py-2 text-xs ${isDark ? 'border-red-900/60 bg-red-950/30 text-red-200' : 'border-red-200 bg-red-50 text-red-700'}`}>{error}</div>}
          {!vaultPath?.trim() && <p className="text-sm text-amber-500">No vault path configured. Set it in Settings → Tools → Obsidian.</p>}

          {tab === 'search' && (
            <div className="relative">
              <div className="relative mb-3">
                <input className={`${inputClass} w-full ${searchResults !== null && searchResults.length === 0 && query.trim() ? 'border-red-500 focus:border-red-500' : ''}`} value={query} onChange={(e) => handleSearchInput(e.target.value)} placeholder="Search your vault..." autoFocus />
                {searchLoading && <Loader size={14} className="animate-spin text-emerald-500 absolute right-3 top-1/2 -translate-y-1/2" />}
              </div>
              {searchResults && searchResults.length > 0 && !selectedNoteContent && (
                <div className={`rounded-lg border shadow-lg max-h-48 overflow-y-auto ${isDark ? 'border-zinc-700 bg-zinc-900' : 'border-stone-200 bg-white'}`}>
                  {searchResults.map((r: any) => (
                    <div key={r.path} className={`px-3 py-2 text-sm cursor-pointer border-b last:border-0 ${isDark ? 'border-zinc-800 hover:bg-zinc-800' : 'border-stone-100 hover:bg-stone-100'}`} onClick={() => loadSearchNote(r.path)}>
                      <div className={`font-medium ${isDark ? 'text-zinc-200' : 'text-slate-800'}`}>{r.name}</div>
                      <div className={`text-xs truncate ${isDark ? 'text-zinc-500' : 'text-slate-400'}`}>{r.path}{r.snippet ? ` — ${r.snippet}` : ''}</div>
                    </div>
                  ))}
                </div>
              )}
              {!query.trim() && !selectedNoteContent && <p className={`text-sm italic ${isDark ? 'text-zinc-500' : 'text-slate-400'}`}>Start typing to search your vault.</p>}
            </div>
          )}
          {/* Search note reader — fills remaining space (rendered at content div level like Graph tab) */}
          {tab === 'search' && selectedNoteContent !== null && selectedNotePath && (
            <div style={{ height: 'calc(100% - 20px)' }} className="flex flex-col space-y-2">
              <div className="flex items-center justify-between shrink-0">
                <span className={`text-xs ${isDark ? 'text-zinc-500' : 'text-slate-400'}`}>{selectedNotePath}</span>
                {!editingNote && (
                  <button className={`text-xs transition ${isDark ? 'text-zinc-400 hover:text-zinc-200' : 'text-slate-500 hover:text-slate-800'}`} onClick={() => setEditingNote(true)} type="button">Edit</button>
                )}
              </div>
              <div className="flex-1 min-h-0">
                {editingNote ? (
                  <div className="h-full flex flex-col space-y-2">
                    <textarea className={`${inputClass} flex-1 min-h-0 resize-none font-mono text-sm`} value={selectedNoteContent} onChange={(e) => setSelectedNoteContent(e.target.value)} />
                    <div className="flex gap-2 shrink-0">
                      <button className="rounded-lg bg-emerald-600 px-4 py-2 text-sm font-medium text-white hover:bg-emerald-500" onClick={saveSearchNote} type="button">Save</button>
                      <button className={`rounded-lg border px-4 py-2 text-sm transition ${isDark ? 'border-zinc-700 text-zinc-300 hover:bg-zinc-900' : 'border-stone-300 text-slate-700 hover:bg-stone-100'}`} onClick={() => { loadSearchNote(selectedNotePath); setEditingNote(false); }} type="button">Cancel</button>
                    </div>
                  </div>
                ) : (
                  <div className={`h-full rounded-lg border p-3 text-sm leading-6 overflow-y-auto ${isDark ? 'border-zinc-800 bg-zinc-900/60' : 'border-stone-200 bg-stone-50'}`}>
                    <AssistantMarkdown content={selectedNoteContent} isDark={isDark} vaultPath={vaultPath} noteDir={selectedNotePath.includes('/') ? selectedNotePath.slice(0, selectedNotePath.lastIndexOf('/')) : undefined} />
                  </div>
                )}
              </div>
            </div>
          )}

          {tab === 'read' && (
            <div className="space-y-3">
              <div className="flex gap-2">
                <input className={inputClass} value={notePath} onChange={(e) => setNotePath(e.target.value)} placeholder="Note path (e.g. Projects/MyNote)" onKeyDown={(e) => { if (e.key === 'Enter') handleRead(); }} />
                <button className="shrink-0 rounded-lg bg-emerald-600 px-4 py-2 text-sm font-medium text-white hover:bg-emerald-500 disabled:opacity-60" disabled={loading || !notePath.trim()} onClick={handleRead} type="button">{loading ? <Loader size={16} className="animate-spin" /> : <FileText size={16} />}</button>
              </div>
              <p className={`text-xs ${isDark ? 'text-zinc-500' : 'text-slate-400'}`}>Browse vault</p>
              {readTree ? (
                <div className={`rounded-lg border p-3 text-sm leading-5 max-h-64 overflow-y-auto ${isDark ? 'border-zinc-800 bg-zinc-900/60' : 'border-stone-200 bg-stone-50'}`}>
                  {(() => {
                    const flat: { path: string; name: string; depth: number; isFile: boolean }[] = [];
                    function walk(node: TreeNode, prefix: string) {
                      for (const [n, c] of sortEntries(node)) {
                        const full = prefix ? `${prefix}/${n}` : n;
                        flat.push({ path: full, name: c.isFile ? n.replace(/\.md$/i, '') : n, depth: prefix ? prefix.split('/').length : 0, isFile: c.isFile });
                        if (!c.isFile) walk(c, full);
                      }
                    }
                    walk(readTree, '');
                    return flat.map((item) => (
                      <div key={item.path} className={`flex items-center gap-1 rounded px-1 ${item.isFile ? 'cursor-pointer hover:bg-emerald-500/10' : ''}`} style={{ paddingLeft: `${item.depth * 1.25 + 0.5}rem` }} onClick={() => { if (item.isFile) setNotePath(item.path); }}>
                        <span className="shrink-0 opacity-40">{item.isFile ? '├──' : '📁'}</span>
                        <span className={item.isFile ? 'hover:text-emerald-500' : 'opacity-70'}>{item.name}</span>
                      </div>
                    ));
                  })()}
                </div>
              ) : (
                <p className={`text-sm italic ${isDark ? 'text-zinc-500' : 'text-slate-400'}`}>Loading vault structure...</p>
              )}
              {results && (
                <div className={`rounded-lg border p-3 text-sm leading-6 ${isDark ? 'border-zinc-800 bg-zinc-900/60' : 'border-stone-200 bg-stone-50'}`}>
                  <AssistantMarkdown content={results} isDark={isDark} vaultPath={vaultPath} noteDir={notePath.includes('/') ? notePath.slice(0, notePath.lastIndexOf('/')) : undefined} />
                </div>
              )}
            </div>
          )}

          {tab === 'create' && (
            <div className="space-y-3">
              <div className="flex gap-2">
                <input className={`${inputClass} flex-1`} value={newNotePath} onChange={(e) => setNewNotePath(e.target.value)} placeholder="Folder (optional, e.g. Projects/Linked List)" />
                <input className={`${inputClass} flex-1`} value={newNoteName} onChange={(e) => setNewNoteName(e.target.value)} placeholder="Note name (required)" />
              </div>
              <textarea className={`${inputClass} min-h-[120px] resize-none font-mono`} value={newNoteContent} onChange={(e) => { setNewNoteContent(e.target.value); setShowPreview(false); }} placeholder="Note content (markdown supported)" />
              {createSuccess && (
                <div className={`rounded-lg border px-3 py-2 text-xs ${isDark ? 'border-emerald-900/60 bg-emerald-950/30 text-emerald-200' : 'border-emerald-200 bg-emerald-50 text-emerald-700'}`}>{createSuccess}</div>
              )}
              <div className="flex gap-2">
                <button className="flex-1 rounded-lg bg-emerald-600 px-4 py-2 text-sm font-medium text-white hover:bg-emerald-500 disabled:opacity-60" disabled={!newNoteName.trim() || !newNoteContent.trim() || loading} onClick={handleCreate} type="button">{loading ? 'Creating...' : 'Create Note'}</button>
                <button className={`rounded-lg border px-4 py-2 text-sm transition disabled:opacity-40 active:scale-95 hover:bg-emerald-500/10 hover:border-emerald-500/50 ${showPreview ? 'bg-emerald-500/10 border-emerald-500/50 text-emerald-600' : ''}`} disabled={!newNoteContent.trim()} onClick={() => setShowPreview((v) => !v)} type="button">Render Markdown</button>
              </div>
              {showPreview && newNoteContent.trim() && (
                <div className={`rounded-lg border p-3 text-sm leading-6 ${isDark ? 'border-zinc-800 bg-zinc-900/60' : 'border-stone-200 bg-stone-50'}`}>
                  <AssistantMarkdown content={newNoteContent} isDark={isDark} vaultPath={vaultPath} noteDir={newNotePath ? newNotePath.replace(/^\/+|\/+$/g, '') : undefined} />
                </div>
              )}
            </div>
          )}

          {tab === 'tree' && (
            treeLoading ? (
              <div className="flex items-center justify-center py-16"><Loader size={24} className="animate-spin text-emerald-500" /><span className="ml-3 text-sm text-zinc-500">Loading vault structure...</span></div>
            ) : treeData ? (
              <div style={{ height: 'calc(100% - 20px)' }} className="flex flex-col space-y-3">
                <div className="flex items-center justify-between shrink-0">
                  <span className={`text-xs ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>Vault tree</span>
                  <button className={`text-xs transition ${isDark ? 'text-zinc-400 hover:text-zinc-200' : 'text-slate-500 hover:text-slate-800'}`} onClick={() => { setTreeData(null); handleList(); }} type="button">Reload</button>
                </div>
                <div className={`flex-1 overflow-y-auto rounded-lg border p-4 text-sm leading-6 whitespace-pre font-mono ${isDark ? 'border-zinc-800 bg-zinc-900/60 text-zinc-200' : 'border-stone-200 bg-stone-50 text-slate-800'}`}>{renderTree(treeData)}</div>
              </div>
            ) : null
          )}

          {/* Graph tab — always mounted so InteractiveGraph keeps its simulation state across tab switches */}
          <div style={{ display: tab === 'graph' ? '' : 'none', height: 'calc(100% - 20px)' }} className="space-y-3">
            {graphLoading ? (
              <div className="flex items-center justify-center py-16"><Loader size={32} className="animate-spin text-emerald-500" /><span className="ml-3 text-sm text-zinc-500">Building graph...</span></div>
            ) : graphData ? (
              <div className="flex flex-col h-full">
                <div className="flex items-center justify-between mb-2 shrink-0">
                  <span className={`text-xs ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>{graphData.nodes?.length || 0} notes · {graphData.edges?.length || 0} links{graphData.elapsed_ms ? ` · ${graphData.elapsed_ms}ms` : ''}</span>
                  <button className={`text-xs transition ${isDark ? 'text-zinc-400 hover:text-zinc-200' : 'text-slate-500 hover:text-slate-800'}`} onClick={handleLoadGraph} type="button">Reload</button>
                </div>
                <div className="flex-1 rounded-lg border overflow-hidden relative" style={{ minHeight: '300px' }}>
                  <InteractiveGraph data={graphData} isDark={isDark} />
                </div>
              </div>
            ) : (
              <div className="space-y-3">
                <button className="w-full rounded-lg bg-emerald-600 px-4 py-2 text-sm font-medium text-white hover:bg-emerald-500 disabled:opacity-60" onClick={handleLoadGraph} type="button">Load Graph</button>
                <p className={`text-sm italic ${isDark ? 'text-zinc-500' : 'text-slate-400'}`}>Build a force-directed graph of all notes and their [[wikilink]] connections.</p>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

// Interactive graph with zoom, pan, and node-highlight
function InteractiveGraph({ data, isDark }: { data: any; isDark: boolean }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const simRef = useRef<any>(null);
  const resetFnRef = useRef<(() => void) | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);

  // Build graph on data load
  useEffect(() => {
    const container = containerRef.current;
    const canvas = canvasRef.current;
    if (!container || !canvas) return;

    const dpr = window.devicePixelRatio || 1;
    const rect = container.getBoundingClientRect();
    const w = rect.width || 600;
    const h = rect.height || 400;
    canvas.width = w * dpr;
    canvas.height = h * dpr;
    canvas.style.width = w + 'px';
    canvas.style.height = h + 'px';
    const ctx = canvas.getContext('2d')!;
    ctx.scale(dpr, dpr);

    // Build node list with connection counts for sizing
    const edgeCount = new Map<string, number>();
    for (const e of data.edges || []) {
      edgeCount.set(e.source, (edgeCount.get(e.source) || 0) + 1);
      edgeCount.set(e.target, (edgeCount.get(e.target) || 0) + 1);
    }
    const maxConn = Math.max(1, ...edgeCount.values());

    const cx = w / 2;
    const cy = h / 2;
    const nodes = (data.nodes || []).map((n: any) => ({
      id: n.id,
      name: n.name,
      x: cx + (Math.random() - 0.5) * 4,
      y: cy + (Math.random() - 0.5) * 4,
      vx: 0, vy: 0,
      radius: 3 + ((edgeCount.get(n.id) || 0) / maxConn) * 9,
      connections: edgeCount.get(n.id) || 0,
    }));

    const nodeMap = new Map(nodes.map((n: any) => [n.id, n]));
    const edges = (data.edges || []).filter((e: any) => nodeMap.has(e.source) && nodeMap.has(e.target));
    // Update the displayed count to reflect actually rendered edges
    if (data.edges) data.edges = edges;

    // Simulation state — smooth, Obsidian-style expansion from center
    let running = true;
    let frame = 0;
    const maxFrames = 300;

    function simulate() {
      if (!running || frame > maxFrames) return;
      frame++;

      const ease = Math.min(1, frame / 80);

      for (let i = 0; i < nodes.length; i++) {
        for (let j = i + 1; j < nodes.length; j++) {
          const a = nodes[i], b = nodes[j];
          let dx = b.x - a.x, dy = b.y - a.y;
          let dist = Math.sqrt(dx * dx + dy * dy) || 1;
          if (dist < 60) {
            const force = 30 * ease / (dist + 5);
            const fx = (dx / dist) * force;
            const fy = (dy / dist) * force;
            a.vx -= fx; a.vy -= fy;
            b.vx += fx; b.vy += fy;
          }
        }
      }

      for (const edge of edges) {
        const a = nodeMap.get(edge.source);
        const b = nodeMap.get(edge.target);
        if (!a || !b) continue;
        let dx = b.x - a.x, dy = b.y - a.y;
        let dist = Math.sqrt(dx * dx + dy * dy) || 1;
        const idealDist = 20 + (a.radius + b.radius);
        const force = (dist - idealDist) * 0.02 * ease;
        const fx = (dx / dist) * force;
        const fy = (dy / dist) * force;
        a.vx += fx; a.vy += fy;
        b.vx -= fx; b.vy -= fy;
      }

      // Settle: weak damping early so nodes explode freely, ramps to strong damping in last 80 frames
      const settling = frame > maxFrames - 80 ? (frame - (maxFrames - 80)) / 80 : 0;

      for (const n of nodes) {
        n.vx += (cx - n.x) * 0.001 * ease;
        n.vy += (cy - n.y) * 0.001 * ease;
        n.vx *= 0.98 - 0.12 * settling;
        n.vy *= 0.98 - 0.12 * settling;
        n.x += n.vx;
        n.y += n.vy;
      }

      // Render
      ctx.fillStyle = isDark ? '#18181b' : '#fafaf9';
      ctx.fillRect(0, 0, w, h);

      ctx.strokeStyle = isDark ? 'rgba(113,113,122,0.25)' : 'rgba(120,113,108,0.5)';
      ctx.lineWidth = 0.6;
      for (const edge of edges) {
        const a = nodeMap.get(edge.source);
        const b = nodeMap.get(edge.target);
        if (!a || !b) continue;
        ctx.beginPath();
        ctx.moveTo(a.x, a.y);
        ctx.lineTo(b.x, b.y);
        ctx.stroke();
      }

      for (const n of nodes) {
        ctx.beginPath();
        ctx.arc(n.x, n.y, n.radius, 0, Math.PI * 2);
        ctx.fillStyle = isDark ? '#10b981' : '#059669';
        ctx.fill();
      }
      requestAnimationFrame(simulate);
    }

    simRef.current = { nodes, nodeMap, edges, w, h };
    simulate();

    return () => { running = false; };
  }, [data, isDark]);

  // Render on zoom/pan/select via transform
  useEffect(() => {
    const canvas = canvasRef.current;
    const container = containerRef.current;
    if (!canvas || !container) return;

    const ctx = canvas.getContext('2d')!;

    // Zoom and pan state
    let scale = 1;
    let offsetX = 0, offsetY = 0;
    let isPanning = false;
    let panStartX = 0, panStartY = 0;
    let mouseScreenX = -9999, mouseScreenY = -9999;
    const minScale = 0.02, maxScale = 20;

    function render() {
      const dpr = window.devicePixelRatio || 1;
      const w = canvas.width / dpr;
      const h = canvas.height / dpr;
      const sim = simRef.current;
      if (!sim) return;

      ctx.fillStyle = isDark ? '#18181b' : '#fafaf9';
      ctx.fillRect(0, 0, w, h);

      // Draw screen-space background dots — offset smoothly with pan, fixed spacing regardless of zoom
      const dotSpacing = 20;
      const screenDotRadius = 200;
      const dotMaxAlpha = isDark ? 0.06 : 0.18;
      const startOffX = ((-offsetX % dotSpacing) + dotSpacing) % dotSpacing;
      const startOffY = ((-offsetY % dotSpacing) + dotSpacing) % dotSpacing;

      for (let sx = startOffX - dotSpacing; sx <= w + dotSpacing; sx += dotSpacing) {
        for (let sy = startOffY - dotSpacing; sy <= h + dotSpacing; sy += dotSpacing) {
          const sdist = Math.sqrt((sx - mouseScreenX) ** 2 + (sy - mouseScreenY) ** 2);
          if (sdist > screenDotRadius) continue;
          const alpha = dotMaxAlpha * (1 - sdist / screenDotRadius);
          ctx.fillStyle = isDark ? `rgba(255,255,255,${alpha})` : `rgba(0,0,0,${alpha})`;
          ctx.beginPath();
          ctx.arc(sx, sy, 1.5, 0, Math.PI * 2);
          ctx.fill();
        }
      }

      ctx.save();
      ctx.translate(offsetX, offsetY);
      ctx.scale(scale, scale);

      // Edges
      ctx.strokeStyle = isDark ? 'rgba(113,113,122,0.25)' : 'rgba(120,113,108,0.5)';
      ctx.lineWidth = 0.6 / scale;
      for (const edge of sim.edges) {
        const a = sim.nodeMap.get(edge.source);
        const b = sim.nodeMap.get(edge.target);
        if (!a || !b) continue;
        ctx.beginPath();
        ctx.moveTo(a.x, a.y);
        ctx.lineTo(b.x, b.y);
        ctx.stroke();
      }

      // Nodes
      for (const n of sim.nodes) {
        const highlighted = selectedId && (selectedId === n.id || sim.edges.some((e: any) =>
          (e.source === selectedId && e.target === n.id) || (e.target === selectedId && e.source === n.id)
        ));
        ctx.beginPath();
        ctx.arc(n.x, n.y, n.radius, 0, Math.PI * 2);
        ctx.fillStyle = highlighted ? '#f59e0b' : (isDark ? '#10b981' : '#059669');
        ctx.fill();
        if (highlighted) {
          ctx.strokeStyle = '#f59e0b';
          ctx.lineWidth = 2 / scale;
          ctx.stroke();
        }
      }

      // Labels — show for all visible nodes only when zoomed in very close
      if (scale > 1.2) {
        ctx.fillStyle = isDark ? '#a1a1aa' : '#57534e';
        const fontSize = Math.max(8, 13 / scale);
        ctx.font = `${fontSize}px sans-serif`;
        for (const n of sim.nodes) {
          // Screen-space visibility check
          const sx = n.x * scale + offsetX;
          const sy = n.y * scale + offsetY;
          if (sx > -80 && sx < w + 80 && sy > -80 && sy < h + 80) {
            // Draw in world space (transform is already applied)
            ctx.fillText(n.name, n.x + n.radius + 3 / scale, n.y + fontSize * 0.4);
          }
        }
      }

      ctx.restore();
    }

    function worldToScreen(wx: number, wy: number) {
      return { x: wx * scale + offsetX, y: wy * scale + offsetY };
    }

    function screenToWorld(sx: number, sy: number) {
      return { x: (sx - offsetX) / scale, y: (sy - offsetY) / scale };
    }

    function getNodeAt(sx: number, sy: number) {
      const world = screenToWorld(sx, sy);
      const sim = simRef.current;
      if (!sim) return null;
      for (const n of sim.nodes) {
        const dx = world.x - n.x, dy = world.y - n.y;
        if (dx * dx + dy * dy < (n.radius + 5) * (n.radius + 5)) return n;
      }
      return null;
    }

    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const delta = e.deltaY > 0 ? 0.85 : 1.15;
      const newScale = Math.min(maxScale, Math.max(minScale, scale * delta));
      const mx = e.offsetX, my = e.offsetY;
      offsetX = mx - (mx - offsetX) * (newScale / scale);
      offsetY = my - (my - offsetY) * (newScale / scale);
      scale = newScale;
      render();
    };

    const onMouseDown = (e: MouseEvent) => {
      const node = getNodeAt(e.offsetX, e.offsetY);
      if (node) {
        // Toggle selection — yellow highlight on connected nodes
        setSelectedId(selectedId === node.id ? null : node.id);
        return;
      }
      setSelectedId(null);
      isPanning = true;
      panStartX = e.clientX - offsetX;
      panStartY = e.clientY - offsetY;
      canvas.style.cursor = 'grabbing';
    };

    const onMouseMove = (e: MouseEvent) => {
      // Update screen mouse position for dot proximity effect
      mouseScreenX = e.offsetX;
      mouseScreenY = e.offsetY;

      if (isPanning) {
        offsetX = e.clientX - panStartX;
        offsetY = e.clientY - panStartY;
        render();
        return;
      }
      // Re-render on move so the grid follows the cursor (throttled via flag)
      render();

      const node = getNodeAt(e.offsetX, e.offsetY);
      canvas.style.cursor = node ? 'pointer' : 'grab';
    };

    const onMouseUp = () => {
      isPanning = false;
      canvas.style.cursor = 'grab';
    };

    canvas.addEventListener('wheel', onWheel, { passive: false });
    canvas.addEventListener('mousedown', onMouseDown);
    canvas.addEventListener('mousemove', onMouseMove);
    canvas.addEventListener('mouseup', onMouseUp);
    canvas.addEventListener('mouseleave', onMouseUp);
    canvas.style.cursor = 'grab';

    // Expose reset function for the "Return to Graph" button
    resetFnRef.current = () => {
      const w = canvas.width, h = canvas.height;
      const sim = simRef.current;
      if (!sim) return;
      const startOX = offsetX, startOY = offsetY;
      const startScale = scale;
      let startTime: number | null = null;
      function animateReset(t: number) {
        if (!startTime) startTime = t;
        const elapsed = t - startTime;
        const progress = Math.min(1, elapsed / 350);
        const ease = 1 - Math.pow(1 - progress, 3);
        offsetX = startOX + (0 - startOX) * ease;
        offsetY = startOY + (0 - startOY) * ease;
        scale = startScale + (1 - startScale) * ease;
        render();
        if (progress < 1) requestAnimationFrame(animateReset);
      }
      requestAnimationFrame(animateReset);
    };

    render();

    return () => {
      canvas.removeEventListener('wheel', onWheel);
      canvas.removeEventListener('mousedown', onMouseDown);
      canvas.removeEventListener('mousemove', onMouseMove);
      canvas.removeEventListener('mouseup', onMouseUp);
      canvas.removeEventListener('mouseleave', onMouseUp);
    };
  }, [data, isDark, selectedId]);

  useEffect(() => {
    if (containerRef.current && resetFnRef.current) {
      (containerRef.current as any).__resetView = resetFnRef.current;
    }
    return () => {
      if (containerRef.current) delete (containerRef.current as any).__resetView;
    };
  });

  return (
    <div ref={containerRef} className="w-full h-full relative">
      <canvas ref={canvasRef} className="w-full h-full" />
      <button
        className={`absolute top-2 left-2 text-[11px] font-medium px-2.5 py-1 rounded-md border transition opacity-60 hover:opacity-100 ${isDark ? 'border-zinc-700 bg-zinc-900/80 text-zinc-300 hover:bg-zinc-800' : 'border-stone-300 bg-white/80 text-slate-600 hover:bg-white'}`}
        onClick={() => (containerRef.current as any)?.__resetView?.()}
        type="button"
      >
        Return to Graph
      </button>
    </div>
  );
}
