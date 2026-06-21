// Main AEGIS chat application component
// Orchestrates state, API calls, and composes all UI components

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { FormEvent } from 'react';
import {
  PanelLeftClose, PanelLeftOpen, Sun, Moon, Settings, X,
} from 'lucide-react';

// Types
import type {
  ThemeMode, AppearanceTheme, SettingsTab, ResponseStyle,
  Message, RetrievalChunk, ChatMode, EngineSessionSummary,
  EngineSession, EngineSessionsResponse, EngineTurn,
  ModelResponse, ProviderResponse, ModelListResponse,
  ProviderListResponse, ProfileResponse, ModelDownloadState,
  IngestResponse, DeleteIndexedDocumentResponse,
  OutlookCalendar, OutlookCalendarsResponse,
  OutlookCalendarSelectionResponse, CalendarResult, CalendarCreateResponse,
  CodeProject, SystemStats, ContextUsage, IndexedDocument,
  FileSystemDirectoryHandle, InferenceStats, ImportPhase,
} from './types';

// Constants
import {
  API_BASE, THEME_STORAGE_KEY, APPEARANCE_THEME_STORAGE_KEY,
  INDEXED_DOCUMENTS_STORAGE_KEY, PINNED_SESSIONS_STORAGE_KEY,
  RESPONSE_STYLE_STORAGE_KEY, VOICE_LOW_RAM_MODE_STORAGE_KEY,
  VOICE_TTS_ENABLED_STORAGE_KEY, RAG_ENABLED_STORAGE_KEY,
  RAG_TOP_K_STORAGE_KEY, RAG_THRESHOLD_STORAGE_KEY, LANGUAGE_STORAGE_KEY,
  OBSIDIAN_VAULT_PATH_KEY, OBSIDIAN_ENABLED_KEY,
  OLLAMA_MODEL_CATALOG, EMPTY_CONTEXT_USAGE,
} from './constants';

// Lib utilities
import {
  loadStoredTheme, loadStoredAppearanceTheme, loadStoredResponseStyle,
  loadStoredVoiceLowRamMode, loadStoredTtsEnabled, loadStoredRagEnabled,
  loadStoredRagTopK, loadStoredRagThreshold,
  loadStoredIndexedDocumentsBySession, loadStoredPinnedSessionIds,
  fetchContextUsage, formatTokenMeter,
  turnsToMessages, sessionUpdatedAtMs, mergeIndexedDocuments,
  scanProjectDirectory, buildProjectSnapshot, findProjectFile,
  extractUnifiedDiff, parsePatchTarget, applySimpleUnifiedDiff,
  extractSseEvents, sseEventData, splitAssistantStreamSegments,
  sanitizeTextForTts, copyTextToClipboard,
  fitTextareaToContent, isFatalUiError,
  downloadConversationPdf, formatSessionLastAccessed,
  cleanOutlookCalendarName, isVisibleOutlookCalendar, outlookCalendarLabel,
  parseWelcomeMessages, randomWelcomeMessage, personalizeWelcomeMessage,
} from './lib';

// Components
import { Sidebar } from './components/Sidebar';
import { Header } from './components/Header';
import { MessageBubble } from './components/MessageBubble';
import { Composer } from './components/Composer';
import { SettingsPanel } from './components/SettingsPanel';
import { CalendarModal } from './components/CalendarModal';
import { MetricsSidebar } from './components/MetricsSidebar';
import { VoiceModeOverlay } from './components/VoiceModeOverlay';
import { ProjectPermissionModal } from './components/ProjectPermissionModal';
import { DeleteConfirmModal } from './components/DeleteConfirmModal';
import { MemoriesPopup } from './components/MemoriesPopup';
import { ObsidianModal } from './components/ObsidianModal';
import { I18nProvider, type Language } from './lib/i18n';
import translations from './lib/translations';
import { useAudioRecorder } from './hooks/useAudioRecorder';
import { modelSearchPlaceholder, installedModelsLabel, modelReadyMessage, modelDownloadPercent, type PullModelChunk } from './lib/modelDownload';

declare global {
  interface Window {
    showDirectoryPicker?: () => Promise<FileSystemDirectoryHandle>;
  }
}

export default function App() {
  const [sessions, setSessions] = useState<EngineSessionSummary[]>([]);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState('');
  const [lang, setLang] = useState<Language>(() => {
    if (typeof window === 'undefined') return 'en';
    const stored = window.localStorage.getItem(LANGUAGE_STORAGE_KEY);
    return stored === 'tr' ? 'tr' : 'en';
  });
  const [theme, setTheme] = useState<ThemeMode>(loadStoredTheme);
  const [systemPrefersDark, setSystemPrefersDark] = useState(() => {
    if (typeof window === 'undefined') return false;
    return window.matchMedia('(prefers-color-scheme: dark)').matches;
  });
  const [appearanceTheme, setAppearanceTheme] = useState<AppearanceTheme>(loadStoredAppearanceTheme as AppearanceTheme);
  const [isStreaming, setIsStreaming] = useState(false);
  const [isUploading, setIsUploading] = useState(false);
  const [isClearingIndexedDocuments, setIsClearingIndexedDocuments] = useState(false);
  const [documentContextNotice, setDocumentContextNotice] = useState<string | null>(null);
  const [importProgress, setImportProgress] = useState(0);
  const [importPhase, setImportPhase] = useState<ImportPhase>('idle');
  const [importFileLabel, setImportFileLabel] = useState('');
  const [indexedDocumentsBySession, setIndexedDocumentsBySession] = useState<Record<string, IndexedDocument[]>>(loadStoredIndexedDocumentsBySession);
  const [pinnedSessionIds, setPinnedSessionIds] = useState<string[]>(loadStoredPinnedSessionIds);

  // Voice state
  const [isVoiceMode, setIsVoiceMode] = useState(false);
  const [isSpeaking, setIsSpeaking] = useState(false);
  const [isTranscribing, setIsTranscribing] = useState(false);
  const [isTtsEnabled, setIsTtsEnabled] = useState<boolean>(loadStoredTtsEnabled);
  const [isVoiceLowRamMode, setIsVoiceLowRamMode] = useState<boolean>(loadStoredVoiceLowRamMode);
  const [isRagEnabled, setIsRagEnabled] = useState<boolean>(loadStoredRagEnabled);
  const [ragTopK, setRagTopK] = useState<number>(loadStoredRagTopK);
  const [ragSimilarityThreshold, setRagSimilarityThreshold] = useState<number>(loadStoredRagThreshold);
  const [selectedMessageSources, setSelectedMessageSources] = useState<RetrievalChunk[] | null>(null);
  const [selectedMessageSourcesIndex, setSelectedMessageSourcesIndex] = useState<number | null>(null);
  const [metricsTab, setMetricsTab] = useState<'metrics' | 'sources'>('metrics');
  const [speakingMessageIndex, setSpeakingMessageIndex] = useState<number | null>(null);
  const activeAudioRef = useRef<HTMLAudioElement | null>(null);
  const { isRecording, analyser, startRecording, stopRecording } = useAudioRecorder();

  const [projectsOpen, setProjectsOpen] = useState(true);
  const [sessionsOpen, setSessionsOpen] = useState(true);
  const [codeProjects, setCodeProjects] = useState<CodeProject[]>([]);
  const [activeProjectId, setActiveProjectId] = useState<string | null>(null);
  const [scanningProject, setScanningProject] = useState(false);
  const [projectPermissionRequestId, setProjectPermissionRequestId] = useState<string | null>(null);
  const [projectEditMessage, setProjectEditMessage] = useState<string | null>(null);
  const [status, setStatus] = useState('Ready');
  const [error, setError] = useState<string | null>(null);
  const [dismissedResourceWarning, setDismissedResourceWarning] = useState<string | null>(null);
  const [toolsOpen, setToolsOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsClosing, setSettingsClosing] = useState(false);
  const [settingsTab, setSettingsTab] = useState<SettingsTab>('general');
  const [settingsMessage, setSettingsMessage] = useState<string | null>(null);
  const [settingsLoading, setSettingsLoading] = useState(false);
  const [availableModels, setAvailableModels] = useState<ModelResponse[]>([]);
  const [availableProviders, setAvailableProviders] = useState<ProviderResponse[]>([]);
  const [modelSearch, setModelSearch] = useState('');
  const [selectedModelProviderTag, setSelectedModelProviderTag] = useState('All');
  const [downloadingModel, setDownloadingModel] = useState<string | null>(null);
  const [pausedModelDownload, setPausedModelDownload] = useState<string | null>(null);
  const [modelDownloadState, setModelDownloadState] = useState<ModelDownloadState>('idle');
  const [modelDownloadProgress, setModelDownloadProgress] = useState(0);
  const [modelDownloadStatus, setModelDownloadStatus] = useState('');
  const [responseStyle, setResponseStyle] = useState<ResponseStyle>(loadStoredResponseStyle);
  const [profileText, setProfileText] = useState('');
  const [profilePath, setProfilePath] = useState('');
  const [memoryInput, setMemoryInput] = useState('');
  const [memoriesPopupOpen, setMemoriesPopupOpen] = useState(false);
  const [welcomeMessages, setWelcomeMessages] = useState(parseWelcomeMessages(''));
  const [activeWelcomeMessage, setActiveWelcomeMessage] = useState(() => randomWelcomeMessage(parseWelcomeMessages('')));
  const [obsidianVaultPath, setObsidianVaultPath] = useState(() => {
    if (typeof window === 'undefined') return '';
    return window.localStorage.getItem(OBSIDIAN_VAULT_PATH_KEY) || '';
  });
  const [obsidianEnabled, setObsidianEnabled] = useState(() => {
    if (typeof window === 'undefined') return false;
    return window.localStorage.getItem(OBSIDIAN_ENABLED_KEY) === 'true';
  });
  const [sessionPendingDeletion, setSessionPendingDeletion] = useState<EngineSessionSummary | null>(null);
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [calendarOpen, setCalendarOpen] = useState(false);
  const [obsidianOpen, setObsidianOpen] = useState(false);
  const [calendarPrompt, setCalendarPrompt] = useState('');
  const [creatingCalendarEvent, setCreatingCalendarEvent] = useState(false);
  const [calendarResult, setCalendarResult] = useState<CalendarResult | null>(null);
  const [calendarMessage, setCalendarMessage] = useState<string | null>(null);
  const [outlookCalendars, setOutlookCalendars] = useState<OutlookCalendar[]>([]);
  const [selectedOutlookCalendarId, setSelectedOutlookCalendarId] = useState('');
  const [loadingOutlookCalendars, setLoadingOutlookCalendars] = useState(false);
  const [systemStats, setSystemStats] = useState<SystemStats>({ cpu: 0, ram: 0 });
  const [contextUsage, setContextUsage] = useState<ContextUsage>(EMPTY_CONTEXT_USAGE);
  const [streamingSessionId, setStreamingSessionId] = useState<string | null>(null);
  const [streamingMessagesBySession, setStreamingMessagesBySession] = useState<Record<string, Message[]>>({});
  const [editingMessageIndex, setEditingMessageIndex] = useState<number | null>(null);
  const [editingMessageText, setEditingMessageText] = useState('');
  const [copiedMessageIndex, setCopiedMessageIndex] = useState<number | null>(null);
  const [editingSessionId, setEditingSessionId] = useState<string | null>(null);
  const [editingTitle, setEditingTitle] = useState('');
  const [deletingSessionIds, setDeletingSessionIds] = useState<string[]>([]);
  const [isMetricsOpen, setIsMetricsOpen] = useState(false);
  const [newSessionPulseId, setNewSessionPulseId] = useState<string | null>(null);
  const [inferenceStats, setInferenceStats] = useState<InferenceStats>({
    latency: 0, tps: 0, ttft: 0, ragTime: 0, similarity: 0, chunks: 0, backend: '---',
  });
  const inferenceStartTime = useRef<number | null>(null);
  const [sessionMenuOpenId, setSessionMenuOpenId] = useState<string | null>(null);
  const [chatMode, setChatMode] = useState<ChatMode>('general');
  const scrollRef = useRef<HTMLDivElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const composerTextareaRef = useRef<HTMLTextAreaElement>(null);
  const modelDownloadAbortRef = useRef<AbortController | null>(null);
  const modelDownloadAbortReasonRef = useRef<'pause' | 'cancel' | null>(null);
  const settingsCloseTimeoutRef = useRef<number | null>(null);
  const activeSessionIdRef = useRef<string | null>(activeSessionId);
  const streamingMessagesBySessionRef = useRef<Record<string, Message[]>>({});
  const isDark = theme === 'system' ? systemPrefersDark : theme === 'dark';
  const activeSession = useMemo(() => sessions.find((s) => s.session_id === activeSessionId), [activeSessionId, sessions]);
  const activeProject = useMemo(() => codeProjects.find((p) => p.id === activeProjectId) ?? null, [activeProjectId, codeProjects]);
  const pinnedSessionIdSet = useMemo(() => new Set(pinnedSessionIds), [pinnedSessionIds]);
  const resourceWarning = systemStats.cpu > 80 || systemStats.ram > 80
    ? `${[systemStats.cpu > 80 ? 'CPU' : null, systemStats.ram > 80 ? 'RAM' : null].filter(Boolean).join(' and ')} ${systemStats.cpu > 80 && systemStats.ram > 80 ? 'are' : 'is'} almost at full capacity.`
    : null;
  const visibleResourceWarning = resourceWarning && resourceWarning !== dismissedResourceWarning ? resourceWarning : null;
  const errorDismissible = error ? !isFatalUiError(error) : false;
  const tokenMeterLabel = formatTokenMeter(contextUsage);
  const showCenteredComposer = !activeSessionId && messages.length === 0;
  const filteredCatalogModels = OLLAMA_MODEL_CATALOG.filter((model) => {
    const search = modelSearch.trim().toLowerCase();
    return (!search || model.name.toLowerCase().includes(search) || model.provider.toLowerCase().includes(search) || model.tags.some((t) => t.toLowerCase().includes(search)))
      && (selectedModelProviderTag === 'All' || model.provider === selectedModelProviderTag || model.tags.includes(selectedModelProviderTag));
  });
  const activeProvider = availableProviders.find((p) => p.active);
  const indexedDocuments = activeSessionId ? indexedDocumentsBySession[activeSessionId] ?? [] : [];
  const showImportProgress = importPhase !== 'idle';
  const indexedDocumentLabel = indexedDocuments.length === 1 ? indexedDocuments[0].file_name : `${indexedDocuments.length} documents`;
  const indexedChunkCount = indexedDocuments.reduce((t, d) => t + d.chunks_added, 0);

  const sortedSessions = useMemo(() => {
    const originalOrder = new Map(sessions.map((s, i) => [s.session_id, i]));
    return [...sessions].sort((a, b) => {
      const p = Number(pinnedSessionIdSet.has(b.session_id)) - Number(pinnedSessionIdSet.has(a.session_id));
      if (p !== 0) return p;
      const r = sessionUpdatedAtMs(b) - sessionUpdatedAtMs(a);
      if (r !== 0) return r;
      return (originalOrder.get(a.session_id) ?? 0) - (originalOrder.get(b.session_id) ?? 0);
    });
  }, [pinnedSessionIdSet, sessions]);

  // --- API Functions ---

  const loadSessions = useCallback(async () => {
    setError(null);
    const res = await fetch(`${API_BASE}/sessions`);
    if (!res.ok) throw new Error(engineErr('engine.sessions_load', res.status));
    setSessions(((await res.json()) as EngineSessionsResponse).sessions);
  }, []);

  const createSession = useCallback(async () => {
    setError(null);
    const res = await fetch(`${API_BASE}/sessions`, { method: 'POST' });
    if (!res.ok) throw new Error(engineErr('engine.session_create', res.status));
    const session = (await res.json()) as EngineSession;
    activeSessionIdRef.current = session.session_id;
    setActiveSessionId(session.session_id);
    return session;
  }, []);

  const loadSession = useCallback(async (sessionId: string) => {
    setError(null);
    setStatus(t('status.loading_session'));
    const res = await fetch(`${API_BASE}/sessions/${encodeURIComponent(sessionId)}`);
    if (!res.ok) throw new Error(engineErr('engine.session_load', res.status));
    const session = (await res.json()) as EngineSession;
    activeSessionIdRef.current = session.session_id;
    setActiveSessionId(session.session_id);
    setMessages(turnsToMessages(session.history.turns, session.session_id));
    setStatus(t('status.ready'));
  }, []);

  const loadSettingsData = useCallback(async () => {
    setSettingsLoading(true);
    setSettingsMessage(null);
    try {
      const [modelsRes, providersRes, profileRes] = await Promise.allSettled([
        fetch(`${API_BASE}/models`), fetch(`${API_BASE}/providers`), fetch(`${API_BASE}/profile`),
      ]);
      if (modelsRes.status === 'fulfilled' && modelsRes.value.ok) setAvailableModels(((await modelsRes.value.json()) as ModelListResponse).models);
      if (providersRes.status === 'fulfilled' && providersRes.value.ok) setAvailableProviders(((await providersRes.value.json()) as ProviderListResponse).providers);
      if (profileRes.status === 'fulfilled' && profileRes.value.ok) {
        const data = (await profileRes.value.json()) as ProfileResponse;
        setProfileText(data.contents);
        setProfilePath(data.path);
      }
    } catch (e) {
      setSettingsMessage(e instanceof Error ? e.message : t('error.could_not_load_settings'));
    } finally {
      setSettingsLoading(false);
    }
  }, []);

  // --- Effects ---

  useEffect(() => {
    const interval = setInterval(() => {
      fetch(`${API_BASE}/system/stats`).then((r) => r.json()).then((d: { cpu: number; ram: number }) => setSystemStats(d)).catch(() => {});
    }, 3000);
    return () => clearInterval(interval);
  }, []);

  useEffect(() => { loadSessions().catch((e) => { setError(e instanceof Error ? e.message : t('error.could_not_load_sessions')); setStatus(t('error.engine_unavailable')); }); }, [loadSessions]);

  useEffect(() => {
    fetch(`${API_BASE}/profile`).then((r) => (r.ok ? r.json() : null)).then((d: ProfileResponse | null) => { if (d) { setProfileText(d.contents); setProfilePath(d.path); } }).catch(() => {});
  }, []);

  useEffect(() => { if (settingsOpen) void loadSettingsData(); }, [loadSettingsData, settingsOpen]);
  useEffect(() => { activeSessionIdRef.current = activeSessionId; }, [activeSessionId]);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const res = await fetch(`${API_BASE}/system/stats`);
        if (!res.ok) return;
        const data = (await res.json()) as Partial<SystemStats>;
        if (!cancelled) setSystemStats({ cpu: Math.max(0, Math.min(100, Math.round(Number(data.cpu ?? 0)))), ram: Math.max(0, Math.min(100, Math.round(Number(data.ram ?? 0)))) });
      } catch {}
    }
    void load();
    const id = window.setInterval(load, 2000);
    return () => { cancelled = true; window.clearInterval(id); };
  }, []);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const usage = await fetchContextUsage(activeSessionId);
        if (!cancelled) setContextUsage(usage);
      } catch { if (!cancelled && contextUsage.context_window <= 0) setContextUsage({ ...EMPTY_CONTEXT_USAGE, usage_source: 'unavailable' }); }
    }
    void load();
    const id = window.setInterval(load, 4000);
    return () => { cancelled = true; window.clearInterval(id); };
  }, [activeSessionId]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (typeof window === 'undefined') return;
    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    const handler = (e: MediaQueryListEvent) => setSystemPrefersDark(e.matches);
    mediaQuery.addEventListener('change', handler);
    return () => mediaQuery.removeEventListener('change', handler);
  }, []);

  useEffect(() => { if (typeof window !== 'undefined') window.localStorage.setItem(THEME_STORAGE_KEY, theme); }, [theme]);
  useEffect(() => { if (typeof window !== 'undefined') window.localStorage.setItem(APPEARANCE_THEME_STORAGE_KEY, appearanceTheme); }, [appearanceTheme]);
  useEffect(() => { if (typeof window !== 'undefined') window.localStorage.setItem(RESPONSE_STYLE_STORAGE_KEY, responseStyle); }, [responseStyle]);
  useEffect(() => { if (typeof window !== 'undefined') window.localStorage.setItem(LANGUAGE_STORAGE_KEY, lang); }, [lang]);
  useEffect(() => { if (typeof window !== 'undefined') window.localStorage.setItem(OBSIDIAN_VAULT_PATH_KEY, obsidianVaultPath); }, [obsidianVaultPath]);
  useEffect(() => { if (typeof window !== 'undefined') window.localStorage.setItem(OBSIDIAN_ENABLED_KEY, String(obsidianEnabled)); }, [obsidianEnabled]);
  const t = useCallback((key: string) => translations[lang]?.[key] ?? translations.en[key] ?? key, [lang]);
  const engineErr = useCallback((key: string, status: number, file?: string) => {
    let msg = t(key);
    msg = msg.replace('{status}', status.toString());
    if (file !== undefined) msg = msg.replace('{file}', file);
    return msg;
  }, [t]);
  useEffect(() => {
    if (typeof window === 'undefined') return;
    window.localStorage.setItem(PINNED_SESSIONS_STORAGE_KEY, JSON.stringify(pinnedSessionIds));
  }, [pinnedSessionIds]);

  useEffect(() => {
    let cancelled = false;
    async function loadWelcome() {
      try {
        const res = await fetch('/welcome-messages.md', { cache: 'no-cache' });
        if (!res.ok) return;
        const msgs = parseWelcomeMessages(await res.text());
        if (!cancelled) {
          setWelcomeMessages(msgs);
          setActiveWelcomeMessage((current) => parseWelcomeMessages('').includes(current) ? randomWelcomeMessage(msgs) : current);
        }
      } catch {}
    }
    void loadWelcome();
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    if (sessions.length === 0) return;
    const availableIds = new Set(sessions.map((s) => s.session_id));
    setPinnedSessionIds((cur) => { const next = cur.filter((id) => availableIds.has(id)); return next.length === cur.length ? cur : next; });
  }, [sessions]);

  useEffect(() => {
    if (typeof window === 'undefined') return;
    window.localStorage.removeItem('aegis-indexed-documents');
    window.localStorage.setItem(INDEXED_DOCUMENTS_STORAGE_KEY, JSON.stringify(indexedDocumentsBySession));
  }, [indexedDocumentsBySession]);

  useEffect(() => { scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight, behavior: 'smooth' }); }, [messages, isStreaming]);
  useEffect(() => { if (composerTextareaRef.current) fitTextareaToContent(composerTextareaRef.current); }, [input]);

  useEffect(() => {
    return () => { if (settingsCloseTimeoutRef.current !== null) window.clearTimeout(settingsCloseTimeoutRef.current); };
  }, []);

  useEffect(() => {
    if (!documentContextNotice) return;
    const t = window.setTimeout(() => setDocumentContextNotice(null), 7000);
    return () => window.clearTimeout(t);
  }, [documentContextNotice]);

  // --- Voice ---

  const speakAssistantResponse = useCallback(async (text: string, force = false, messageIndex?: number) => {
    if (!isTtsEnabled && !force) return;
    if (activeAudioRef.current) { activeAudioRef.current.pause(); activeAudioRef.current = null; setIsSpeaking(false); setSpeakingMessageIndex(null); }
    if (messageIndex !== undefined && speakingMessageIndex === messageIndex) return;
    const cleanText = sanitizeTextForTts(text);
    if (!cleanText) return;
    try {
      if (messageIndex !== undefined) setSpeakingMessageIndex(messageIndex);
      setIsSpeaking(true);
      const res = await fetch(`${API_BASE}/voice/synthesize?text=${encodeURIComponent(cleanText)}`);
      if (!res.ok) throw new Error('Synthesis failed');
      const blob = await res.blob();
      const url = URL.createObjectURL(blob);
      const audio = new Audio(url);
      activeAudioRef.current = audio;
      audio.onended = () => { setIsSpeaking(false); setSpeakingMessageIndex(null); URL.revokeObjectURL(url); activeAudioRef.current = null; };
      audio.onerror = () => { setIsSpeaking(false); setSpeakingMessageIndex(null); URL.revokeObjectURL(url); activeAudioRef.current = null; };
      audio.play();
    } catch (err) { console.error('TTS error:', err); setIsSpeaking(false); setSpeakingMessageIndex(null); }
  }, [isTtsEnabled, speakingMessageIndex]);

  const handleStopDictation = async () => {
    setIsTranscribing(true);
    try {
      const audioBlob = await stopRecording();
      const formData = new FormData();
      formData.append('file', audioBlob, 'voice.wav');
      const res = await fetch(`${API_BASE}/voice/transcribe`, { method: 'POST', body: formData });
      if (!res.ok) throw new Error('Transcription failed');
      const data = await res.json();
      if (data.text && data.text.trim()) {
        const prompt = data.text.trim();
        setInput('');
        const ts = new Date().toISOString();
        await streamPrompt(prompt, [...messages, { role: 'user', content: prompt, timestamp: ts }, { role: 'assistant', content: '' }]);
      }
    } catch (err) { console.error('Dictation error:', err); setError(t('error.could_not_transcribe')); }
    finally { setIsTranscribing(false); }
  };

  // --- Session Actions ---

  const handleSessionSelect = async (sessionId: string) => {
    if (deletingSessionIds.includes(sessionId)) return;
    setSessionMenuOpenId(null);
    setSelectedMessageSources(null);
    setSelectedMessageSourcesIndex(null);
    setMetricsTab('metrics');
    if (streamingSessionId === sessionId) {
      const msgs = streamingMessagesBySession[sessionId];
      if (msgs) { activeSessionIdRef.current = sessionId; setActiveSessionId(sessionId); setMessages(msgs); setStatus(t('status.inference')); return; }
    }
    try { await loadSession(sessionId); } catch (e) { setError(e instanceof Error ? e.message : t('error.could_not_load_session')); setStatus(t('error.session_load_failed')); }
  };

  const handleNewSession = () => {
    if (isStreaming) return;
    setSessionMenuOpenId(null);
    activeSessionIdRef.current = null;
    setActiveSessionId(null); setMessages([]); setSelectedMessageSources(null);
    setSelectedMessageSourcesIndex(null); setMetricsTab('metrics'); setInput('');
    setError(null); setEditingMessageIndex(null); setEditingMessageText('');
    setEditingSessionId(null); setEditingTitle(''); setImportPhase('idle');
    setImportProgress(0); setImportFileLabel('');
    setActiveWelcomeMessage(randomWelcomeMessage(welcomeMessages));
    setStatus(t('status.ready'));
  };

  const toggleVoiceLowRamMode = useCallback(async (enabled: boolean) => {
    setIsVoiceLowRamMode(enabled);
    if (typeof window !== 'undefined') window.localStorage.setItem(VOICE_LOW_RAM_MODE_STORAGE_KEY, JSON.stringify(enabled));
    try { await fetch(`${API_BASE}/voice/config`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ keep_cached: !enabled }) }); } catch {}
  }, []);

  const toggleRagEnabled = useCallback((enabled: boolean) => {
    setIsRagEnabled(enabled);
    if (typeof window !== 'undefined') window.localStorage.setItem(RAG_ENABLED_STORAGE_KEY, JSON.stringify(enabled));
  }, []);

  const changeRagTopK = useCallback((val: number) => {
    setRagTopK(val);
    if (typeof window !== 'undefined') window.localStorage.setItem(RAG_TOP_K_STORAGE_KEY, JSON.stringify(val));
  }, []);

  const changeRagThreshold = useCallback((val: number) => {
    setRagSimilarityThreshold(val);
    if (typeof window !== 'undefined') window.localStorage.setItem(RAG_THRESHOLD_STORAGE_KEY, JSON.stringify(val));
  }, []);

  const changeTtsEnabled = useCallback((enabled: boolean) => {
    setIsTtsEnabled(enabled);
    if (typeof window !== 'undefined') window.localStorage.setItem(VOICE_TTS_ENABLED_STORAGE_KEY, JSON.stringify(enabled));
  }, []);

  // --- Project Actions ---

  const handleAddProject = async () => {
    if (!window.showDirectoryPicker) { setError(t('error.no_folder_support')); return; }
    setScanningProject(true);
    setProjectEditMessage(null);
    setError(null);
    setStatus(t('status.scanning_project'));
    try {
      const rootHandle = await window.showDirectoryPicker();
      const files = await scanProjectDirectory(rootHandle);
      const project: CodeProject = {
        id: `${rootHandle.name}-${Date.now()}`, name: rootHandle.name, fileCount: files.length,
        totalBytes: files.reduce((t, f) => t + f.size, 0), snapshot: buildProjectSnapshot(rootHandle.name, files),
        files, writable: false, updatedAt: new Date().toISOString(), rootHandle,
      };
      setCodeProjects((cur) => [project, ...cur.filter((p) => p.name !== project.name)]);
      setActiveProjectId(project.id);
      setProjectsOpen(true);
      setChatMode('coder');
      setProjectPermissionRequestId(project.id);
      setStatus(`Project ${project.name} scanned`);
    } catch (e) {
      if (e instanceof DOMException && e.name === 'AbortError') setStatus(t('status.ready'));
      else { setError(e instanceof Error ? e.message : t('error.could_not_scan_project')); setStatus(t('error.project_scan_failed')); }
    } finally { setScanningProject(false); }
  };

  const requestProjectWritePermission = async (projectId: string) => {
    const project = codeProjects.find((p) => p.id === projectId);
    if (!project) { setProjectPermissionRequestId(null); return; }
    try {
      const permission = (await project.rootHandle.requestPermission?.({ mode: 'readwrite' })) ?? 'denied';
      setCodeProjects((cur) => cur.map((p) => p.id === projectId ? { ...p, writable: permission === 'granted' } : p));
      setProjectEditMessage(permission === 'granted' ? `AEGIS can apply approved patches inside ${project.name}.` : `${project.name} remains read-only until write access is granted.`);
    } catch (e) { setProjectEditMessage(e instanceof Error ? e.message : t('error.could_not_request_permission')); }
    finally { setProjectPermissionRequestId(null); }
  };

  const removeProject = (projectId: string) => {
    setCodeProjects((cur) => cur.filter((p) => p.id !== projectId));
    setActiveProjectId((cur) => cur === projectId ? null : cur);
    setProjectEditMessage(null);
  };

  const applyAssistantPatch = async (messageContent: string) => {
    if (!activeProject) { setError(t('error.no_active_project')); return; }
    if (!activeProject.writable) { setError(t('error.project_readonly')); return; }
    const diff = extractUnifiedDiff(messageContent);
    const changedFiles = diff.split('\n').filter((l) => l.startsWith('+++ ') && !l.includes('/dev/null'));
    const targetPath = parsePatchTarget(diff);
    const targetFile = targetPath ? findProjectFile(activeProject, targetPath) : null;
    if (changedFiles.length > 1) { setError(t('error.patch_single_file')); return; }
    if (!diff || !targetFile?.handle.createWritable) { setError(t('error.patch_no_diff')); return; }
    try {
      const nextContent = applySimpleUnifiedDiff(targetFile.content, diff);
      const writable = await targetFile.handle.createWritable();
      await writable.write(nextContent);
      await writable.close();
      const nextFiles = activeProject.files.map((f) => f.path === targetFile.path ? { ...f, content: nextContent, size: new Blob([nextContent]).size } : f);
      const nextProject = { ...activeProject, files: nextFiles, fileCount: nextFiles.length, totalBytes: nextFiles.reduce((t, f) => t + f.size, 0), snapshot: buildProjectSnapshot(activeProject.name, nextFiles), updatedAt: new Date().toISOString() };
      setCodeProjects((cur) => cur.map((p) => p.id === nextProject.id ? nextProject : p));
      setProjectEditMessage(`Applied patch to ${targetFile.path}.`);
      setStatus(t('status.project_patch_applied'));
    } catch (e) { setError(e instanceof Error ? e.message : t('error.could_not_apply_patch')); setStatus(t('error.patch_failed')); }
  };

  // --- Delete Session ---

  const handleDeleteSession = async (session: EngineSessionSummary) => {
    if (isStreaming || deletingSessionIds.includes(session.session_id)) return;
    setSessionMenuOpenId(null);
    setSessionPendingDeletion(session);
  };

  const confirmDeleteSession = async () => {
    const session = sessionPendingDeletion;
    if (!session || deletingSessionIds.includes(session.session_id)) return;
    setSessionPendingDeletion(null);
    setError(null);
    setStatus(t('status.deleting_session'));
    try {
      const res = await fetch(`${API_BASE}/sessions/${encodeURIComponent(session.session_id)}`, { method: 'DELETE' });
      if (!res.ok) throw new Error(engineErr('engine.session_delete', res.status));
      setDeletingSessionIds((cur) => [...cur, session.session_id]);
      if (session.session_id === activeSessionId) { setActiveSessionId(null); setMessages([]); }
      setIndexedDocumentsBySession((cur) => { const n = { ...cur }; delete n[session.session_id]; return n; });
      setPinnedSessionIds((cur) => cur.filter((id) => id !== session.session_id));
      await new Promise((r) => window.setTimeout(r, 320));
      await loadSessions();
      setDeletingSessionIds((cur) => cur.filter((id) => id !== session.session_id));
      setStatus(t('status.ready'));
    } catch (e) {
      setDeletingSessionIds((cur) => cur.filter((id) => id !== session.session_id));
      setError(e instanceof Error ? e.message : t('error.could_not_delete_session'));
      setStatus(t('error.could_not_delete_session'));
    }
  };

  // --- Rename Session ---

  const beginRenamingSession = (session: EngineSessionSummary) => {
    if (isStreaming) return;
    setSessionMenuOpenId(null);
    setEditingSessionId(session.session_id);
    setEditingTitle(session.title);
    setError(null);
  };

  const submitRenamingSession = async (session: EngineSessionSummary) => {
    const nextTitle = editingTitle.trim();
    if (!nextTitle || nextTitle === session.title) { setEditingSessionId(null); setEditingTitle(''); return; }
    setError(null);
    setStatus(t('status.renaming_session'));
    try {
      const res = await fetch(`${API_BASE}/sessions/${encodeURIComponent(session.session_id)}`, { method: 'PATCH', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ title: nextTitle }) });
      if (!res.ok) throw new Error(engineErr('engine.session_rename', res.status));
      await loadSessions();
      setStatus(t('status.ready'));
      setEditingSessionId(null);
      setEditingTitle('');
    } catch (e) { setError(e instanceof Error ? e.message : t('error.could_not_rename_session')); setStatus(t('error.could_not_rename_session')); }
  };

  // --- File Upload ---

  const handleFileUpload = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const files = event.target.files;
    if (!files || files.length === 0 || isUploading) return;
    const selected = Array.from(files);
    const unsupported = selected.filter((f) => !['.pdf', '.txt'].some((ext) => f.name.toLowerCase().endsWith(ext)));
    if (unsupported.length > 0) { setError(`Unsupported file types: ${unsupported.map((f) => f.name).join(', ')}. Only PDF and TXT are supported.`); event.target.value = ''; return; }
    setIsUploading(true);
    setImportPhase('uploading');
    setImportProgress(3);
    setImportFileLabel(selected.length === 1 ? selected[0].name : `${selected.length} documents`);
    setStatus(activeSessionId ? t('status.indexing_documents') : t('status.starting_document_session'));
    setError(null);
    try {
      let sessionId = activeSessionId;
      let createdSessionId: string | null = null;
      if (!sessionId) { const s = await createSession(); sessionId = s.session_id; createdSessionId = s.session_id; setNewSessionPulseId(s.session_id); setMessages([]); }
      setStatus(t('status.indexing_documents'));
      const fd = new FormData();
      fd.append('session_id', sessionId);
      selected.forEach((f) => fd.append('file', f));
      const ingestRes = await new Promise<IngestResponse>((resolve, reject) => {
        const req = new XMLHttpRequest();
        req.open('POST', `${API_BASE}/ingest`);
        req.upload.onprogress = (e) => {
          if (!e.lengthComputable) { setImportProgress((c) => Math.max(c, 12)); return; }
          setImportProgress(Math.max(5, Math.min(70, Math.round((e.loaded / e.total) * 68))));
        };
        req.upload.onload = () => { setImportPhase('indexing'); setImportProgress(72); };
        req.onload = () => {
          if (req.status >= 200 && req.status < 300) { try { resolve(JSON.parse(req.responseText)); } catch { reject(new Error('Engine indexed the document but returned an unreadable response.')); } return; }
          reject(new Error(req.responseText || engineErr('engine.upload', req.status)));
        };
        req.onerror = () => reject(new Error(t('error.could_not_reach_engine')));
        req.onabort = () => reject(new Error('Document import was cancelled.'));
        req.send(fd);
      });
      setIndexedDocumentsBySession((cur) => ({ ...cur, [sessionId]: mergeIndexedDocuments(cur[sessionId] ?? [], ingestRes.documents) }));
      setImportFileLabel(ingestRes.documents.length === 1 ? ingestRes.documents[0].file_name : `${ingestRes.documents.length} documents`);
      setImportPhase('complete');
      setImportProgress(100);
      if (ingestRes.session) setActiveSessionId(ingestRes.session.session_id);
      await loadSessions();
      if (createdSessionId) window.setTimeout(() => setNewSessionPulseId((c) => c === createdSessionId ? null : c), 1400);
      setStatus(`Indexed ${ingestRes.total_chunks} document chunks`);
    } catch (e) { setImportPhase('error'); setImportProgress(100); setError(e instanceof Error ? e.message : t('error.upload_failed')); setStatus(t('error.upload_failed')); }
    finally { setIsUploading(false); event.target.value = ''; window.setTimeout(() => { setImportPhase('idle'); setImportProgress(0); setImportFileLabel(''); }, 1800); }
  };

  const deleteIndexedDocumentFromRag = async (sessionId: string, document: IndexedDocument): Promise<DeleteIndexedDocumentResponse> => {
    const res = await fetch(`${API_BASE}/ingest/document`, { method: 'DELETE', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ session_id: sessionId, stored_path: document.stored_path }) });
    if (!res.ok) { const body = await res.text(); throw new Error(body || engineErr('engine.document_remove', res.status, document.file_name)); }
    return (await res.json()) as DeleteIndexedDocumentResponse;
  };

  const clearIndexedDocuments = async () => {
    if (!activeSessionId || indexedDocuments.length === 0 || isClearingIndexedDocuments) return;
    const sessionId = activeSessionId;
    const docs = [...indexedDocuments];
    setIsClearingIndexedDocuments(true);
    setDocumentContextNotice(null);
    setError(null);
    setStatus(t('status.removing_document_context'));
    try {
      const results = await Promise.allSettled(docs.map((d) => deleteIndexedDocumentFromRag(sessionId, d)));
      const removedPaths = new Set<string>();
      const failures: string[] = [];
      let deletedChunks = 0;
      results.forEach((r, i) => {
        const doc = docs[i];
        if (r.status === 'fulfilled') { removedPaths.add(doc.stored_path); deletedChunks += Math.max(0, Number(r.value.deleted_chunks ?? 0)); return; }
        failures.push(r.reason instanceof Error ? r.reason.message : `Could not remove ${doc.file_name}.`);
      });
      if (removedPaths.size > 0) {
        setIndexedDocumentsBySession((cur) => {
          const remaining = (cur[sessionId] ?? []).filter((d) => !removedPaths.has(d.stored_path));
          const nxt = { ...cur };
          if (remaining.length > 0) nxt[sessionId] = remaining; else delete nxt[sessionId];
          return nxt;
        });
      }
      setImportPhase('idle'); setImportProgress(0); setImportFileLabel('');
      if (failures.length > 0) { setError(`Could not remove ${failures.length} imported document${failures.length === 1 ? '' : 's'} from RAG memory. ${failures[0]}`); setStatus(t('error.document_removal_incomplete')); return; }
      setStatus(deletedChunks > 0 ? `Removed ${deletedChunks} document chunks` : 'Removed document context');
      setDocumentContextNotice('Imported document cleared.');
    } finally { setIsClearingIndexedDocuments(false); }
  };

  // --- Calendar ---

  const loadOutlookCalendars = async () => {
    setLoadingOutlookCalendars(true);
    try {
      const res = await fetch(`${API_BASE}/calendar/outlook/calendars`);
      if (!res.ok) { const body = await res.text(); throw new Error(body || engineErr('engine.calendars_load', res.status)); }
      const data = (await res.json()) as OutlookCalendarsResponse;
      const visible = data.calendars.filter(isVisibleOutlookCalendar);
      setOutlookCalendars(visible);
      setSelectedOutlookCalendarId(visible.find((c) => c.is_selected)?.id ?? '');
    } catch (e) { setError(e instanceof Error ? e.message : t('error.could_not_load_calendars')); }
    finally { setLoadingOutlookCalendars(false); }
  };

  const selectOutlookCalendar = async (calendarId: string) => {
    setSelectedOutlookCalendarId(calendarId);
    setCalendarMessage(null);
    if (!calendarId) return;
    try {
      const res = await fetch(`${API_BASE}/calendar/outlook/select`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ calendar_id: calendarId }) });
      if (!res.ok) { const body = await res.text(); throw new Error(body || engineErr('engine.calendar_select', res.status)); }
      const data = (await res.json()) as OutlookCalendarSelectionResponse;
      setCalendarMessage(`Outlook calendar selected: ${outlookCalendarLabel(data.calendar)}`);
      setOutlookCalendars((cur) => cur.map((c) => ({ ...c, is_selected: c.id === data.calendar.id })));
    } catch (e) { setError(e instanceof Error ? e.message : t('error.could_not_select_calendar')); }
  };

  const openCalendarTool = () => {
    setCalendarOpen(true);
    setToolsOpen(false);
    setError(null);
    setCalendarPrompt('');
    setCalendarResult(null);
    setCalendarMessage(null);
    void loadOutlookCalendars();
  };

  const openObsidianTool = () => {
    setObsidianOpen(true);
    setToolsOpen(false);
    setError(null);
  };

  const createCalendarEvent = async () => {
    const prompt = calendarPrompt.trim();
    if (!prompt || creatingCalendarEvent) return;
    setCreatingCalendarEvent(true);
    setError(null);
    setCalendarResult(null);
    setCalendarMessage(null);
    setStatus(t('status.creating_calendar_event'));
    try {
      const res = await fetch(`${API_BASE}/calendar/create-from-prompt`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ prompt }) });
      if (!res.ok) { const body = await res.text(); throw new Error(body || engineErr('engine.calendar_create', res.status)); }
      const data = (await res.json()) as CalendarCreateResponse;
      setCalendarResult(data.parsed);
      setCalendarMessage(data.message);
      setStatus(data.saved_to_calendar ? 'Calendar event saved' : 'Calendar event created');
    } catch (e) { setError(e instanceof Error ? e.message : t('error.could_not_create_calendar_event')); setStatus(t('error.calendar_failed')); }
    finally { setCreatingCalendarEvent(false); }
  };

  // --- Export ---

  const exportChatAsPdf = () => {
    if (messages.length === 0) return;
    downloadConversationPdf({ title: activeSession?.title ?? 'AEGIS Chat Export', sessionId: activeSession?.session_id, messages, indexedDocuments });
    setToolsOpen(false);
  };

  const exportSessionAsPdf = async (sessionSummary: EngineSessionSummary) => {
    if (isStreaming) return;
    setSessionMenuOpenId(null);
    setError(null);
    setStatus(t('status.preparing_export'));
    try {
      const res = await fetch(`${API_BASE}/sessions/${encodeURIComponent(sessionSummary.session_id)}`);
      if (!res.ok) throw new Error(engineErr('engine.session_export', res.status));
      const session = (await res.json()) as EngineSession;
      downloadConversationPdf({ title: session.title || sessionSummary.title || 'AEGIS Chat Export', sessionId: session.session_id, messages: turnsToMessages(session.history.turns, session.session_id), indexedDocuments: indexedDocumentsBySession[session.session_id] ?? [] });
      setStatus(t('status.export_ready'));
    } catch (e) { setError(e instanceof Error ? e.message : t('error.could_not_export_session')); setStatus(t('error.export_failed')); }
  };

  const togglePinnedSession = (sessionId: string) => {
    setPinnedSessionIds((cur) => cur.includes(sessionId) ? cur.filter((id) => id !== sessionId) : [sessionId, ...cur]);
    setSessionMenuOpenId(null);
  };

  // --- Settings ---

  const openSettings = (tab: SettingsTab = 'general') => {
    if (settingsCloseTimeoutRef.current !== null) { window.clearTimeout(settingsCloseTimeoutRef.current); settingsCloseTimeoutRef.current = null; }
    setSettingsTab(tab);
    setSettingsClosing(false);
    setSettingsOpen(true);
    setSettingsMessage(null);
  };

  const closeSettings = () => {
    if (!settingsOpen || settingsClosing) return;
    setSettingsClosing(true);
    if (settingsCloseTimeoutRef.current !== null) window.clearTimeout(settingsCloseTimeoutRef.current);
    settingsCloseTimeoutRef.current = window.setTimeout(() => { setSettingsOpen(false); setSettingsClosing(false); settingsCloseTimeoutRef.current = null; }, 200);
  };

  const selectProvider = async (providerName: string) => {
    setSettingsMessage(null);
    try {
      const res = await fetch(`${API_BASE}/providers/select`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ name: providerName }) });
      if (!res.ok) { const body = await res.text(); throw new Error(body || engineErr('engine.provider_switch', res.status)); }
      await loadSettingsData();
      setSettingsMessage(`Inference provider switched to ${providerName}.`);
    } catch (e) { setSettingsMessage(e instanceof Error ? e.message : t('error.could_not_switch_provider')); }
  };

  const selectModel = async (modelName: string) => {
    setSettingsMessage(null);
    try {
      const res = await fetch(`${API_BASE}/models/select`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ name: modelName }) });
      if (!res.ok) { const body = await res.text(); throw new Error(body || engineErr('engine.model_switch', res.status)); }
      await loadSettingsData();
      setSettingsMessage(`Active model switched to ${modelName}.`);
    } catch (e) { setSettingsMessage(e instanceof Error ? e.message : t('error.could_not_switch_model')); }
  };

  const downloadModel = async (modelNameOverride?: string) => {
    const modelName = (modelNameOverride ?? modelSearch).trim();
    if (!modelName || modelDownloadState === 'downloading') return;
    const providerName = activeProvider?.name ?? 'active provider';
    const controller = new AbortController();
    modelDownloadAbortRef.current = controller;
    modelDownloadAbortReasonRef.current = null;
    setDownloadingModel(modelName);
    setPausedModelDownload(null);
    setModelDownloadState('downloading');
    setModelDownloadProgress(0);
    setModelDownloadStatus('Starting download');
    setSettingsMessage(null);
    try {
      const res = await fetch(`${API_BASE}/models/download`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ name: modelName }), signal: controller.signal });
      if (!res.ok || !res.body) { const body = await res.text(); throw new Error(body || engineErr('engine.model_download', res.status)); }
      const reader = res.body.getReader();
      const decoder = new TextDecoder();
      let pending = '';
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        pending += decoder.decode(value, { stream: true });
        const parsed = extractSseEvents(pending);
        pending = parsed.remaining;
        for (const evt of parsed.events) {
          const data = sseEventData(evt);
          if (!data) continue;
          const chunk = JSON.parse(data) as PullModelChunk;
          if (chunk.error) throw new Error(chunk.error);
          setModelDownloadStatus(chunk.status ?? 'Downloading');
          const pct = modelDownloadPercent(chunk);
          if (pct !== null) setModelDownloadProgress(pct);
        }
      }
      setModelDownloadProgress(100);
      setModelDownloadStatus('Download complete');
      await loadSettingsData();
      setSettingsMessage(modelReadyMessage(modelName, providerName));
    } catch (e) {
      if (controller.signal.aborted) return;
      setModelDownloadStatus('Download failed');
      setSettingsMessage(e instanceof Error ? e.message : t('error.could_not_download_model'));
    } finally {
      const reason = modelDownloadAbortReasonRef.current;
      modelDownloadAbortRef.current = null;
      modelDownloadAbortReasonRef.current = null;
      if (reason === 'pause') { setPausedModelDownload(modelName); setDownloadingModel(null); setModelDownloadState('paused'); setModelDownloadStatus('Paused'); }
      else if (reason === 'cancel') { setPausedModelDownload(null); setDownloadingModel(null); setModelDownloadState('idle'); setModelDownloadProgress(0); setModelDownloadStatus(''); }
      else { setPausedModelDownload(null); setDownloadingModel(null); setModelDownloadState('idle'); }
    }
  };

  const pauseModelDownload = () => {
    if (!downloadingModel || modelDownloadState !== 'downloading') return;
    modelDownloadAbortReasonRef.current = 'pause';
    setModelDownloadStatus('Pausing');
    modelDownloadAbortRef.current?.abort();
  };

  const cancelModelDownload = () => {
    if (!downloadingModel && !pausedModelDownload) return;
    modelDownloadAbortReasonRef.current = 'cancel';
    modelDownloadAbortRef.current?.abort();
    setPausedModelDownload(null); setDownloadingModel(null); setModelDownloadState('idle'); setModelDownloadProgress(0); setModelDownloadStatus('');
    setSettingsMessage(t('error.model_download_cancelled'));
  };

  const resumeModelDownload = () => { if (pausedModelDownload) void downloadModel(pausedModelDownload); };

  const handleAddMemory = () => {
    const memory = memoryInput.trim();
    if (!memory) return;
    const timestamp = new Date().toLocaleString();
    const entry = `- ${memory} (remembered on ${timestamp})`;
    setProfileText((prev) => {
      const text = prev.trim();
      return text ? `${text}\n${entry}` : entry;
    });
    setMemoryInput('');
    setSettingsMessage(t('error.memory_added'));
  };

  const saveProfileSettings = async () => {
    setSettingsMessage(null);
    try {
      const res = await fetch(`${API_BASE}/profile`, { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ contents: profileText }) });
      if (!res.ok) { const body = await res.text(); throw new Error(body || engineErr('engine.profile_save', res.status)); }
      const data = (await res.json()) as ProfileResponse;
      setProfileText(data.contents);
      setProfilePath(data.path);
      setSettingsMessage(t('error.memories_saved'));
    } catch (e) { setSettingsMessage(e instanceof Error ? e.message : t('error.could_not_save_profile')); }
  };

  // --- Core Streaming ---

  async function streamPrompt(prompt: string, nextMessages: Message[], editFromTurnIndex?: number) {
    setError(null);
    setStatus(t('status.inference'));
    setIsStreaming(true);

    let targetSessionId: string | null = null;
    let seedMessages = nextMessages;
    const pendingSegments: string[] = [];
    let streamFlushTimer: number | null = null;
    let streamDrainResolver: (() => void) | null = null;
    let streamClosed = false;

    const updateTarget = (updater: (cur: Message[]) => Message[]) => {
      if (!targetSessionId) return;
      const sid = targetSessionId;
      const updated = updater(streamingMessagesBySessionRef.current[sid] ?? seedMessages);
      updated.forEach((msg, idx) => { if (msg.role === 'assistant' && msg.sources && msg.sources.length > 0) localStorage.setItem(`aegis-sources-${sid}-${idx}`, JSON.stringify(msg.sources)); });
      streamingMessagesBySessionRef.current = { ...streamingMessagesBySessionRef.current, [sid]: updated };
      setStreamingMessagesBySession(streamingMessagesBySessionRef.current);
      if (activeSessionIdRef.current === sid) setMessages(updated);
    };

    const settleDrain = () => { if (streamClosed && pendingSegments.length === 0 && streamFlushTimer === null && streamDrainResolver) { const r = streamDrainResolver; streamDrainResolver = null; r(); } };
    const flushSegments = (forceAll = false) => {
      streamFlushTimer = null;
      if (pendingSegments.length === 0) { settleDrain(); return; }
      const count = forceAll ? pendingSegments.length : pendingSegments.length > 48 ? 8 : pendingSegments.length > 24 ? 5 : pendingSegments.length > 12 ? 3 : 1;
      const chunk = pendingSegments.splice(0, count).join('');
      updateTarget((cur) => { const next = [...cur]; const last = next[next.length - 1]; if (last?.role === 'assistant') next[next.length - 1] = { ...last, content: `${last.content}${chunk}`, timestamp: last.timestamp ?? new Date().toISOString() }; return next; });
      if (pendingSegments.length > 0) { streamFlushTimer = window.setTimeout(() => flushSegments(), pendingSegments.length > 60 ? 10 : pendingSegments.length > 28 ? 14 : 18); return; }
      settleDrain();
    };
    const scheduleFlush = () => { if (streamFlushTimer === null) streamFlushTimer = window.setTimeout(() => flushSegments(), 12); };
    const enqueue = (content: string) => { if (content) { pendingSegments.push(...splitAssistantStreamSegments(content)); scheduleFlush(); } };
    const waitDrain = () => pendingSegments.length === 0 && streamFlushTimer === null ? Promise.resolve() : new Promise<void>((r) => { streamDrainResolver = r; });

    inferenceStartTime.current = Date.now();
    setInferenceStats({ latency: 0, tps: 0, ttft: 0, ragTime: 0, similarity: 0, chunks: 0, backend: '---' });

    try {
      let sessionId = activeSessionId;
      let createdSessionId: string | null = null;
      if (!sessionId) { const s = await createSession(); sessionId = s.session_id; createdSessionId = s.session_id; setNewSessionPulseId(s.session_id); await loadSessions(); }
      targetSessionId = sessionId;
      seedMessages = nextMessages;
      setStreamingSessionId(sessionId);
      streamingMessagesBySessionRef.current = { ...streamingMessagesBySessionRef.current, [sessionId]: seedMessages };
      setStreamingMessagesBySession(streamingMessagesBySessionRef.current);
      if (activeSessionIdRef.current === sessionId) setMessages(seedMessages);

      const res = await fetch(`${API_BASE}/chat`, {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          session_id: sessionId, message: prompt,
          attachments: indexedDocuments.map((d) => `${d.file_name} (${d.chunks_added} chunks)`),
          edit_from_turn_index: editFromTurnIndex, mode: chatMode,
          response_style: responseStyle, code_project_name: activeProject?.name,
          code_project_context: activeProject?.snapshot, rag_enabled: isRagEnabled,
          rag_top_k: ragTopK, rag_similarity_threshold: ragSimilarityThreshold,
        }),
      });
      if (!res.ok || !res.body) throw new Error(engineErr('engine.chat_send', res.status));

      const reader = res.body.getReader();
      const decoder = new TextDecoder();
      let pending = '';
      let accumulated = '';

      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        pending += decoder.decode(value, { stream: true });
        const parsed = extractSseEvents(pending);
        pending = parsed.remaining;
        for (const evt of parsed.events) {
          const data = sseEventData(evt);
          if (!data) continue;
          if (data.startsWith('[RAG_METRICS] ')) {
            try { const m = JSON.parse(data.replace('[RAG_METRICS] ', '')); setInferenceStats((p) => ({ ...p, ragTime: m.retrieval_time_ms, similarity: m.avg_similarity, chunks: m.chunk_count, backend: m.backend })); } catch {}
            continue;
          }
          if (data.startsWith('[RAG_SOURCES] ')) {
            try { const parsedSources = JSON.parse(data.replace('[RAG_SOURCES] ', '')) as RetrievalChunk[]; updateTarget((cur) => { const n = [...cur]; const l = n[n.length - 1]; if (l?.role === 'assistant') n[n.length - 1] = { ...l, sources: parsedSources }; return n; }); } catch {}
            continue;
          }
          if (data === '[DONE]') { setStatus(t('status.complete')); continue; }
          if (data.startsWith('[ERROR]')) throw new Error(data);
          if (accumulated === '' && inferenceStartTime.current) setInferenceStats((p) => ({ ...p, ttft: Date.now() - (inferenceStartTime.current ?? 0) }));
          accumulated += data;
          enqueue(data);
        }
      }

      const finalData = sseEventData(pending);
      if (finalData && finalData !== '[DONE]') {
        if (finalData.startsWith('[ERROR]')) throw new Error(finalData);
        if (accumulated === '' && inferenceStartTime.current) setInferenceStats((p) => ({ ...p, ttft: Date.now() - (inferenceStartTime.current ?? 0) }));
        accumulated += finalData;
        enqueue(finalData);
      }

      streamClosed = true;
      await waitDrain();

      const totalLatency = Date.now() - (inferenceStartTime.current ?? Date.now());
      const charCount = accumulated.length;
      setInferenceStats((p) => ({ ...p, latency: totalLatency, tps: totalLatency > 0 ? parseFloat(((Math.max(1, Math.floor(charCount / 4)) / totalLatency) * 1000).toFixed(1)) : 0 }));
      setStatus(t('status.complete'));
      await loadSessions();
      if (isTtsEnabled && accumulated) speakAssistantResponse(accumulated, false, messages.length - 1);
      try { setContextUsage(await fetchContextUsage(sessionId)); } catch {}
      if (createdSessionId) window.setTimeout(() => setNewSessionPulseId((c) => c === createdSessionId ? null : c), 1400);
    } catch (e) {
      if (pendingSegments.length > 0) flushSegments(true);
      if (streamFlushTimer !== null) { window.clearTimeout(streamFlushTimer); streamFlushTimer = null; }
      setError(e instanceof Error ? e.message : t('error.could_not_send_chat'));
      setStatus(t('error.chat_failed'));
      updateTarget((cur) => cur.filter((m) => m.content.length > 0));
    } finally {
      if (streamFlushTimer !== null) window.clearTimeout(streamFlushTimer);
      setIsStreaming(false);
      setStreamingSessionId(null);
      if (targetSessionId) {
        const n = { ...streamingMessagesBySessionRef.current };
        delete n[targetSessionId]; streamingMessagesBySessionRef.current = n;
        setStreamingMessagesBySession(n);
      }
    }
  }

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const prompt = input.trim();
    if (!prompt || isStreaming) return;
    setInput('');
    const ts = new Date().toISOString();
    await streamPrompt(prompt, [...messages, { role: 'user', content: prompt, timestamp: ts }, { role: 'assistant', content: '' }]);
  };

  const beginEditingMessage = (index: number, content: string) => {
    if (isStreaming) return;
    setEditingMessageIndex(index);
    setEditingMessageText(content);
    setError(null);
  };

  const cancelEditingMessage = () => { setEditingMessageIndex(null); setEditingMessageText(''); };

  const copyUserMessage = async (index: number, content: string) => {
    await copyTextToClipboard(content);
    setCopiedMessageIndex(index);
    window.setTimeout(() => setCopiedMessageIndex((c) => (c === index ? null : c)), 1400);
  };

  const resendEditedMessage = async (index: number) => {
    const prompt = editingMessageText.trim();
    if (!prompt || isStreaming) return;
    const turnIndex = messages.slice(0, index).filter((m) => m.role === 'user').length;
    setEditingMessageIndex(null);
    setEditingMessageText('');
    const ts = new Date().toISOString();
    await streamPrompt(prompt, [...messages.slice(0, index), { role: 'user', content: prompt, edited: true, timestamp: ts }, { role: 'assistant', content: '' }], turnIndex);
  };

  return (
    <I18nProvider lang={lang}>
    <div className={`aegis-shell aegis-mode-${theme} aegis-theme-${appearanceTheme} flex h-screen overflow-hidden ${isDark ? 'bg-zinc-950 text-zinc-100' : 'bg-stone-100 text-slate-900'}`} onClick={() => setSessionMenuOpenId(null)}>
      {/* Left Icon Bar */}
      <nav aria-label="Sidebar controls" className={`flex w-14 shrink-0 flex-col items-center border-r ${isDark ? 'border-zinc-800 bg-zinc-950' : 'border-stone-300 bg-stone-50'}`}>
        <button
          aria-label={sidebarOpen ? 'Close sidebar' : 'Open sidebar'}
          className={`mt-4 inline-flex h-9 w-9 items-center justify-center rounded-lg transition ${isDark ? 'text-zinc-400 hover:bg-zinc-900 hover:text-zinc-100' : 'text-slate-600 hover:bg-stone-200 hover:text-slate-950'}`}
          onClick={(e) => { e.stopPropagation(); setSidebarOpen((c) => !c); }}
          type="button"
        >
          {sidebarOpen ? <PanelLeftClose size={18} /> : <PanelLeftOpen size={18} />}
        </button>
        <button
          aria-label={isDark ? 'Switch to light mode' : 'Switch to dark mode'}
          className={`mt-2 inline-flex h-9 w-9 items-center justify-center rounded-lg transition ${isDark ? 'text-zinc-400 hover:bg-zinc-900 hover:text-zinc-100' : 'text-slate-600 hover:bg-stone-200 hover:text-slate-950'}`}
          onClick={(e) => { e.stopPropagation(); setTheme((c) => (c === 'dark' ? 'light' : 'dark')); }}
          type="button"
        >
          {isDark ? <Sun size={17} /> : <Moon size={17} />}
        </button>
        <button
          aria-label="Open settings"
          className={`aegis-accent-ghost mt-2 inline-flex h-9 w-9 items-center justify-center rounded-lg border border-transparent transition ${settingsOpen ? 'aegis-accent-subtle' : isDark ? 'text-zinc-400 hover:bg-zinc-900 hover:text-zinc-100' : 'text-slate-600 hover:bg-stone-200 hover:text-slate-950'}`}
          onClick={(e) => { e.stopPropagation(); openSettings(); }}
          type="button"
        >
          <Settings size={17} />
        </button>
      </nav>

      {/* Sidebar */}
      <Sidebar
        isDark={isDark}
        isStreaming={isStreaming}
        scanningProject={scanningProject}
        sidebarOpen={sidebarOpen}
        projectsOpen={projectsOpen}
        sessionsOpen={sessionsOpen}
        codeProjects={codeProjects}
        activeProjectId={activeProjectId}
        sessions={sessions}
        sortedSessions={sortedSessions}
        activeSessionId={activeSessionId}
        editingSessionId={editingSessionId}
        editingTitle={editingTitle}
        deletingSessionIds={deletingSessionIds}
        newSessionPulseId={newSessionPulseId}
        pinnedSessionIdSet={pinnedSessionIdSet}
        sessionMenuOpenId={sessionMenuOpenId}
        onToggleSidebar={() => setSidebarOpen((c) => !c)}
        onToggleProjects={() => setProjectsOpen((c) => !c)}
        onToggleSessions={() => setSessionsOpen((c) => !c)}
        onNewSession={handleNewSession}
        onAddProject={handleAddProject}
        onSelectProject={(id) => { setActiveProjectId(id); setChatMode('coder'); }}
        onRemoveProject={removeProject}
        onSelectSession={handleSessionSelect}
        onBeginRenaming={beginRenamingSession}
        onSubmitRenaming={submitRenamingSession}
        onCancelRenaming={() => { setEditingSessionId(null); setEditingTitle(''); }}
        onEditingTitleChange={setEditingTitle}
        onExportSession={exportSessionAsPdf}
        onTogglePinned={togglePinnedSession}
        onDeleteSession={handleDeleteSession}
        onSetSessionMenuOpen={setSessionMenuOpenId}
      />

      {/* Main Content */}
      <main className="relative flex min-w-0 flex-1 flex-col">
        <Header
          isDark={isDark}
          activeSessionTitle={activeSession?.title}
          activeSessionId={activeSessionId}
          chatMode={chatMode}
          isMetricsOpen={isMetricsOpen}
          status={status}
          onSetChatMode={setChatMode}
          onToggleMetrics={() => setIsMetricsOpen((c) => !c)}
        />

        {/* Resource Warning */}
        {visibleResourceWarning && (
          <div className={`flex items-center justify-between gap-4 border-b px-6 py-3 text-sm font-medium ${isDark ? 'border-amber-900/60 bg-amber-950/30 text-amber-200' : 'border-amber-200 bg-amber-50 text-amber-800'}`}>
            <span className="min-w-0 flex-1">Warning: {visibleResourceWarning}</span>
            <button aria-label={t('error.dismiss_warning')} className={`inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-md transition ${isDark ? 'text-amber-200/80 hover:bg-amber-900/40 hover:text-amber-100' : 'text-amber-700/80 hover:bg-amber-100 hover:text-amber-900'}`} onClick={() => setDismissedResourceWarning(visibleResourceWarning)} type="button">
              <X size={15} />
            </button>
          </div>
        )}

        {/* Error Banner */}
        {error && (
          <div className={`flex items-center justify-between gap-4 border-b px-6 py-3 text-sm ${isDark ? 'border-red-900/60 bg-red-950/30 text-red-200' : 'border-red-200 bg-red-50 text-red-700'}`} role="alert">
            <span className="min-w-0 flex-1">{error}</span>
            {errorDismissible && (
              <button aria-label={t('error.dismiss')} className={`inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-md transition ${isDark ? 'text-red-200/80 hover:bg-red-900/40 hover:text-red-100' : 'text-red-700/80 hover:bg-red-100 hover:text-red-900'}`} onClick={() => setError(null)} type="button">
                <X size={15} />
              </button>
            )}
          </div>
        )}

        {/* AEGIS heading — fixed position, unaffected by error banners */}
        {messages.length === 0 && (
          <div className={`absolute left-1/2 top-[calc(25vh+32px)] z-10 -translate-x-1/2 text-center text-8xl font-black tracking-[0.15em] transition-all duration-700 ${isDark ? 'text-white' : 'text-black'}`} style={{ textShadow: '0 0 1px currentColor' }}>
            AEGIS
          </div>
        )}

        {/* Messages Area */}
        <div ref={scrollRef} className={`min-h-0 flex-1 overflow-y-auto px-6 pb-12 pt-6 ${isDark ? 'bg-zinc-950' : 'bg-[radial-gradient(circle_at_top,_rgba(255,255,255,0.75),_rgba(245,245,244,0)_42%)]'}`}>
          <div className="mx-auto flex max-w-4xl flex-col gap-4">
            {messages.map((message, index) => (
                <MessageBubble
                  key={`${message.role}-${index}`}
                  message={message}
                  index={index}
                  isDark={isDark}
                  isStreaming={isStreaming}
                  editingMessageIndex={editingMessageIndex}
                  editingMessageText={editingMessageText}
                  copiedMessageIndex={copiedMessageIndex}
                  selectedMessageSourcesIndex={selectedMessageSourcesIndex}
                  speakingMessageIndex={speakingMessageIndex}
                  activeProject={activeProject}
                  onBeginEditing={beginEditingMessage}
                  onCancelEditing={cancelEditingMessage}
                  onEditingTextChange={setEditingMessageText}
                  onResendEdited={resendEditedMessage}
                  onCopyMessage={copyUserMessage}
                  onToggleSources={(index, sources) => {
                    if (selectedMessageSourcesIndex === index) {
                      setSelectedMessageSources(null);
                      setSelectedMessageSourcesIndex(null);
                      setMetricsTab('metrics');
                    } else {
                      setSelectedMessageSourcesIndex(index);
                      setSelectedMessageSources(sources);
                      setMetricsTab('sources');
                      setIsMetricsOpen(true);
                    }
                  }}
                  onSpeak={speakAssistantResponse}
                  onApplyPatch={() => applyAssistantPatch(message.content)}
                />
              ))}
          </div>
        </div>

        {/* Composer Footer */}
        <Composer
          isDark={isDark}
          isStreaming={isStreaming}
          isUploading={isUploading}
          isClearingIndexedDocuments={isClearingIndexedDocuments}
          isVoiceMode={isVoiceMode}
          showCenteredComposer={showCenteredComposer}
          showImportProgress={showImportProgress}
          toolsOpen={toolsOpen}
          input={input}
          importPhase={importPhase}
          importProgress={importProgress}
          importFileLabel={importFileLabel}
          indexedDocuments={indexedDocuments}
          indexedDocumentLabel={indexedDocumentLabel}
          indexedChunkCount={indexedChunkCount}
          documentContextNotice={documentContextNotice}
          activeProject={activeProject}
          projectEditMessage={projectEditMessage}
          tokenMeterLabel={tokenMeterLabel}
          contextUsage={contextUsage}
          activeWelcomeMessage={activeWelcomeMessage}
          profileText={profileText}
          fileInputRef={fileInputRef}
          composerTextareaRef={composerTextareaRef}
          onInputChange={setInput}
          onSubmit={handleSubmit}
          onToggleTools={() => setToolsOpen((c) => !c)}
          onImportClick={() => { setToolsOpen(false); fileInputRef.current?.click(); }}
          onCalendarOpen={openCalendarTool}
          onExportPdf={exportChatAsPdf}
          obsidianEnabled={obsidianEnabled}
          onObsidianOpen={openObsidianTool}
          onFileUpload={handleFileUpload}
          onClearDocuments={clearIndexedDocuments}
          onVoiceModeOpen={() => setIsVoiceMode(true)}
          onDetachProject={() => setActiveProjectId(null)}
        />

        {/* Voice Mode Overlay */}
        {isVoiceMode && (
          <VoiceModeOverlay
            isDark={isDark}
            isRecording={isRecording}
            isSpeaking={isSpeaking}
            isTranscribing={isTranscribing}
            isStreaming={isStreaming}
            isTtsEnabled={isTtsEnabled}
            analyser={analyser}
            messages={messages}
            onClose={() => setIsVoiceMode(false)}
            onToggleTts={() => changeTtsEnabled(!isTtsEnabled)}
            onStartRecording={startRecording}
            onStopDictation={handleStopDictation}
          />
        )}
      </main>

      {/* Project Permission Modal */}
      {projectPermissionRequestId && (
        <ProjectPermissionModal
          isDark={isDark}
          onClose={() => setProjectPermissionRequestId(null)}
          onKeepReadonly={() => { setProjectPermissionRequestId(null); setProjectEditMessage('Project attached in read-only mode.'); }}
          onRequestEditAccess={() => requestProjectWritePermission(projectPermissionRequestId)}
        />
      )}

      {/* Delete Confirmation Modal */}
      {sessionPendingDeletion && (
        <DeleteConfirmModal
          isDark={isDark}
          session={sessionPendingDeletion}
          onClose={() => setSessionPendingDeletion(null)}
          onConfirm={confirmDeleteSession}
        />
      )}

      {/* Settings Panel */}
      <SettingsPanel
        isDark={isDark}
        settingsOpen={settingsOpen}
        settingsClosing={settingsClosing}
        settingsTab={settingsTab}
        settingsMessage={settingsMessage}
        settingsLoading={settingsLoading}
        theme={theme}
        appearanceTheme={appearanceTheme}
        responseStyle={responseStyle}
        availableModels={availableModels}
        availableProviders={availableProviders}
        activeProvider={activeProvider}
        modelSearch={modelSearch}
        selectedModelProviderTag={selectedModelProviderTag}
        filteredCatalogModels={filteredCatalogModels}
        downloadingModel={downloadingModel}
        pausedModelDownload={pausedModelDownload}
        modelDownloadState={modelDownloadState}
        modelDownloadProgress={modelDownloadProgress}
        modelDownloadStatus={modelDownloadStatus}
        isVoiceLowRamMode={isVoiceLowRamMode}
        isTtsEnabled={isTtsEnabled}
        isRagEnabled={isRagEnabled}
        ragTopK={ragTopK}
        ragSimilarityThreshold={ragSimilarityThreshold}
        profileText={profileText}
        profilePath={profilePath}
        memoryInput={memoryInput}
        onClose={closeSettings}
        onSetSettingsTab={setSettingsTab}
        onSetTheme={setTheme}
        onSetAppearanceTheme={(t) => setAppearanceTheme(t as AppearanceTheme)}
        onSetResponseStyle={(s) => setResponseStyle(s as ResponseStyle)}
        onSelectModel={selectModel}
        onSelectProvider={selectProvider}
        onModelSearchChange={setModelSearch}
        onSetModelProviderTag={setSelectedModelProviderTag}
        onDownloadModel={downloadModel}
        onPauseDownload={pauseModelDownload}
        onCancelDownload={cancelModelDownload}
        onResumeDownload={resumeModelDownload}
        onToggleVoiceLowRam={toggleVoiceLowRamMode}
        onToggleTts={changeTtsEnabled}
        onToggleRag={toggleRagEnabled}
        onChangeRagTopK={changeRagTopK}
        onChangeRagThreshold={changeRagThreshold}
        onMemoryInputChange={setMemoryInput}
        onAddMemory={handleAddMemory}
        onDisplayMemories={() => setMemoriesPopupOpen(true)}
        onSaveProfile={saveProfileSettings}
        lang={lang}
        onSetLanguage={setLang}
        obsidianVaultPath={obsidianVaultPath}
        onObsidianVaultPathChange={setObsidianVaultPath}
        obsidianEnabled={obsidianEnabled}
        onObsidianEnabledChange={setObsidianEnabled}
      />

      {/* Memories Popup */}
      <MemoriesPopup
        isDark={isDark}
        isOpen={memoriesPopupOpen}
        memories={profileText.split('\n').filter((l) => l.trim().startsWith('- ')).map((l) => l.replace(/^-\s+/, ''))}
        onClose={() => setMemoriesPopupOpen(false)}
      />

      {/* Calendar Modal */}
      <CalendarModal
        isDark={isDark}
        calendarOpen={calendarOpen}
        calendarPrompt={calendarPrompt}
        creatingCalendarEvent={creatingCalendarEvent}
        loadingOutlookCalendars={loadingOutlookCalendars}
        outlookCalendars={outlookCalendars}
        selectedOutlookCalendarId={selectedOutlookCalendarId}
        calendarResult={calendarResult}
        calendarMessage={calendarMessage}
        onClose={() => setCalendarOpen(false)}
        onCalendarPromptChange={setCalendarPrompt}
        onCalendarSelect={selectOutlookCalendar}
        onCreateEvent={createCalendarEvent}
      />

      {/* Obsidian Modal */}
      <ObsidianModal
        isDark={isDark}
        isOpen={obsidianOpen}
        onClose={() => setObsidianOpen(false)}
        vaultPath={obsidianVaultPath}
      />

      {/* Metrics Sidebar */}
      <MetricsSidebar
        isDark={isDark}
        isMetricsOpen={isMetricsOpen}
        metricsTab={metricsTab}
        systemStats={systemStats}
        inferenceStats={inferenceStats}
        selectedMessageSources={selectedMessageSources}
        selectedMessageSourcesIndex={selectedMessageSourcesIndex}
        onClose={() => setIsMetricsOpen(false)}
        onSetMetricsTab={setMetricsTab}
        onClearSelection={() => { setSelectedMessageSources(null); setSelectedMessageSourcesIndex(null); }}
      />
    </div>
    </I18nProvider>
  );
}
