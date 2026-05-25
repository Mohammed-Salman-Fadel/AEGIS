import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { FormEvent, ReactNode } from 'react';
import {
  Activity,
  Bot,
  Calendar,
  ChevronDown,
  Check,
  Copy,
  Cpu,
  Download,
  Edit3,
  FileCode,
  FolderOpen,
  FolderPlus,
  GraduationCap,
  MessageSquare,
  Mic,
  MoreHorizontal,
  Moon,
  PanelLeftClose,
  PanelLeftOpen,
  Pause,
  Pin,
  Play,
  Send,
  Settings,
  Sun,
  Trash2,
  Upload,
  User,
  Volume2,
  VolumeX,
  Wrench,
  X,
  BookOpen,
  FileText,
} from 'lucide-react';
import { VoiceOrb } from './components/VoiceOrb';
import { useAudioRecorder } from './hooks/useAudioRecorder';

type Role = 'user' | 'assistant';
type ThemeMode = 'dark' | 'light';
type AppearanceTheme = 'default' | 'terminal' | 'ocean' | 'ember' | 'rose' | 'slate';
type MarkdownHeadingLevel = 1 | 2 | 3 | 4 | 5 | 6;
type MarkdownBlock =
  | { type: 'heading'; level: MarkdownHeadingLevel; text: string }
  | { type: 'paragraph'; text: string }
  | { type: 'ordered'; items: string[] }
  | { type: 'unordered'; items: string[] }
  | { type: 'code'; text: string; language: string };

type ChatMode = 'general' | 'coder' | 'academic';
type SettingsTab = 'general' | 'inference' | 'models' | 'personalize' | 'voice' | 'rag';
type ResponseStyle = 'default' | 'friendly' | 'concise' | 'elaborate' | 'technical';
type ModelDownloadState = 'idle' | 'downloading' | 'paused';

interface CatalogModel {
  name: string;
  provider: string;
  tags: string[];
  description: string;
}

interface RetrievalChunk {
  text: string;
  source: string;
  page?: number;
  score: number;
}

interface Message {
  role: Role;
  content: string;
  edited?: boolean;
  timestamp?: string;
  sources?: RetrievalChunk[];
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
  created_at?: string;
  edited?: boolean;
}

interface EngineSession {
  session_id: string;
  title: string;
  history: {
    turns: EngineTurn[];
  };
}

interface CalendarResult {
  title: string;
  start: string;
  end: string;
  description?: string | null;
  location?: string | null;
}

interface CalendarCreateResponse {
  message: string;
  saved_to_calendar?: boolean;
  file_opened?: boolean;
  delivery_method?: string;
  parsed: CalendarResult | null;
}

interface OutlookCalendar {
  id: string;
  name: string;
  store_name: string;
  email_address?: string | null;
  path: string;
  is_selected: boolean;
}

interface OutlookCalendarsResponse {
  calendars: OutlookCalendar[];
}

interface OutlookCalendarSelectionResponse {
  calendar: OutlookCalendar;
  message: string;
}

interface SystemStats {
  cpu: number;
  ram: number;
}

interface ContextUsage {
  provider: string;
  model: string;
  used_tokens: number;
  context_window: number;
  usage_source: string;
}

interface IndexedDocument {
  file_name: string;
  stored_path: string;
  chunks_added: number;
}

interface IngestResponse {
  status: string;
  total_chunks: number;
  documents: IndexedDocument[];
  session?: EngineSession | null;
}

interface DeleteIndexedDocumentResponse {
  status: string;
  deleted_chunks: number;
}

interface ModelResponse {
  name: string;
  description: string;
  active: boolean;
}

interface ModelListResponse {
  provider: string;
  models: ModelResponse[];
}

interface ProviderResponse {
  name: string;
  description: string;
  active: boolean;
}

interface ProviderListResponse {
  providers: ProviderResponse[];
}

interface ProfileResponse {
  contents: string;
  path: string;
}

interface PullModelChunk {
  status?: string;
  digest?: string;
  total?: number;
  completed?: number;
  error?: string;
}

interface FileSystemHandlePermissionDescriptor {
  mode?: 'read' | 'readwrite';
}

interface FileSystemHandle {
  kind: 'file' | 'directory';
  name: string;
  queryPermission?: (descriptor?: FileSystemHandlePermissionDescriptor) => Promise<PermissionState>;
  requestPermission?: (descriptor?: FileSystemHandlePermissionDescriptor) => Promise<PermissionState>;
}

interface FileSystemFileHandle extends FileSystemHandle {
  kind: 'file';
  getFile: () => Promise<File>;
  createWritable?: () => Promise<FileSystemWritableFileStream>;
}

interface FileSystemDirectoryHandle extends FileSystemHandle {
  kind: 'directory';
  entries: () => AsyncIterableIterator<[string, FileSystemFileHandle | FileSystemDirectoryHandle]>;
  getFileHandle?: (name: string, options?: { create?: boolean }) => Promise<FileSystemFileHandle>;
  getDirectoryHandle?: (
    name: string,
    options?: { create?: boolean },
  ) => Promise<FileSystemDirectoryHandle>;
}

interface FileSystemWritableFileStream extends WritableStream {
  write: (data: string | Blob | BufferSource) => Promise<void>;
  close: () => Promise<void>;
}

interface ProjectFileSnapshot {
  path: string;
  content: string;
  size: number;
  handle: FileSystemFileHandle;
}

interface CodeProject {
  id: string;
  name: string;
  fileCount: number;
  totalBytes: number;
  snapshot: string;
  files: ProjectFileSnapshot[];
  writable: boolean;
  updatedAt: string;
  rootHandle: FileSystemDirectoryHandle;
}

declare global {
  interface Window {
    showDirectoryPicker?: () => Promise<FileSystemDirectoryHandle>;
  }
}

type ImportPhase = 'idle' | 'uploading' | 'indexing' | 'complete' | 'error';

const API_BASE = '/api';
const THEME_STORAGE_KEY = 'aegis-ui-theme';
const APPEARANCE_THEME_STORAGE_KEY = 'aegis-ui-appearance-theme';
const INDEXED_DOCUMENTS_STORAGE_KEY = 'aegis-indexed-documents-by-session';
const PINNED_SESSIONS_STORAGE_KEY = 'aegis-pinned-session-ids';
const RESPONSE_STYLE_STORAGE_KEY = 'aegis-response-style';
const MAX_PROJECT_FILES = 120;
const MAX_PROJECT_FILE_BYTES = 64 * 1024;
const MAX_PROJECT_CONTEXT_CHARS = 120_000;
const IGNORED_PROJECT_DIRECTORIES = new Set([
  '.git',
  '.next',
  '.svelte-kit',
  '.venv',
  'dist',
  'node_modules',
  'target',
  'vendor',
]);
const IGNORED_PROJECT_FILES = new Set([
  'package-lock.json',
  'pnpm-lock.yaml',
  'yarn.lock',
  'Cargo.lock',
]);
const CODE_PROJECT_EXTENSIONS = new Set([
  '.c',
  '.cpp',
  '.cs',
  '.css',
  '.go',
  '.h',
  '.html',
  '.java',
  '.js',
  '.json',
  '.jsx',
  '.md',
  '.py',
  '.rs',
  '.toml',
  '.ts',
  '.tsx',
  '.vue',
  '.yaml',
  '.yml',
]);
const DEFAULT_WELCOME_MESSAGES = [
  'Welcome back [insert_name]!',
  'How may I assist you today?',
  'What should we build or explore next?',
  'Ready when you are.',
  'What would you like AEGIS to help with?',
];

const RESPONSE_STYLE_OPTIONS: Array<{ value: ResponseStyle; label: string; description: string }> = [
  {
    value: 'default',
    label: 'Default',
    description: 'Balanced, direct, and close to the original AEGIS assistant behavior.',
  },
  {
    value: 'friendly',
    label: 'Friendly',
    description: 'Warmer and more conversational while staying concise.',
  },
  {
    value: 'concise',
    label: 'Concise',
    description: 'Shorter answers that prioritize the direct result.',
  },
  {
    value: 'elaborate',
    label: 'Elaborate',
    description: 'More detailed explanations with fuller context and reasoning.',
  },
  {
    value: 'technical',
    label: 'Technical',
    description: 'Precise engineering-oriented responses with implementation detail.',
  },
];

const APPEARANCE_THEME_OPTIONS: Array<{
  value: AppearanceTheme;
  label: string;
  description: string;
  preview: string;
}> = [
  {
    value: 'default',
    label: 'Default',
    description: 'The current AEGIS emerald look with the clean baseline palette.',
    preview: 'linear-gradient(135deg, #10b981 0%, #047857 100%)',
  },
  {
    value: 'terminal',
    label: 'Terminal',
    description: 'Neon green accents with a sharper operator-console feel.',
    preview: 'linear-gradient(135deg, #84cc16 0%, #14532d 100%)',
  },
  {
    value: 'ocean',
    label: 'Ocean',
    description: 'Cool cyan and deep blue accents for a crisp analytical feel.',
    preview: 'linear-gradient(135deg, #38bdf8 0%, #1d4ed8 100%)',
  },
  {
    value: 'ember',
    label: 'Ember',
    description: 'Copper-orange highlights with a more energetic operations tone.',
    preview: 'linear-gradient(135deg, #fb923c 0%, #b45309 100%)',
  },
  {
    value: 'rose',
    label: 'Rose',
    description: 'Soft magenta accents for a warmer, more editorial presentation.',
    preview: 'linear-gradient(135deg, #f472b6 0%, #be185d 100%)',
  },
  {
    value: 'slate',
    label: 'Slate',
    description: 'Subdued steel-blue accents for a quieter, more minimal workspace.',
    preview: 'linear-gradient(135deg, #94a3b8 0%, #334155 100%)',
  },
];

const MODEL_PROVIDER_TAGS = [
  'All',
  'Llama',
  'Qwen',
  'DeepSeek',
  'Mistral',
  'Gemma',
  'Phi',
  'Code',
  'Reasoning',
  'Vision',
  'Embedding',
];

const OLLAMA_MODEL_CATALOG: CatalogModel[] = [
  {
    name: 'llama3.2:1b',
    provider: 'Llama',
    tags: ['General'],
    description: 'Lightweight Llama 3.2 model for very fast local responses.',
  },
  {
    name: 'llama3.2:3b',
    provider: 'Llama',
    tags: ['General'],
    description: 'Fast general-purpose local model for everyday chat.',
  },
  {
    name: 'llama3.1:8b',
    provider: 'Llama',
    tags: ['General'],
    description: 'Balanced Llama 3.1 model for local chat and analysis.',
  },
  {
    name: 'llama3.1:70b',
    provider: 'Llama',
    tags: ['General'],
    description: 'Large Llama 3.1 model for stronger reasoning on high-memory hardware.',
  },
  {
    name: 'llama3.1:405b',
    provider: 'Llama',
    tags: ['General'],
    description: 'Frontier-scale Llama 3.1 model for very large local or hosted setups.',
  },
  {
    name: 'llama3:8b',
    provider: 'Llama',
    tags: ['General'],
    description: 'Llama 3 general model with broad local support.',
  },
  {
    name: 'llama3:70b',
    provider: 'Llama',
    tags: ['General'],
    description: 'Large Llama 3 model for stronger generation quality.',
  },
  {
    name: 'codellama:7b',
    provider: 'Llama',
    tags: ['Code'],
    description: 'Code-focused Llama model for lightweight programming assistance.',
  },
  {
    name: 'codellama:13b',
    provider: 'Llama',
    tags: ['Code'],
    description: 'Mid-size Code Llama model for code understanding and generation.',
  },
  {
    name: 'codellama:34b',
    provider: 'Llama',
    tags: ['Code'],
    description: 'Larger Code Llama model for deeper codebase work.',
  },
  {
    name: 'codellama:70b',
    provider: 'Llama',
    tags: ['Code'],
    description: 'Large Code Llama model for high-quality coding workflows.',
  },
  {
    name: 'llava:7b',
    provider: 'Llama',
    tags: ['Vision'],
    description: 'Vision-capable model for image-aware local workflows.',
  },
  {
    name: 'llava:13b',
    provider: 'Llama',
    tags: ['Vision'],
    description: 'Larger LLaVA model for image-aware local workflows.',
  },
  {
    name: 'qwen3:0.6b',
    provider: 'Qwen',
    tags: ['General'],
    description: 'Very small Qwen 3 model for quick local responses.',
  },
  {
    name: 'qwen3:1.7b',
    provider: 'Qwen',
    tags: ['General'],
    description: 'Compact Qwen 3 model for low-resource local use.',
  },
  {
    name: 'qwen3:4b',
    provider: 'Qwen',
    tags: ['General'],
    description: 'Balanced compact Qwen 3 model.',
  },
  {
    name: 'qwen3:8b',
    provider: 'Qwen',
    tags: ['General'],
    description: 'General Qwen 3 model with stronger local reasoning.',
  },
  {
    name: 'qwen3:14b',
    provider: 'Qwen',
    tags: ['General'],
    description: 'Mid-size Qwen 3 model for higher-quality responses.',
  },
  {
    name: 'qwen3:30b',
    provider: 'Qwen',
    tags: ['General'],
    description: 'Large Qwen 3 model for capable local reasoning.',
  },
  {
    name: 'qwen3:32b',
    provider: 'Qwen',
    tags: ['General'],
    description: 'Large Qwen 3 model for advanced local workloads.',
  },
  {
    name: 'qwen3:235b',
    provider: 'Qwen',
    tags: ['General'],
    description: 'Very large Qwen 3 model for high-memory environments.',
  },
  {
    name: 'qwen2.5:0.5b',
    provider: 'Qwen',
    tags: ['General'],
    description: 'Tiny Qwen 2.5 model for very low-resource devices.',
  },
  {
    name: 'qwen2.5:1.5b',
    provider: 'Qwen',
    tags: ['General'],
    description: 'Small Qwen 2.5 model for fast local use.',
  },
  {
    name: 'qwen2.5:3b',
    provider: 'Qwen',
    tags: ['General'],
    description: 'Compact multilingual reasoning model.',
  },
  {
    name: 'qwen2.5:7b',
    provider: 'Qwen',
    tags: ['General'],
    description: 'Balanced reasoning and writing model.',
  },
  {
    name: 'qwen2.5:14b',
    provider: 'Qwen',
    tags: ['General'],
    description: 'Mid-size Qwen 2.5 model for stronger multilingual reasoning.',
  },
  {
    name: 'qwen2.5:32b',
    provider: 'Qwen',
    tags: ['General'],
    description: 'Large Qwen 2.5 model for advanced local workloads.',
  },
  {
    name: 'qwen2.5:72b',
    provider: 'Qwen',
    tags: ['General'],
    description: 'Large Qwen 2.5 model for high-memory machines.',
  },
  {
    name: 'qwen2.5-coder:0.5b',
    provider: 'Qwen',
    tags: ['Code'],
    description: 'Tiny Qwen coder model for lightweight coding assistance.',
  },
  {
    name: 'qwen2.5-coder:1.5b',
    provider: 'Qwen',
    tags: ['Code'],
    description: 'Small Qwen coder model for fast code tasks.',
  },
  {
    name: 'qwen2.5-coder:3b',
    provider: 'Qwen',
    tags: ['Code'],
    description: 'Compact coding model for local development workflows.',
  },
  {
    name: 'qwen2.5-coder:7b',
    provider: 'Qwen',
    tags: ['Code'],
    description: 'Coding-focused model for project and patch workflows.',
  },
  {
    name: 'qwen2.5-coder:14b',
    provider: 'Qwen',
    tags: ['Code'],
    description: 'Mid-size Qwen coder model for stronger code generation.',
  },
  {
    name: 'qwen2.5-coder:32b',
    provider: 'Qwen',
    tags: ['Code'],
    description: 'Large Qwen coder model for advanced coding tasks.',
  },
  {
    name: 'qwen2.5vl:3b',
    provider: 'Qwen',
    tags: ['Vision'],
    description: 'Compact Qwen vision-language model.',
  },
  {
    name: 'qwen2.5vl:7b',
    provider: 'Qwen',
    tags: ['Vision'],
    description: 'Balanced Qwen vision-language model.',
  },
  {
    name: 'qwen2.5vl:32b',
    provider: 'Qwen',
    tags: ['Vision'],
    description: 'Large Qwen vision-language model.',
  },
  {
    name: 'qwen2.5vl:72b',
    provider: 'Qwen',
    tags: ['Vision'],
    description: 'Very large Qwen vision-language model.',
  },
  {
    name: 'deepseek-r1:1.5b',
    provider: 'DeepSeek',
    tags: ['Reasoning'],
    description: 'Small DeepSeek R1 reasoning model.',
  },
  {
    name: 'deepseek-r1:7b',
    provider: 'DeepSeek',
    tags: ['Reasoning'],
    description: 'Reasoning-oriented model for harder analytical prompts.',
  },
  {
    name: 'deepseek-r1:8b',
    provider: 'DeepSeek',
    tags: ['Reasoning'],
    description: 'DeepSeek R1 distilled reasoning model.',
  },
  {
    name: 'deepseek-r1:14b',
    provider: 'DeepSeek',
    tags: ['Reasoning'],
    description: 'Mid-size DeepSeek R1 reasoning model.',
  },
  {
    name: 'deepseek-r1:32b',
    provider: 'DeepSeek',
    tags: ['Reasoning'],
    description: 'Large DeepSeek R1 reasoning model.',
  },
  {
    name: 'deepseek-r1:70b',
    provider: 'DeepSeek',
    tags: ['Reasoning'],
    description: 'Large DeepSeek R1 model for high-memory reasoning workloads.',
  },
  {
    name: 'deepseek-r1:671b',
    provider: 'DeepSeek',
    tags: ['Reasoning'],
    description: 'Very large DeepSeek R1 model for specialized high-memory setups.',
  },
  {
    name: 'deepseek-coder:1.3b',
    provider: 'DeepSeek',
    tags: ['Code'],
    description: 'Small DeepSeek coder model.',
  },
  {
    name: 'deepseek-coder:6.7b',
    provider: 'DeepSeek',
    tags: ['Code'],
    description: 'Balanced DeepSeek coder model.',
  },
  {
    name: 'deepseek-coder:33b',
    provider: 'DeepSeek',
    tags: ['Code'],
    description: 'Large DeepSeek coder model.',
  },
  {
    name: 'deepseek-coder-v2:16b',
    provider: 'DeepSeek',
    tags: ['Code'],
    description: 'Larger coding model for codebase questions.',
  },
  {
    name: 'deepseek-coder-v2:236b',
    provider: 'DeepSeek',
    tags: ['Code'],
    description: 'Very large DeepSeek coder model for high-memory setups.',
  },
  {
    name: 'deepseek-v2:16b',
    provider: 'DeepSeek',
    tags: ['General'],
    description: 'DeepSeek V2 general-purpose model.',
  },
  {
    name: 'deepseek-v2:236b',
    provider: 'DeepSeek',
    tags: ['General'],
    description: 'Very large DeepSeek V2 model.',
  },
  {
    name: 'mistral:7b',
    provider: 'Mistral',
    tags: ['General'],
    description: 'Efficient general-purpose model with concise outputs.',
  },
  {
    name: 'mistral-nemo:12b',
    provider: 'Mistral',
    tags: ['General'],
    description: 'Mistral Nemo model for multilingual local workloads.',
  },
  {
    name: 'mixtral:8x7b',
    provider: 'Mistral',
    tags: ['General'],
    description: 'Mixture-of-experts Mistral model for stronger generation.',
  },
  {
    name: 'mixtral:8x22b',
    provider: 'Mistral',
    tags: ['General'],
    description: 'Large Mixtral mixture-of-experts model.',
  },
  {
    name: 'codestral:22b',
    provider: 'Mistral',
    tags: ['Code'],
    description: 'Mistral coding model for software development tasks.',
  },
  {
    name: 'gemma3:1b',
    provider: 'Gemma',
    tags: ['General'],
    description: 'Very lightweight Gemma 3 model.',
  },
  {
    name: 'gemma3:4b',
    provider: 'Gemma',
    tags: ['General'],
    description: 'Compact Gemma 3 model.',
  },
  {
    name: 'gemma3:12b',
    provider: 'Gemma',
    tags: ['General'],
    description: 'Mid-size Gemma 3 model.',
  },
  {
    name: 'gemma3:27b',
    provider: 'Gemma',
    tags: ['General'],
    description: 'Large Gemma 3 model.',
  },
  {
    name: 'gemma2:2b',
    provider: 'Gemma',
    tags: ['General'],
    description: 'Very lightweight model for quick local responses.',
  },
  {
    name: 'gemma2:9b',
    provider: 'Gemma',
    tags: ['General'],
    description: 'Higher-quality Gemma model for writing and reasoning.',
  },
  {
    name: 'gemma2:27b',
    provider: 'Gemma',
    tags: ['General'],
    description: 'Large Gemma 2 model for higher-quality responses.',
  },
  {
    name: 'codegemma:2b',
    provider: 'Gemma',
    tags: ['Code'],
    description: 'Small Gemma coding model.',
  },
  {
    name: 'codegemma:7b',
    provider: 'Gemma',
    tags: ['Code'],
    description: 'Gemma coding model for local development tasks.',
  },
  {
    name: 'phi3:mini',
    provider: 'Phi',
    tags: ['General'],
    description: 'Small model for low-resource devices.',
  },
  {
    name: 'phi3:medium',
    provider: 'Phi',
    tags: ['General'],
    description: 'Mid-size Phi 3 model.',
  },
  {
    name: 'phi4',
    provider: 'Phi',
    tags: ['General'],
    description: 'Phi 4 model for capable local reasoning and writing.',
  },
  {
    name: 'phi4-mini',
    provider: 'Phi',
    tags: ['General'],
    description: 'Compact Phi 4 model for efficient local use.',
  },
  {
    name: 'nomic-embed-text',
    provider: 'Embedding',
    tags: ['Embedding'],
    description: 'Text embedding model for retrieval and semantic search workflows.',
  },
  {
    name: 'mxbai-embed-large',
    provider: 'Embedding',
    tags: ['Embedding'],
    description: 'Large embedding model for semantic retrieval.',
  },
];

const EMPTY_CONTEXT_USAGE: ContextUsage = {
  provider: '',
  model: '',
  used_tokens: 0,
  context_window: 0,
  usage_source: 'not-loaded',
};

function normalizeContextUsage(data: Partial<ContextUsage>): ContextUsage {
  return {
    provider: String(data.provider ?? ''),
    model: String(data.model ?? ''),
    used_tokens: Math.max(0, Math.round(Number(data.used_tokens ?? 0))),
    context_window: Math.max(0, Math.round(Number(data.context_window ?? 0))),
    usage_source: String(data.usage_source ?? ''),
  };
}

async function fetchContextUsage(sessionId: string | null): Promise<ContextUsage> {
  const params = new URLSearchParams();

  if (sessionId) {
    params.set('session_id', sessionId);
  }

  const query = params.toString();
  const suffix = query ? `?${query}` : '';
  const urls = [`${API_BASE}/context/usage${suffix}`, `/context/usage${suffix}`];
  let lastError: Error | null = null;

  for (const url of urls) {
    try {
      const response = await fetch(url);
      if (!response.ok) {
        throw new Error(`Engine returned HTTP ${response.status} while loading context usage.`);
      }

      return normalizeContextUsage((await response.json()) as Partial<ContextUsage>);
    } catch (error) {
      lastError = error instanceof Error ? error : new Error('Could not load context usage.');
    }
  }

  throw lastError ?? new Error('Could not load context usage.');
}

function formatTokenMeter(usage: ContextUsage) {
  if (usage.usage_source === 'unavailable') {
    return 'Tokens unavailable';
  }

  if (usage.context_window <= 0) {
    return 'Loading tokens...';
  }

  const used = usage.used_tokens.toLocaleString();
  const limit = usage.context_window.toLocaleString();

  return `${used} / ${limit}`;
}

function ThinkingIndicator({ isDark }: { isDark: boolean }) {
  return (
    <div
      className={`flex items-center gap-2 text-xs font-medium ${isDark ? 'text-zinc-400' : 'text-slate-500'
        }`}
    >
      <span>Thinking</span>
      <span className="flex items-center gap-1" aria-hidden="true">
        <span className="thinking-dot" />
        <span className="thinking-dot thinking-dot-delay-1" />
        <span className="thinking-dot thinking-dot-delay-2" />
      </span>
    </div>
  );
}

function turnsToMessages(turns: EngineTurn[], sessionId: string): Message[] {
  return turns.flatMap((turn, turnIdx) => {
    const assistantIdx = turnIdx * 2 + 1;
    let sources: RetrievalChunk[] | undefined = undefined;
    const saved = localStorage.getItem(`aegis-sources-${sessionId}-${assistantIdx}`);
    if (saved) {
      try {
        sources = JSON.parse(saved);
      } catch (e) {
        console.error('Failed to parse saved sources:', e);
      }
    }

    return [
      { role: 'user' as const, content: turn.query, edited: turn.edited, timestamp: turn.created_at },
      { role: 'assistant' as const, content: turn.response, timestamp: turn.created_at, sources },
    ];
  });
}

function cleanOutlookCalendarName(name: string) {
  return name.replace(/\s*\(this computer only\)\s*/gi, ' ').replace(/\s+/g, ' ').trim();
}

function isGenericOutlookDataFileCalendar(calendar: OutlookCalendar) {
  const calendarName = cleanOutlookCalendarName(calendar.name).toLowerCase();
  const storeName = calendar.store_name.trim().toLowerCase();
  const hasEmail = Boolean(calendar.email_address?.trim());

  return !hasEmail && calendarName === 'calendar' && storeName.includes('outlook data file');
}

function isVisibleOutlookCalendar(calendar: OutlookCalendar) {
  return !isGenericOutlookDataFileCalendar(calendar);
}

function outlookCalendarLabel(calendar: OutlookCalendar) {
  const calendarName = cleanOutlookCalendarName(calendar.name);
  const emailAddress = calendar.email_address?.trim();
  const storeName = calendar.store_name.trim();

  if (emailAddress) {
    return `${calendarName} (${emailAddress})`;
  }

  if (storeName && !storeName.toLowerCase().includes('outlook data file')) {
    return `${calendarName} (${storeName})`;
  }

  return calendarName;
}

function wrapPdfLine(line: string, maxLength: number) {
  const words = line.replace(/\r/g, '').split(/\s+/);
  const wrapped: string[] = [];
  let current = '';

  for (const word of words) {
    if (!word) {
      continue;
    }

    if (word.length > maxLength) {
      if (current) {
        wrapped.push(current);
        current = '';
      }
      for (let index = 0; index < word.length; index += maxLength) {
        wrapped.push(word.slice(index, index + maxLength));
      }
      continue;
    }

    const next = current ? `${current} ${word}` : word;
    if (next.length > maxLength) {
      wrapped.push(current);
      current = word;
    } else {
      current = next;
    }
  }

  if (current) {
    wrapped.push(current);
  }

  return wrapped.length > 0 ? wrapped : [''];
}

function escapePdfText(text: string) {
  return text
    .replace(/[^\x09\x0A\x0D\x20-\x7E]/g, '?')
    .replace(/\\/g, '\\\\')
    .replace(/\(/g, '\\(')
    .replace(/\)/g, '\\)');
}

function formatExportTimestamp(timestamp?: string) {
  if (!timestamp) {
    return 'time not recorded';
  }

  const date = new Date(timestamp);
  if (Number.isNaN(date.getTime())) {
    return timestamp;
  }

  return new Intl.DateTimeFormat(undefined, {
    year: 'numeric',
    month: 'short',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  }).format(date);
}

function speakerLabel(role: Role) {
  return role === 'user' ? 'User' : 'AEGIS';
}

const VOICE_LOW_RAM_MODE_STORAGE_KEY = 'aegis-voice-low-ram-mode';

function loadStoredVoiceLowRamMode(): boolean {
  if (typeof window === 'undefined') {
    return false;
  }
  try {
    const stored = window.localStorage.getItem(VOICE_LOW_RAM_MODE_STORAGE_KEY);
    return stored ? JSON.parse(stored) === true : false;
  } catch {
    return false;
  }
}

const VOICE_TTS_ENABLED_STORAGE_KEY = 'aegis-voice-tts-enabled';

function loadStoredTtsEnabled(): boolean {
  if (typeof window === 'undefined') {
    return false;
  }
  try {
    const stored = window.localStorage.getItem(VOICE_TTS_ENABLED_STORAGE_KEY);
    return stored ? JSON.parse(stored) === true : false;
  } catch {
    return false;
  }
}

const RAG_ENABLED_STORAGE_KEY = 'aegis-rag-enabled';
const RAG_TOP_K_STORAGE_KEY = 'aegis-rag-top-k';
const RAG_THRESHOLD_STORAGE_KEY = 'aegis-rag-threshold';

function loadStoredRagEnabled(): boolean {
  if (typeof window === 'undefined') return true;
  try {
    const stored = window.localStorage.getItem(RAG_ENABLED_STORAGE_KEY);
    return stored ? JSON.parse(stored) === true : true;
  } catch {
    return true;
  }
}

function loadStoredRagTopK(): number {
  if (typeof window === 'undefined') return 5;
  try {
    const stored = window.localStorage.getItem(RAG_TOP_K_STORAGE_KEY);
    return stored ? Math.max(1, Math.min(10, Number(JSON.parse(stored)))) : 5;
  } catch {
    return 5;
  }
}

function loadStoredRagThreshold(): number {
  if (typeof window === 'undefined') return 0.0;
  try {
    const stored = window.localStorage.getItem(RAG_THRESHOLD_STORAGE_KEY);
    return stored ? Math.max(0.0, Math.min(1.0, Number(JSON.parse(stored)))) : 0.0;
  } catch {
    return 0.0;
  }
}

function loadStoredIndexedDocumentsBySession() {
  if (typeof window === 'undefined') {
    return {};
  }

  try {
    const raw = window.localStorage.getItem(INDEXED_DOCUMENTS_STORAGE_KEY);
    if (!raw) {
      return {};
    }

    const parsed = JSON.parse(raw) as Record<string, IndexedDocument[]>;
    return parsed && typeof parsed === 'object' && !Array.isArray(parsed) ? parsed : {};
  } catch {
    return {};
  }
}

function loadStoredPinnedSessionIds() {
  if (typeof window === 'undefined') {
    return [];
  }

  try {
    const raw = window.localStorage.getItem(PINNED_SESSIONS_STORAGE_KEY);
    if (!raw) {
      return [];
    }

    const parsed = JSON.parse(raw) as unknown;
    return Array.isArray(parsed)
      ? parsed.filter((sessionId): sessionId is string => typeof sessionId === 'string')
      : [];
  } catch {
    return [];
  }
}

function loadStoredResponseStyle(): ResponseStyle {
  if (typeof window === 'undefined') {
    return 'default';
  }

  const storedStyle = window.localStorage.getItem(RESPONSE_STYLE_STORAGE_KEY);
  return RESPONSE_STYLE_OPTIONS.some((option) => option.value === storedStyle)
    ? (storedStyle as ResponseStyle)
    : 'default';
}

function loadStoredAppearanceTheme(): AppearanceTheme {
  if (typeof window === 'undefined') {
    return 'default';
  }

  const storedTheme = window.localStorage.getItem(APPEARANCE_THEME_STORAGE_KEY);
  return APPEARANCE_THEME_OPTIONS.some((option) => option.value === storedTheme)
    ? (storedTheme as AppearanceTheme)
    : 'default';
}

function parseWelcomeMessages(markdown: string) {
  const messages = markdown
    .split(/\r?\n/)
    .map((line) => line.trim().replace(/^[-*]\s+/, ''))
    .filter((line) => line && !line.startsWith('#'));

  return messages.length > 0 ? messages : DEFAULT_WELCOME_MESSAGES;
}

function randomWelcomeMessage(messages: string[]) {
  const index = Math.floor(Math.random() * Math.max(messages.length, 1));
  return messages[index] ?? DEFAULT_WELCOME_MESSAGES[0];
}

function profileDisplayName(profileText: string) {
  const match = profileText.match(/\b(?:my name is|name is|i am|i'm)\s+([A-Za-z][A-Za-z '-]{0,40})/i);
  const rawName = match?.[1]?.trim().replace(/[.!,;:].*$/, '');
  return rawName || 'there';
}

function personalizeWelcomeMessage(message: string, profileText: string) {
  return message.replace(/\[insert_name\]/gi, profileDisplayName(profileText));
}

function modelDownloadPercent(chunk: PullModelChunk) {
  if (chunk.total && chunk.total > 0 && typeof chunk.completed === 'number') {
    return Math.max(0, Math.min(100, Math.round((chunk.completed / chunk.total) * 100)));
  }

  if (chunk.status === 'success') {
    return 100;
  }

  return null;
}

function projectFileExtension(path: string) {
  const dotIndex = path.lastIndexOf('.');
  return dotIndex >= 0 ? path.slice(dotIndex).toLowerCase() : '';
}

function shouldReadProjectFile(path: string, size: number) {
  const fileName = path.split('/').pop() ?? path;
  return (
    size <= MAX_PROJECT_FILE_BYTES &&
    !IGNORED_PROJECT_FILES.has(fileName) &&
    CODE_PROJECT_EXTENSIONS.has(projectFileExtension(path))
  );
}

async function scanProjectDirectory(
  directoryHandle: FileSystemDirectoryHandle,
  prefix = '',
  files: ProjectFileSnapshot[] = [],
) {
  for await (const [name, handle] of directoryHandle.entries()) {
    if (files.length >= MAX_PROJECT_FILES) {
      break;
    }

    const path = prefix ? `${prefix}/${name}` : name;

    if (handle.kind === 'directory') {
      if (!IGNORED_PROJECT_DIRECTORIES.has(name)) {
        await scanProjectDirectory(handle, path, files);
      }
      continue;
    }

    const file = await handle.getFile();
    if (!shouldReadProjectFile(path, file.size)) {
      continue;
    }

    try {
      files.push({
        path,
        content: await file.text(),
        size: file.size,
        handle,
      });
    } catch {
      // Skip files the browser cannot decode as text.
    }
  }

  return files;
}

function buildProjectSnapshot(projectName: string, files: ProjectFileSnapshot[]) {
  const sortedFiles = [...files].sort((left, right) => left.path.localeCompare(right.path));
  const tableOfContents = sortedFiles.map((file) => `- ${file.path} (${file.size} bytes)`).join('\n');
  let snapshot = `PROJECT: ${projectName}\nFILES SCANNED: ${files.length}\n\nFILE TREE:\n${tableOfContents}\n`;

  for (const file of sortedFiles) {
    const next = `\n\n--- FILE: ${file.path} ---\n${file.content}`;
    if (snapshot.length + next.length > MAX_PROJECT_CONTEXT_CHARS) {
      snapshot += '\n\n[AEGIS truncated the project snapshot to fit the model context budget.]';
      break;
    }

    snapshot += next;
  }

  return snapshot;
}

function findProjectFile(project: CodeProject, path: string) {
  const normalizedPath = path.replace(/^[/\\]+/, '').replace(/\\/g, '/');
  return project.files.find((file) => file.path === normalizedPath);
}

function extractUnifiedDiff(content: string) {
  const fencedMatch = content.match(/```(?:diff|patch)?\s*\n([\s\S]*?^```)/m);
  const candidate = fencedMatch
    ? fencedMatch[1].replace(/\n```$/, '')
    : content.slice(content.indexOf('diff --git'));

  if (!candidate || !candidate.includes('--- ') || !candidate.includes('+++ ')) {
    return '';
  }

  return candidate.trim();
}

function parsePatchTarget(diff: string) {
  const plusLine = diff
    .split('\n')
    .find((line) => line.startsWith('+++ ') && !line.includes('/dev/null'));

  if (!plusLine) {
    return '';
  }

  return plusLine
    .replace(/^\+\+\+\s+/, '')
    .replace(/^[ab]\//, '')
    .trim();
}

function applySimpleUnifiedDiff(original: string, diff: string) {
  const lines = original.split('\n');
  const output: string[] = [];
  let sourceIndex = 0;
  const diffLines = diff.split(/\r?\n/);
  let index = 0;

  while (index < diffLines.length) {
    const line = diffLines[index];
    const hunkMatch = line.match(/^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/);
    if (!hunkMatch) {
      index += 1;
      continue;
    }

    const hunkStart = Math.max(0, Number(hunkMatch[1]) - 1);
    while (sourceIndex < hunkStart) {
      output.push(lines[sourceIndex] ?? '');
      sourceIndex += 1;
    }

    index += 1;
    while (index < diffLines.length && !diffLines[index].startsWith('@@ ')) {
      const hunkLine = diffLines[index];
      const marker = hunkLine[0];
      const value = hunkLine.slice(1);

      if (marker === ' ') {
        if ((lines[sourceIndex] ?? '') !== value) {
          throw new Error('Patch context did not match the current file contents.');
        }
        output.push(value);
        sourceIndex += 1;
      } else if (marker === '-') {
        if ((lines[sourceIndex] ?? '') !== value) {
          throw new Error('Patch removal did not match the current file contents.');
        }
        sourceIndex += 1;
      } else if (marker === '+') {
        output.push(value);
      }

      index += 1;
    }
  }

  while (sourceIndex < lines.length) {
    output.push(lines[sourceIndex] ?? '');
    sourceIndex += 1;
  }

  return output.join('\n');
}

function mergeIndexedDocuments(
  currentDocuments: IndexedDocument[],
  nextDocuments: IndexedDocument[],
) {
  const merged = new Map<string, IndexedDocument>();

  currentDocuments.forEach((document) => {
    merged.set(document.stored_path, document);
  });
  nextDocuments.forEach((document) => {
    merged.set(document.stored_path, document);
  });

  return Array.from(merged.values());
}

function normalizeAssistantMarkdownProse(content: string) {
  return content
    .replace(/\r\n/g, '\n')
    .replace(/\(([^()\n]+?)\s+[-*+]\s+([^()\n]+?)\)/g, '($1 and $2)')
    .replace(/(^|\n)\s{0,3}(#{1,6})([^\s#])/g, '$1$2 $3')
    .replace(/([:.!?])\s*(#{1,6}\s+[A-Za-z0-9])/g, '$1\n$2')
    .replace(/([:.!?])\s*(\d+\.\s+)/g, '$1\n$2')
    .replace(/([:.!?])\s*([*+-]\s+)/g, '$1\n$2')
    .replace(/([A-Za-z0-9)])\s+(\d+\.\s+)/g, '$1\n$2')
    .replace(/([^\n])(\d+\.\s+\*\*)/g, '$1\n$2')
    .replace(/\n{3,}/g, '\n\n');
}

function normalizeAssistantMarkdown(content: string) {
  return content
    .replace(/\r\n/g, '\n')
    .split(/(```[\s\S]*?```)/g)
    .map((segment) =>
      segment.startsWith('```') ? segment : normalizeAssistantMarkdownProse(segment),
    )
    .join('');
}

function extractSseEvents(buffer: string) {
  const events: string[] = [];
  let remaining = buffer;
  let boundary = remaining.match(/\r?\n\r?\n/);

  while (boundary?.index !== undefined) {
    events.push(remaining.slice(0, boundary.index));
    remaining = remaining.slice(boundary.index + boundary[0].length);
    boundary = remaining.match(/\r?\n\r?\n/);
  }

  return { events, remaining };
}

function sseEventData(event: string) {
  return event
    .split(/\r?\n/)
    .filter((line) => line.startsWith('data:'))
    .map((line) => line.replace(/^data: ?/, ''))
    .join('\n');
}

function splitAssistantStreamSegments(content: string) {
  const segments = content.match(/(\r?\n|[^\S\r\n]+|[^\s]+)/g);
  return segments && segments.length > 0 ? segments : [content];
}

function parseMarkdownBlocks(content: string): MarkdownBlock[] {
  const normalized = normalizeAssistantMarkdown(content);
  const lines = normalized.split('\n');
  const blocks: MarkdownBlock[] = [];
  let paragraph: string[] = [];
  let codeLines: string[] = [];
  let codeLanguage = '';
  let inCode = false;

  function flushParagraph() {
    if (paragraph.length === 0) {
      return;
    }

    blocks.push({ type: 'paragraph', text: paragraph.join(' ').trim() });
    paragraph = [];
  }

  function pushList(type: 'ordered' | 'unordered', firstItem: string, startIndex: number) {
    const items = [firstItem.trim()];
    let index = startIndex + 1;

    while (index < lines.length) {
      const line = lines[index].trim();
      const orderedMatch = line.match(/^\d+\.\s+(.*)$/);
      const unorderedMatch = line.match(/^[-*+]\s+(.*)$/);

      if (type === 'ordered' && orderedMatch) {
        items.push(orderedMatch[1].trim());
        index += 1;
        continue;
      }

      if (type === 'unordered' && unorderedMatch) {
        items.push(unorderedMatch[1].trim());
        index += 1;
        continue;
      }

      break;
    }

    blocks.push({ type, items });
    return index - 1;
  }

  for (let index = 0; index < lines.length; index += 1) {
    const rawLine = lines[index];
    const line = rawLine.trim();

    const fenceMatch = line.match(/^```([A-Za-z0-9_+.#-]*)/);
    if (fenceMatch) {
      if (inCode) {
        blocks.push({ type: 'code', text: codeLines.join('\n'), language: codeLanguage });
        codeLines = [];
        codeLanguage = '';
        inCode = false;
      } else {
        flushParagraph();
        inCode = true;
        codeLanguage = fenceMatch[1]?.trim().toLowerCase() || 'text';
      }
      continue;
    }

    if (inCode) {
      codeLines.push(rawLine);
      continue;
    }

    if (!line) {
      flushParagraph();
      continue;
    }

    const headingMatch = line.match(/^(#{1,6})\s+(.+)$/);
    if (headingMatch) {
      flushParagraph();
      blocks.push({
        type: 'heading',
        level: headingMatch[1].length as MarkdownHeadingLevel,
        text: headingMatch[2].trim(),
      });
      continue;
    }

    const orderedMatch = line.match(/^\d+\.\s+(.*)$/);
    if (orderedMatch) {
      flushParagraph();
      index = pushList('ordered', orderedMatch[1], index);
      continue;
    }

    const unorderedMatch = line.match(/^[-*+]\s+(.*)$/);
    if (unorderedMatch) {
      flushParagraph();
      index = pushList('unordered', unorderedMatch[1], index);
      continue;
    }

    paragraph.push(line);
  }

  if (inCode && codeLines.length > 0) {
    blocks.push({ type: 'code', text: codeLines.join('\n'), language: codeLanguage });
  }
  flushParagraph();

  return blocks.length > 0 ? blocks : [{ type: 'paragraph', text: content }];
}

function renderInlineMarkdown(text: string) {
  const parts: ReactNode[] = [];
  const pattern = /(`[^`]+`|\*\*[^*]+\*\*|\*[^*\s][^*]*\*)/g;
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = pattern.exec(text)) !== null) {
    if (match.index > lastIndex) {
      parts.push(text.slice(lastIndex, match.index));
    }

    const value = match[0];
    if (value.startsWith('`')) {
      parts.push(
        <code
          className="rounded bg-black/15 px-1.5 py-0.5 font-mono text-[0.92em] text-emerald-500"
          key={`${match.index}-code`}
        >
          {value.slice(1, -1)}
        </code>,
      );
    } else if (value.startsWith('**')) {
      parts.push(
        <strong className="font-semibold" key={`${match.index}-strong`}>
          {value.slice(2, -2)}
        </strong>,
      );
    } else {
      parts.push(
        <em className="italic" key={`${match.index}-em`}>
          {value.slice(1, -1)}
        </em>,
      );
    }

    lastIndex = match.index + value.length;
  }

  if (lastIndex < text.length) {
    parts.push(text.slice(lastIndex));
  }

  return parts;
}

const CODE_KEYWORDS = new Set([
  'as',
  'async',
  'await',
  'break',
  'case',
  'catch',
  'class',
  'const',
  'continue',
  'def',
  'else',
  'enum',
  'export',
  'extends',
  'false',
  'fn',
  'for',
  'from',
  'function',
  'if',
  'impl',
  'import',
  'in',
  'interface',
  'let',
  'match',
  'mod',
  'mut',
  'new',
  'none',
  'null',
  'ok',
  'pub',
  'return',
  'self',
  'some',
  'struct',
  'switch',
  'this',
  'throw',
  'true',
  'try',
  'type',
  'use',
  'var',
  'while',
  'with',
]);

const CODE_TYPES = new Set([
  'bool',
  'dict',
  'error',
  'i32',
  'i64',
  'number',
  'object',
  'result',
  'str',
  'string',
  'u32',
  'u64',
  'vec',
  'void',
]);

const CODE_TOKEN_PATTERN =
  /(\/\/.*|#.*|\/\*.*?\*\/|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|`(?:\\.|[^`\\])*`|\b\d+(?:\.\d+)?\b|\b[A-Za-z_][A-Za-z0-9_]*\b|[{}()[\].,;:+\-*/%=<>!&|?]+)/g;

function normalizedCodeLanguage(language: string) {
  const label = language.trim().toLowerCase();

  if (!label || label === 'text' || label === 'txt') {
    return 'code';
  }

  if (label === 'ts') {
    return 'typescript';
  }

  if (label === 'js') {
    return 'javascript';
  }

  if (label === 'py') {
    return 'python';
  }

  if (label === 'rs') {
    return 'rust';
  }

  return label;
}

function codeTokenClass(token: string) {
  const lowerToken = token.toLowerCase();

  if (token.startsWith('//') || token.startsWith('#') || token.startsWith('/*')) {
    return 'text-emerald-400/80 italic';
  }

  if (token.startsWith('"') || token.startsWith("'") || token.startsWith('`')) {
    return 'text-amber-300';
  }

  if (/^\d/.test(token)) {
    return 'text-cyan-300';
  }

  if (CODE_KEYWORDS.has(lowerToken)) {
    return 'text-sky-300';
  }

  if (CODE_TYPES.has(lowerToken) || /^[A-Z][A-Za-z0-9_]*$/.test(token)) {
    return 'text-violet-300';
  }

  if (/^[{}()[\].,;:+\-*/%=<>!&|?]+$/.test(token)) {
    return 'text-zinc-400';
  }

  return 'text-zinc-100';
}

function renderHighlightedCodeLine(line: string, lineIndex: number) {
  const parts: ReactNode[] = [];
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  CODE_TOKEN_PATTERN.lastIndex = 0;
  while ((match = CODE_TOKEN_PATTERN.exec(line)) !== null) {
    if (match.index > lastIndex) {
      parts.push(line.slice(lastIndex, match.index));
    }

    const token = match[0];
    parts.push(
      <span className={codeTokenClass(token)} key={`${lineIndex}-${match.index}`}>
        {token}
      </span>,
    );
    lastIndex = match.index + token.length;
  }

  if (lastIndex < line.length) {
    parts.push(line.slice(lastIndex));
  }

  return parts.length > 0 ? parts : '\u00A0';
}

function sanitizeTextForTts(rawText: string): string {
  // 1. Remove code blocks entirely (```...```)
  let cleanText = rawText.replace(/```[\s\S]*?```/g, '');
  
  // 2. Remove inline code (`code`)
  cleanText = cleanText.replace(/`([^`]+)`/g, '$1');
  
  // 3. Remove other markdown structures (bold, italic, headers, bullet symbols)
  cleanText = cleanText.replace(/[*#_~>+\-]/g, '');
  
  // 4. Remove excessive spacing or newlines
  cleanText = cleanText.replace(/\s+/g, ' ').trim();
  
  return cleanText;
}

async function copyTextToClipboard(text: string) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const textarea = document.createElement('textarea');
    textarea.value = text;
    textarea.setAttribute('readonly', 'true');
    textarea.style.position = 'fixed';
    textarea.style.left = '-9999px';
    document.body.appendChild(textarea);
    textarea.select();
    document.execCommand('copy');
    document.body.removeChild(textarea);
  }
}

function fitTextareaToContent(textarea: HTMLTextAreaElement) {
  textarea.style.height = '0px';
  textarea.style.height = `${Math.min(textarea.scrollHeight, 224)}px`;
}

function isFatalUiError(message: string) {
  const normalized = message.toLowerCase();
  return normalized.includes('fatal') || normalized.includes('unrecoverable');
}

function CodeBlock({ language, text }: { language: string; text: string }) {
  const [copied, setCopied] = useState(false);
  const languageLabel = normalizedCodeLanguage(language);
  const lines = text.split('\n');

  async function copyCode() {
    await copyTextToClipboard(text);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1400);
  }

  return (
    <div className="group max-w-[42rem] overflow-hidden rounded-lg border border-zinc-800 bg-zinc-950 shadow-md shadow-white/5">
      <div className="flex items-center justify-between gap-3 px-3 pt-2.5">
        <span className="truncate font-mono text-[11px] uppercase tracking-wide text-zinc-500">
          {languageLabel}
        </span>
        <button
          className="inline-flex items-center gap-1.5 rounded-md border border-zinc-700 bg-zinc-950/80 px-1.5 py-0.5 text-[11px] font-medium text-zinc-300 transition hover:border-emerald-500/70 hover:bg-emerald-500/10 hover:text-emerald-200"
          onClick={copyCode}
          type="button"
        >
          {copied ? <Check size={13} /> : <Copy size={13} />}
          {copied ? 'Copied' : 'Copy'}
        </button>
      </div>
      <pre className="overflow-x-auto px-3 pb-3 pt-2 text-left font-mono text-[12px] leading-5">
        <code>
          {lines.map((line, lineIndex) => (
            <span className="block whitespace-pre" key={`${lineIndex}-${line}`}>
              {renderHighlightedCodeLine(line, lineIndex)}
            </span>
          ))}
        </code>
      </pre>
    </div>
  );
}

function MarkdownHeading({ level, text }: { level: MarkdownHeadingLevel; text: string }) {
  const className =
    level === 1
      ? 'mt-1 text-[1.08rem] font-normal leading-7 tracking-[-0.01em] first:mt-0'
      : level === 2
        ? 'mt-3 text-[1.02rem] font-normal leading-7 tracking-[-0.01em] first:mt-0'
        : 'mt-3 text-[0.96rem] font-normal leading-6 tracking-[-0.005em] first:mt-0';

  if (level === 1) {
    return <h3 className={className}>{renderInlineMarkdown(text)}</h3>;
  }

  if (level === 2) {
    return <h4 className={className}>{renderInlineMarkdown(text)}</h4>;
  }

  return <h5 className={className}>{renderInlineMarkdown(text)}</h5>;
}

function AssistantMarkdown({ content }: { content: string }) {
  const blocks = parseMarkdownBlocks(content || '...');

  return (
    <div className="space-y-3">
      {blocks.map((block, blockIndex) => {
        if (block.type === 'heading') {
          return (
            <MarkdownHeading
              key={`heading-${blockIndex}`}
              level={block.level}
              text={block.text}
            />
          );
        }

        if (block.type === 'ordered') {
          return (
            <ol className="list-decimal space-y-1 pl-5" key={`ol-${blockIndex}`}>
              {block.items.map((item, itemIndex) => (
                <li key={`${blockIndex}-${itemIndex}`}>{renderInlineMarkdown(item)}</li>
              ))}
            </ol>
          );
        }

        if (block.type === 'unordered') {
          return (
            <ul className="list-disc space-y-1 pl-5" key={`ul-${blockIndex}`}>
              {block.items.map((item, itemIndex) => (
                <li key={`${blockIndex}-${itemIndex}`}>{renderInlineMarkdown(item)}</li>
              ))}
            </ul>
          );
        }

        if (block.type === 'code') {
          return <CodeBlock key={`code-${blockIndex}`} language={block.language} text={block.text} />;
        }

        return <p key={`p-${blockIndex}`}>{renderInlineMarkdown(block.text)}</p>;
      })}
    </div>
  );
}

function importPhaseLabel(phase: ImportPhase, fileLabel: string) {
  const target = fileLabel ? ` ${fileLabel}` : ' document';

  switch (phase) {
    case 'uploading':
      return `Uploading${target}`;
    case 'indexing':
      return `Reading and indexing${target}`;
    case 'complete':
      return `Finished indexing${target}`;
    case 'error':
      return `Import failed${target}`;
    case 'idle':
    default:
      return 'Ready to import';
  }
}

function createConversationPdf(options: {
  title: string;
  sessionId?: string | null;
  messages: Message[];
  indexedDocuments: IndexedDocument[];
}) {
  const pageWidth = 595;
  const pageHeight = 842;
  const margin = 48;
  const lineHeight = 15;
  const maxLinesPerPage = Math.floor((pageHeight - margin * 2) / lineHeight);
  const maxCharsPerLine = 88;
  const pages: string[][] = [[]];
  const exportedAt = new Date().toISOString();

  function addLine(line: string) {
    const page = pages[pages.length - 1];
    if (page.length >= maxLinesPerPage) {
      pages.push([]);
    }
    pages[pages.length - 1].push(line);
  }

  addLine('AEGIS Chat Transcript');
  addLine('');
  addLine(`Conversation: ${options.title}`);
  if (options.sessionId) {
    addLine(`Session ID: ${options.sessionId}`);
  }
  addLine(`Exported: ${formatExportTimestamp(exportedAt)}`);
  addLine(`Messages: ${options.messages.length}`);
  if (options.indexedDocuments.length > 0) {
    addLine('');
    addLine('Documents Added');
    options.indexedDocuments.forEach((document) => {
      const chunkLabel = document.chunks_added === 1 ? 'chunk' : 'chunks';
      addLine(`- User added document: ${document.file_name} (${document.chunks_added} ${chunkLabel})`);
    });
  }
  addLine('Format: speaker label, timestamp, message body');
  addLine('------------------------------------------------------------');
  addLine('');

  options.messages.forEach((message) => {
    const label = `${speakerLabel(message.role)} | ${formatExportTimestamp(message.timestamp)}${message.edited ? ' | edited' : ''
      }`;
    addLine(label);
    message.content.split('\n').forEach((line) => {
      wrapPdfLine(line, maxCharsPerLine).forEach((wrappedLine) => addLine(`  ${wrappedLine}`));
    });
    addLine('');
  });

  const objects: string[] = [''];
  const fontObjectNumber = 3 + pages.length * 2;
  const kids: string[] = [];

  objects[1] = '<< /Type /Catalog /Pages 2 0 R >>';

  pages.forEach((pageLines, pageIndex) => {
    const pageObjectNumber = 3 + pageIndex * 2;
    const contentObjectNumber = pageObjectNumber + 1;
    kids.push(`${pageObjectNumber} 0 R`);

    const stream = pageLines
      .map((line, lineIndex) => {
        const y = pageHeight - margin - lineIndex * lineHeight;
        return `BT /F1 10 Tf 1 0 0 1 ${margin} ${y} Tm (${escapePdfText(line)}) Tj ET`;
      })
      .join('\n');

    objects[pageObjectNumber] =
      `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ${pageWidth} ${pageHeight}] /Resources << /Font << /F1 ${fontObjectNumber} 0 R >> >> /Contents ${contentObjectNumber} 0 R >>`;
    objects[contentObjectNumber] = `<< /Length ${stream.length} >>\nstream\n${stream}\nendstream`;
  });

  objects[2] = `<< /Type /Pages /Kids [${kids.join(' ')}] /Count ${pages.length} >>`;
  objects[fontObjectNumber] = '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>';

  let pdf = '%PDF-1.4\n';
  const offsets = [0];
  for (let index = 1; index < objects.length; index += 1) {
    offsets[index] = pdf.length;
    pdf += `${index} 0 obj\n${objects[index]}\nendobj\n`;
  }

  const xrefOffset = pdf.length;
  pdf += `xref\n0 ${objects.length}\n0000000000 65535 f \n`;
  for (let index = 1; index < objects.length; index += 1) {
    pdf += `${offsets[index].toString().padStart(10, '0')} 00000 n \n`;
  }
  pdf += `trailer\n<< /Size ${objects.length} /Root 1 0 R >>\nstartxref\n${xrefOffset}\n%%EOF`;

  return new Blob([pdf], { type: 'application/pdf' });
}

function safeExportFileName(title: string) {
  return title
    .trim()
    .replace(/[\\/:*?"<>|]+/g, '-')
    .replace(/\s+/g, '-')
    .toLowerCase();
}

function sessionUpdatedAtMs(session: EngineSessionSummary) {
  const timestamp = Date.parse(session.updated_at);
  return Number.isNaN(timestamp) ? 0 : timestamp;
}

function formatSessionLastAccessed(updatedAt: string) {
  const timestamp = Date.parse(updatedAt);

  if (Number.isNaN(timestamp)) {
    return 'Unavailable';
  }

  const formattedDate = new Intl.DateTimeFormat(undefined, {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
  }).format(new Date(timestamp));

  return formattedDate;
}

function downloadConversationPdf(options: {
  title: string;
  sessionId?: string | null;
  messages: Message[];
  indexedDocuments: IndexedDocument[];
}) {
  const blob = createConversationPdf(options);
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  const sessionFileName = options.sessionId?.trim();
  const safeTitle = safeExportFileName(options.title);

  anchor.href = url;
  anchor.download = `${sessionFileName || safeTitle || 'aegis-chat'}.pdf`;
  anchor.click();
  URL.revokeObjectURL(url);
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
  const [appearanceTheme, setAppearanceTheme] = useState<AppearanceTheme>(
    loadStoredAppearanceTheme,
  );
  const [isStreaming, setIsStreaming] = useState(false);
  const [isUploading, setIsUploading] = useState(false);
  const [isClearingIndexedDocuments, setIsClearingIndexedDocuments] = useState(false);
  const [documentContextNotice, setDocumentContextNotice] = useState<string | null>(null);
  const [importProgress, setImportProgress] = useState(0);
  const [importPhase, setImportPhase] = useState<ImportPhase>('idle');
  const [importFileLabel, setImportFileLabel] = useState('');
  const [indexedDocumentsBySession, setIndexedDocumentsBySession] = useState<
    Record<string, IndexedDocument[]>
  >(
    loadStoredIndexedDocumentsBySession,
  );
  const [pinnedSessionIds, setPinnedSessionIds] = useState<string[]>(
    loadStoredPinnedSessionIds,
  );

  // VOICE STATE
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
  const [welcomeMessages, setWelcomeMessages] = useState(DEFAULT_WELCOME_MESSAGES);
  const [activeWelcomeMessage, setActiveWelcomeMessage] = useState(() =>
    randomWelcomeMessage(DEFAULT_WELCOME_MESSAGES),
  );
  const [sessionPendingDeletion, setSessionPendingDeletion] =
    useState<EngineSessionSummary | null>(null);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [calendarOpen, setCalendarOpen] = useState(false);
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
  const [streamingMessagesBySession, setStreamingMessagesBySession] = useState<
    Record<string, Message[]>
  >({});
  const [editingMessageIndex, setEditingMessageIndex] = useState<number | null>(null);
  const [editingMessageText, setEditingMessageText] = useState('');
  const [copiedMessageIndex, setCopiedMessageIndex] = useState<number | null>(null);
  const [editingSessionId, setEditingSessionId] = useState<string | null>(null);
  const [editingTitle, setEditingTitle] = useState('');
  const [deletingSessionIds, setDeletingSessionIds] = useState<string[]>([]);
  const [isMetricsOpen, setIsMetricsOpen] = useState(false);
  const [newSessionPulseId, setNewSessionPulseId] = useState<string | null>(null);
  const [inferenceStats, setInferenceStats] = useState({
    latency: 0,
    tps: 0,
    ttft: 0,
    ragTime: 0,
    similarity: 0,
    chunks: 0,
    backend: '---',
  });
  const inferenceStartTime = useRef<number | null>(null);
  const [sessionMenuOpenId, setSessionMenuOpenId] = useState<string | null>(null);
  const [chatMode, setChatMode] = useState<ChatMode>('general');
  const scrollRef = useRef<HTMLDivElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const profileImportInputRef = useRef<HTMLInputElement>(null);
  const composerTextareaRef = useRef<HTMLTextAreaElement>(null);
  const modelDownloadAbortRef = useRef<AbortController | null>(null);
  const modelDownloadAbortReasonRef = useRef<'pause' | 'cancel' | null>(null);
  const settingsCloseTimeoutRef = useRef<number | null>(null);
  const activeSessionIdRef = useRef<string | null>(activeSessionId);
  const streamingMessagesBySessionRef = useRef<Record<string, Message[]>>({});
  const isDark = theme === 'dark';
  const activeAppearanceTheme = useMemo(
    () =>
      APPEARANCE_THEME_OPTIONS.find((option) => option.value === appearanceTheme) ??
      APPEARANCE_THEME_OPTIONS[0],
    [appearanceTheme],
  );
  const resourceWarning =
    systemStats.cpu > 80 || systemStats.ram > 80
      ? `${[
        systemStats.cpu > 80 ? 'CPU' : null,
        systemStats.ram > 80 ? 'RAM' : null,
      ]
        .filter(Boolean)
        .join(' and ')} ${systemStats.cpu > 80 && systemStats.ram > 80 ? 'are' : 'is'} almost at full capacity.`
      : null;
  const visibleResourceWarning =
    resourceWarning && resourceWarning !== dismissedResourceWarning ? resourceWarning : null;
  const errorDismissible = error ? !isFatalUiError(error) : false;
  const tokenMeterLabel = formatTokenMeter(contextUsage);
  const showCenteredComposer = !activeSessionId && messages.length === 0;
  const filteredCatalogModels = OLLAMA_MODEL_CATALOG.filter((model) => {
    const search = modelSearch.trim().toLowerCase();
    const matchesSearch =
      !search ||
      model.name.toLowerCase().includes(search) ||
      model.provider.toLowerCase().includes(search) ||
      model.tags.some((tag) => tag.toLowerCase().includes(search));
    const matchesProvider =
      selectedModelProviderTag === 'All' ||
      model.provider === selectedModelProviderTag ||
      model.tags.includes(selectedModelProviderTag);

    return matchesSearch && matchesProvider;
  });
  const activeProvider = availableProviders.find((provider) => provider.active);

  const activeSession = useMemo(
    () => sessions.find((session) => session.session_id === activeSessionId),
    [activeSessionId, sessions],
  );
  const activeProject = useMemo(
    () => codeProjects.find((project) => project.id === activeProjectId) ?? null,
    [activeProjectId, codeProjects],
  );
  const pinnedSessionIdSet = useMemo(
    () => new Set(pinnedSessionIds),
    [pinnedSessionIds],
  );
  const speakAssistantResponse = useCallback(async (text: string, force = false, messageIndex?: number) => {
    if (!isTtsEnabled && !force) return;
    
    if (activeAudioRef.current) {
      activeAudioRef.current.pause();
      activeAudioRef.current = null;
      setIsSpeaking(false);
      setSpeakingMessageIndex(null);
    }

    if (messageIndex !== undefined && speakingMessageIndex === messageIndex) {
      return;
    }
    
    const cleanText = sanitizeTextForTts(text);
    if (!cleanText) return;

    try {
      if (messageIndex !== undefined) {
        setSpeakingMessageIndex(messageIndex);
      }
      setIsSpeaking(true);
      
      const response = await fetch(`${API_BASE}/voice/synthesize?text=${encodeURIComponent(cleanText)}`);
      if (!response.ok) throw new Error('Synthesis failed');
      
      const audioBlob = await response.blob();
      const audioUrl = URL.createObjectURL(audioBlob);
      const audio = new Audio(audioUrl);
      activeAudioRef.current = audio;
      
      audio.onended = () => {
        setIsSpeaking(false);
        setSpeakingMessageIndex(null);
        URL.revokeObjectURL(audioUrl);
        activeAudioRef.current = null;
      };
      
      audio.onerror = () => {
        setIsSpeaking(false);
        setSpeakingMessageIndex(null);
        URL.revokeObjectURL(audioUrl);
        activeAudioRef.current = null;
      };
      
      audio.play();
    } catch (err) {
      console.error('TTS error:', err);
      setIsSpeaking(false);
      setSpeakingMessageIndex(null);
    }
  }, [isTtsEnabled, speakingMessageIndex]);

  const handleStopDictation = async () => {
    setIsTranscribing(true);
    try {
      const audioBlob = await stopRecording();
      const formData = new FormData();
      formData.append('file', audioBlob, 'voice.wav');

      const response = await fetch(`${API_BASE}/voice/transcribe`, {
        method: 'POST',
        body: formData,
      });

      if (!response.ok) throw new Error('Transcription failed');
      const data = await response.json();

      if (data.text && data.text.trim()) {
        const prompt = data.text.trim();
        setInput('');
        const submittedAt = new Date().toISOString();
        await streamPrompt(prompt, [
          ...messages,
          { role: 'user', content: prompt, timestamp: submittedAt },
          { role: 'assistant', content: '' },
        ]);
      }
    } catch (err) {
      console.error('Dictation error:', err);
      setError('Could not transcribe audio. Is the RAG service running?');
    } finally {
      setIsTranscribing(false);
    }
  };

  const sortedSessions = useMemo(() => {
    const originalOrder = new Map(
      sessions.map((session, index) => [session.session_id, index]),
    );

    return [...sessions].sort((left, right) => {
      const pinnedDifference =
        Number(pinnedSessionIdSet.has(right.session_id)) -
        Number(pinnedSessionIdSet.has(left.session_id));

      if (pinnedDifference !== 0) {
        return pinnedDifference;
      }

      const recentDifference = sessionUpdatedAtMs(right) - sessionUpdatedAtMs(left);

      if (recentDifference !== 0) {
        return recentDifference;
      }

      return (
        (originalOrder.get(left.session_id) ?? 0) -
        (originalOrder.get(right.session_id) ?? 0)
      );
    });
  }, [pinnedSessionIdSet, sessions]);
  const indexedDocuments = activeSessionId
    ? indexedDocumentsBySession[activeSessionId] ?? []
    : [];
  const showImportProgress = importPhase !== 'idle';
  const indexedDocumentLabel =
    indexedDocuments.length === 1
      ? indexedDocuments[0].file_name
      : `${indexedDocuments.length} documents`;
  const indexedChunkCount = indexedDocuments.reduce(
    (total, document) => total + document.chunks_added,
    0,
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
    activeSessionIdRef.current = session.session_id;
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
    activeSessionIdRef.current = session.session_id;
    setActiveSessionId(session.session_id);
    setMessages(turnsToMessages(session.history.turns, session.session_id));
    setStatus('Ready');
  }, []);

  const loadSettingsData = useCallback(async () => {
    setSettingsLoading(true);
    setSettingsMessage(null);

    try {
      const [modelsResult, providersResult, profileResult] = await Promise.allSettled([
        fetch(`${API_BASE}/models/ollama`),
        fetch(`${API_BASE}/providers`),
        fetch(`${API_BASE}/profile`),
      ]);

      if (modelsResult.status === 'fulfilled' && modelsResult.value.ok) {
        const data = (await modelsResult.value.json()) as ModelListResponse;
        setAvailableModels(data.models);
      }

      if (providersResult.status === 'fulfilled' && providersResult.value.ok) {
        const data = (await providersResult.value.json()) as ProviderListResponse;
        setAvailableProviders(data.providers);
      }

      if (profileResult.status === 'fulfilled' && profileResult.value.ok) {
        const data = (await profileResult.value.json()) as ProfileResponse;
        setProfileText(data.contents);
        setProfilePath(data.path);
      }
    } catch (settingsError) {
      setSettingsMessage(
        settingsError instanceof Error ? settingsError.message : 'Could not load settings.',
      );
    } finally {
      setSettingsLoading(false);
    }
  }, []);

  useEffect(() => {
    const interval = setInterval(() => {
      fetch(`${API_BASE}/system/stats`)
        .then((res) => res.json())
        .then((data: { cpu: number; ram: number }) => {
          setSystemStats(data);
        })
        .catch(() => {
          // Silent fail for background stats
        });
    }, 3000);
    return () => clearInterval(interval);
  }, []);

  useEffect(() => {
    loadSessions().catch((loadError: unknown) => {
      setError(loadError instanceof Error ? loadError.message : 'Could not load sessions.');
      setStatus('Engine unavailable');
    });
  }, [loadSessions]);

  useEffect(() => {
    fetch(`${API_BASE}/profile`)
      .then((response) => (response.ok ? response.json() : null))
      .then((data: ProfileResponse | null) => {
        if (data) {
          setProfileText(data.contents);
          setProfilePath(data.path);
        }
      })
      .catch(() => {
        // Profile personalization is optional; settings can retry later.
      });
  }, []);

  useEffect(() => {
    if (settingsOpen) {
      void loadSettingsData();
    }
  }, [loadSettingsData, settingsOpen]);

  useEffect(() => {
    activeSessionIdRef.current = activeSessionId;
  }, [activeSessionId]);

  useEffect(() => {
    let cancelled = false;

    async function loadSystemStats() {
      try {
        const response = await fetch(`${API_BASE}/system/stats`);
        if (!response.ok) {
          return;
        }

        const data = (await response.json()) as Partial<SystemStats>;
        if (!cancelled) {
          setSystemStats({
            cpu: Math.max(0, Math.min(100, Math.round(Number(data.cpu ?? 0)))),
            ram: Math.max(0, Math.min(100, Math.round(Number(data.ram ?? 0)))),
          });
        }
      } catch {
        // Keep the last known values if the engine is temporarily unavailable.
      }
    }

    void loadSystemStats();
    const interval = window.setInterval(() => {
      void loadSystemStats();
    }, 2000);

    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    async function loadContextUsage() {
      try {
        const usage = await fetchContextUsage(activeSessionId);
        if (!cancelled) {
          setContextUsage(usage);
        }
      } catch {
        if (!cancelled && contextUsage.context_window <= 0) {
          setContextUsage({
            ...EMPTY_CONTEXT_USAGE,
            usage_source: 'unavailable',
          });
        }
      }
    }

    void loadContextUsage();
    const interval = window.setInterval(() => {
      void loadContextUsage();
    }, 4000);

    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
  }, [activeSessionId, contextUsage.context_window]);

  const toggleVoiceLowRamMode = useCallback(async (enabled: boolean) => {
    setIsVoiceLowRamMode(enabled);
    if (typeof window !== 'undefined') {
      window.localStorage.setItem(VOICE_LOW_RAM_MODE_STORAGE_KEY, JSON.stringify(enabled));
    }
    try {
      await fetch(`${API_BASE}/voice/config`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ keep_cached: !enabled }),
      });
    } catch {
      // Ignored
    }
  }, []);

  const toggleRagEnabled = useCallback((enabled: boolean) => {
    setIsRagEnabled(enabled);
    if (typeof window !== 'undefined') {
      window.localStorage.setItem(RAG_ENABLED_STORAGE_KEY, JSON.stringify(enabled));
    }
  }, []);

  const changeRagTopK = useCallback((val: number) => {
    setRagTopK(val);
    if (typeof window !== 'undefined') {
      window.localStorage.setItem(RAG_TOP_K_STORAGE_KEY, JSON.stringify(val));
    }
  }, []);

  const changeRagThreshold = useCallback((val: number) => {
    setRagSimilarityThreshold(val);
    if (typeof window !== 'undefined') {
      window.localStorage.setItem(RAG_THRESHOLD_STORAGE_KEY, JSON.stringify(val));
    }
  }, []);

  const changeTtsEnabled = useCallback((enabled: boolean) => {
    setIsTtsEnabled(enabled);
    if (typeof window !== 'undefined') {
      window.localStorage.setItem(VOICE_TTS_ENABLED_STORAGE_KEY, JSON.stringify(enabled));
    }
  }, []);

  useEffect(() => {
    const syncVoiceConfig = async () => {
      try {
        await fetch(`${API_BASE}/voice/config`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ keep_cached: !isVoiceLowRamMode }),
        });
      } catch {
        // Ignored
      }
    };
    
    const timer = setTimeout(syncVoiceConfig, 3000);
    return () => clearTimeout(timer);
  }, [isVoiceLowRamMode]);

  useEffect(() => {
    if (typeof window === 'undefined') {
      return;
    }

    window.localStorage.setItem(THEME_STORAGE_KEY, theme);
  }, [theme]);

  useEffect(() => {
    if (typeof window === 'undefined') {
      return;
    }

    window.localStorage.setItem(APPEARANCE_THEME_STORAGE_KEY, appearanceTheme);
  }, [appearanceTheme]);

  useEffect(() => {
    if (typeof window === 'undefined') {
      return;
    }

    window.localStorage.setItem(RESPONSE_STYLE_STORAGE_KEY, responseStyle);
  }, [responseStyle]);

  useEffect(() => {
    let cancelled = false;

    async function loadWelcomeMessages() {
      try {
        const response = await fetch('/welcome-messages.md', { cache: 'no-cache' });
        if (!response.ok) {
          return;
        }

        const messages = parseWelcomeMessages(await response.text());
        if (!cancelled) {
          setWelcomeMessages(messages);
          setActiveWelcomeMessage((current) =>
            DEFAULT_WELCOME_MESSAGES.includes(current) ? randomWelcomeMessage(messages) : current,
          );
        }
      } catch {
        // The built-in welcome messages remain available if the editable file is missing.
      }
    }

    void loadWelcomeMessages();

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (typeof window === 'undefined') {
      return;
    }

    window.localStorage.setItem(
      PINNED_SESSIONS_STORAGE_KEY,
      JSON.stringify(pinnedSessionIds),
    );
  }, [pinnedSessionIds]);

  useEffect(() => {
    if (sessions.length === 0) {
      return;
    }

    const availableSessionIds = new Set(sessions.map((session) => session.session_id));

    setPinnedSessionIds((current) => {
      const next = current.filter((sessionId) => availableSessionIds.has(sessionId));
      return next.length === current.length ? current : next;
    });
  }, [sessions]);

  useEffect(() => {
    if (typeof window === 'undefined') {
      return;
    }

    window.localStorage.removeItem('aegis-indexed-documents');
    window.localStorage.setItem(
      INDEXED_DOCUMENTS_STORAGE_KEY,
      JSON.stringify(indexedDocumentsBySession),
    );
  }, [indexedDocumentsBySession]);

  useEffect(() => {
    scrollRef.current?.scrollTo({
      top: scrollRef.current.scrollHeight,
      behavior: 'smooth',
    });
  }, [messages, isStreaming]);

  useEffect(() => {
    if (composerTextareaRef.current) {
      fitTextareaToContent(composerTextareaRef.current);
    }
  }, [input]);

  useEffect(() => {
    return () => {
      if (settingsCloseTimeoutRef.current !== null) {
        window.clearTimeout(settingsCloseTimeoutRef.current);
      }
    };
  }, []);

  useEffect(() => {
    if (!documentContextNotice) {
      return;
    }

    const timeout = window.setTimeout(() => {
      setDocumentContextNotice(null);
    }, 7000);

    return () => {
      window.clearTimeout(timeout);
    };
  }, [documentContextNotice]);

  async function handleSessionSelect(sessionId: string) {
    if (deletingSessionIds.includes(sessionId)) {
      return;
    }

    setSessionMenuOpenId(null);
    setSelectedMessageSources(null);
    setSelectedMessageSourcesIndex(null);
    setMetricsTab('metrics');

    if (streamingSessionId === sessionId) {
      const streamingMessages = streamingMessagesBySession[sessionId];
      if (streamingMessages) {
        activeSessionIdRef.current = sessionId;
        setActiveSessionId(sessionId);
        setMessages(streamingMessages);
        setStatus('Inference');
        return;
      }
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

    setSessionMenuOpenId(null);
    activeSessionIdRef.current = null;
    setActiveSessionId(null);
    setMessages([]);
    setSelectedMessageSources(null);
    setSelectedMessageSourcesIndex(null);
    setMetricsTab('metrics');
    setInput('');
    setError(null);
    setEditingMessageIndex(null);
    setEditingMessageText('');
    setEditingSessionId(null);
    setEditingTitle('');
    setImportPhase('idle');
    setImportProgress(0);
    setImportFileLabel('');
    setActiveWelcomeMessage(randomWelcomeMessage(welcomeMessages));
    setStatus('Ready');
  }

  async function handleAddProject() {
    if (!window.showDirectoryPicker) {
      setError(
        'Your browser does not support local folder access. Use Chrome or Edge for AEGIS Projects.',
      );
      return;
    }

    setScanningProject(true);
    setProjectEditMessage(null);
    setError(null);
    setStatus('Scanning project');

    try {
      const rootHandle = await window.showDirectoryPicker();
      const files = await scanProjectDirectory(rootHandle);
      const totalBytes = files.reduce((total, file) => total + file.size, 0);
      const project: CodeProject = {
        id: `${rootHandle.name}-${Date.now()}`,
        name: rootHandle.name,
        fileCount: files.length,
        totalBytes,
        snapshot: buildProjectSnapshot(rootHandle.name, files),
        files,
        writable: false,
        updatedAt: new Date().toISOString(),
        rootHandle,
      };

      setCodeProjects((current) => [project, ...current.filter((item) => item.name !== project.name)]);
      setActiveProjectId(project.id);
      setProjectsOpen(true);
      setChatMode('coder');
      setProjectPermissionRequestId(project.id);
      setStatus(`Project ${project.name} scanned`);
    } catch (projectError) {
      if (projectError instanceof DOMException && projectError.name === 'AbortError') {
        setStatus('Ready');
      } else {
        setError(projectError instanceof Error ? projectError.message : 'Could not scan project.');
        setStatus('Project scan failed');
      }
    } finally {
      setScanningProject(false);
    }
  }

  async function requestProjectWritePermission(projectId: string) {
    const project = codeProjects.find((item) => item.id === projectId);
    if (!project) {
      setProjectPermissionRequestId(null);
      return;
    }

    try {
      const permission =
        (await project.rootHandle.requestPermission?.({ mode: 'readwrite' })) ?? 'denied';

      setCodeProjects((current) =>
        current.map((item) =>
          item.id === projectId ? { ...item, writable: permission === 'granted' } : item,
        ),
      );
      setProjectEditMessage(
        permission === 'granted'
          ? `AEGIS can apply approved patches inside ${project.name}.`
          : `${project.name} remains read-only until write access is granted.`,
      );
    } catch (permissionError) {
      setProjectEditMessage(
        permissionError instanceof Error
          ? permissionError.message
          : 'Could not request project write permission.',
      );
    } finally {
      setProjectPermissionRequestId(null);
    }
  }

  function removeProject(projectId: string) {
    setCodeProjects((current) => current.filter((project) => project.id !== projectId));
    setActiveProjectId((current) => (current === projectId ? null : current));
    setProjectEditMessage(null);
  }

  async function applyAssistantPatch(messageContent: string) {
    if (!activeProject) {
      setError('Open a project before applying code patches.');
      return;
    }

    if (!activeProject.writable) {
      setError('Project edits are disabled. Grant edit permission before applying a patch.');
      return;
    }

    const diff = extractUnifiedDiff(messageContent);
    const changedFiles = diff
      .split('\n')
      .filter((line) => line.startsWith('+++ ') && !line.includes('/dev/null'));
    const targetPath = parsePatchTarget(diff);
    const targetFile = targetPath ? findProjectFile(activeProject, targetPath) : null;

    if (changedFiles.length > 1) {
      setError('Automatic patch apply currently supports one file at a time.');
      return;
    }

    if (!diff || !targetFile?.handle.createWritable) {
      setError('AEGIS could not find a supported unified diff for the active project.');
      return;
    }

    try {
      const nextContent = applySimpleUnifiedDiff(targetFile.content, diff);
      const writable = await targetFile.handle.createWritable();
      await writable.write(nextContent);
      await writable.close();

      const nextFiles = activeProject.files.map((file) =>
        file.path === targetFile.path
          ? { ...file, content: nextContent, size: new Blob([nextContent]).size }
          : file,
      );
      const nextProject = {
        ...activeProject,
        files: nextFiles,
        fileCount: nextFiles.length,
        totalBytes: nextFiles.reduce((total, file) => total + file.size, 0),
        snapshot: buildProjectSnapshot(activeProject.name, nextFiles),
        updatedAt: new Date().toISOString(),
      };

      setCodeProjects((current) =>
        current.map((project) => (project.id === nextProject.id ? nextProject : project)),
      );
      setProjectEditMessage(`Applied patch to ${targetFile.path}.`);
      setStatus('Project patch applied');
    } catch (patchError) {
      setError(patchError instanceof Error ? patchError.message : 'Could not apply project patch.');
      setStatus('Patch failed');
    }
  }

  async function handleDeleteSession(session: EngineSessionSummary) {
    if (isStreaming || deletingSessionIds.includes(session.session_id)) {
      return;
    }

    setSessionMenuOpenId(null);
    setSessionPendingDeletion(session);
  }

  async function confirmDeleteSession() {
    const session = sessionPendingDeletion;
    if (!session || deletingSessionIds.includes(session.session_id)) {
      return;
    }

    setSessionPendingDeletion(null);
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
      setIndexedDocumentsBySession((current) => {
        const next = { ...current };
        delete next[session.session_id];
        return next;
      });
      setPinnedSessionIds((current) =>
        current.filter((sessionId) => sessionId !== session.session_id),
      );

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

    setSessionMenuOpenId(null);
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

    const selectedFiles = Array.from(files);
    const validExtensions = ['.pdf', '.txt'];
    const unsupportedFiles = selectedFiles.filter(
      (file) => !validExtensions.some((ext) => file.name.toLowerCase().endsWith(ext)),
    );

    if (unsupportedFiles.length > 0) {
      setError(`Unsupported file types: ${unsupportedFiles.map((f) => f.name).join(', ')}. Only PDF and TXT are supported.`);
      event.target.value = '';
      return;
    }

    setIsUploading(true);
    setImportPhase('uploading');
    setImportProgress(3);
    setImportFileLabel(
      selectedFiles.length === 1 ? selectedFiles[0].name : `${selectedFiles.length} documents`,
    );
    setStatus(activeSessionId ? 'Indexing documents' : 'Starting document session');
    setError(null);

    try {
      let sessionId = activeSessionId;
      let createdSessionId: string | null = null;

      if (!sessionId) {
        const session = await createSession();
        sessionId = session.session_id;
        createdSessionId = session.session_id;
        setNewSessionPulseId(session.session_id);
        setMessages([]);
      }

      setStatus('Indexing documents');
      const formData = new FormData();
      formData.append('session_id', sessionId);
      for (const file of selectedFiles) {
        formData.append('file', file);
      }

      const ingestResponse = await new Promise<IngestResponse>((resolve, reject) => {
        const request = new XMLHttpRequest();
        request.open('POST', `${API_BASE}/ingest`);

        request.upload.onprogress = (progressEvent) => {
          if (!progressEvent.lengthComputable) {
            setImportProgress((current) => Math.max(current, 12));
            return;
          }

          const uploadPercent = Math.round(
            (progressEvent.loaded / progressEvent.total) * 68,
          );
          setImportProgress(Math.max(5, Math.min(70, uploadPercent)));
        };

        request.upload.onload = () => {
          setImportPhase('indexing');
          setImportProgress(72);
        };

        request.onload = () => {
          if (request.status >= 200 && request.status < 300) {
            try {
              resolve(JSON.parse(request.responseText) as IngestResponse);
            } catch {
              reject(new Error('Engine indexed the document but returned an unreadable response.'));
            }
            return;
          }

          reject(
            new Error(
              request.responseText ||
              `Engine returned HTTP ${request.status} while uploading.`,
            ),
          );
        };

        request.onerror = () => {
          reject(new Error('Could not reach the engine while importing documents.'));
        };

        request.onabort = () => {
          reject(new Error('Document import was cancelled.'));
        };

        request.send(formData);
      });

      setIndexedDocumentsBySession((current) => ({
        ...current,
        [sessionId]: mergeIndexedDocuments(
          current[sessionId] ?? [],
          ingestResponse.documents,
        ),
      }));
      setImportFileLabel(
        ingestResponse.documents.length === 1
          ? ingestResponse.documents[0].file_name
          : `${ingestResponse.documents.length} documents`,
      );
      setImportPhase('complete');
      setImportProgress(100);
      if (ingestResponse.session) {
        setActiveSessionId(ingestResponse.session.session_id);
      }
      await loadSessions();
      if (createdSessionId) {
        window.setTimeout(() => {
          setNewSessionPulseId((current) =>
            current === createdSessionId ? null : current,
          );
        }, 1400);
      }
      setStatus(`Indexed ${ingestResponse.total_chunks} document chunks`);
    } catch (uploadError) {
      setImportPhase('error');
      setImportProgress(100);
      setError(uploadError instanceof Error ? uploadError.message : 'Upload failed');
      setStatus('Upload failed');
    } finally {
      setIsUploading(false);
      event.target.value = '';
      window.setTimeout(() => {
        setImportPhase('idle');
        setImportProgress(0);
        setImportFileLabel('');
      }, 1800);
    }
  }

  async function deleteIndexedDocumentFromRag(
    sessionId: string,
    document: IndexedDocument,
  ): Promise<DeleteIndexedDocumentResponse> {
    const response = await fetch(`${API_BASE}/ingest/document`, {
      method: 'DELETE',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        session_id: sessionId,
        stored_path: document.stored_path,
      }),
    });

    if (!response.ok) {
      const body = await response.text();
      throw new Error(
        body ||
        `Engine returned HTTP ${response.status} while removing ${document.file_name}.`,
      );
    }

    return (await response.json()) as DeleteIndexedDocumentResponse;
  }

  async function clearIndexedDocuments() {
    if (!activeSessionId || indexedDocuments.length === 0 || isClearingIndexedDocuments) {
      return;
    }

    const sessionId = activeSessionId;
    const documentsToRemove = [...indexedDocuments];

    setIsClearingIndexedDocuments(true);
    setDocumentContextNotice(null);
    setError(null);
    setStatus('Removing document context');

    try {
      const results = await Promise.allSettled(
        documentsToRemove.map((document) => deleteIndexedDocumentFromRag(sessionId, document)),
      );
      const removedPaths = new Set<string>();
      const failures: string[] = [];
      let deletedChunks = 0;

      results.forEach((result, index) => {
        const document = documentsToRemove[index];
        if (result.status === 'fulfilled') {
          removedPaths.add(document.stored_path);
          deletedChunks += Math.max(0, Number(result.value.deleted_chunks ?? 0));
          return;
        }

        failures.push(
          result.reason instanceof Error
            ? result.reason.message
            : `Could not remove ${document.file_name}.`,
        );
      });

      if (removedPaths.size > 0) {
        setIndexedDocumentsBySession((current) => {
          const remainingDocuments = (current[sessionId] ?? []).filter(
            (document) => !removedPaths.has(document.stored_path),
          );
          const next = { ...current };

          if (remainingDocuments.length > 0) {
            next[sessionId] = remainingDocuments;
          } else {
            delete next[sessionId];
          }

          return next;
        });
      }

      setImportPhase('idle');
      setImportProgress(0);
      setImportFileLabel('');

      if (failures.length > 0) {
        setError(
          `Could not remove ${failures.length} imported document${failures.length === 1 ? '' : 's'} from RAG memory. ${failures[0]}`,
        );
        setStatus('Document removal incomplete');
        return;
      }

      setStatus(
        deletedChunks > 0
          ? `Removed ${deletedChunks} document chunks`
          : 'Removed document context',
      );
      setDocumentContextNotice('Imported document cleared.');
    } finally {
      setIsClearingIndexedDocuments(false);
    }
  }

  async function loadOutlookCalendars() {
    setLoadingOutlookCalendars(true);

    try {
      const response = await fetch(`${API_BASE}/calendar/outlook/calendars`);
      if (!response.ok) {
        const body = await response.text();
        throw new Error(body || `Engine returned HTTP ${response.status} while loading Outlook calendars.`);
      }

      const data = (await response.json()) as OutlookCalendarsResponse;
      const visibleCalendars = data.calendars.filter(isVisibleOutlookCalendar);
      setOutlookCalendars(visibleCalendars);
      setSelectedOutlookCalendarId(
        visibleCalendars.find((calendar) => calendar.is_selected)?.id ?? '',
      );
    } catch (calendarError) {
      setError(
        calendarError instanceof Error
          ? calendarError.message
          : 'Could not load Outlook calendars.',
      );
    } finally {
      setLoadingOutlookCalendars(false);
    }
  }

  async function selectOutlookCalendar(calendarId: string) {
    setSelectedOutlookCalendarId(calendarId);
    setCalendarMessage(null);

    if (!calendarId) {
      return;
    }

    try {
      const response = await fetch(`${API_BASE}/calendar/outlook/select`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ calendar_id: calendarId }),
      });

      if (!response.ok) {
        const body = await response.text();
        throw new Error(body || `Engine returned HTTP ${response.status} while selecting an Outlook calendar.`);
      }

      const data = (await response.json()) as OutlookCalendarSelectionResponse;
      setCalendarMessage(`Outlook calendar selected: ${outlookCalendarLabel(data.calendar)}`);
      setOutlookCalendars((current) =>
        current.map((calendar) => ({
          ...calendar,
          is_selected: calendar.id === data.calendar.id,
        })),
      );
    } catch (calendarError) {
      setError(
        calendarError instanceof Error
          ? calendarError.message
          : 'Could not select Outlook calendar.',
      );
    }
  }

  function openCalendarTool() {
    setCalendarOpen(true);
    setToolsOpen(false);
    setError(null);
    setCalendarPrompt('');
    setCalendarResult(null);
    setCalendarMessage(null);
    void loadOutlookCalendars();
  }

  async function createCalendarEvent() {
    const prompt = calendarPrompt.trim();
    if (!prompt || creatingCalendarEvent) {
      return;
    }

    setCreatingCalendarEvent(true);
    setError(null);
    setCalendarResult(null);
    setCalendarMessage(null);
    setStatus('Creating calendar event');

    try {
      const response = await fetch(`${API_BASE}/calendar/create-from-prompt`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ prompt }),
      });

      if (!response.ok) {
        const body = await response.text();
        throw new Error(body || `Engine returned HTTP ${response.status} while creating the calendar event.`);
      }

      const data = (await response.json()) as CalendarCreateResponse;
      setCalendarResult(data.parsed);
      setCalendarMessage(data.message);
      setStatus(data.saved_to_calendar ? 'Calendar event saved' : 'Calendar event created');
    } catch (calendarError) {
      setError(
        calendarError instanceof Error
          ? calendarError.message
          : 'Could not create calendar event.',
      );
      setStatus('Calendar failed');
    } finally {
      setCreatingCalendarEvent(false);
    }
  }

  function handleImportToolClick() {
    setToolsOpen(false);
    fileInputRef.current?.click();
  }

  function exportChatAsPdf() {
    if (messages.length === 0) {
      return;
    }

    downloadConversationPdf({
      title: activeSession?.title ?? 'AEGIS Chat Export',
      sessionId: activeSession?.session_id,
      messages,
      indexedDocuments,
    });
    setToolsOpen(false);
  }

  async function exportSessionAsPdf(sessionSummary: EngineSessionSummary) {
    if (isStreaming) {
      return;
    }

    setSessionMenuOpenId(null);
    setError(null);
    setStatus('Preparing export');

    try {
      const response = await fetch(
        `${API_BASE}/sessions/${encodeURIComponent(sessionSummary.session_id)}`,
      );

      if (!response.ok) {
        throw new Error(`Engine returned HTTP ${response.status} while loading the session export.`);
      }

      const session = (await response.json()) as EngineSession;
      downloadConversationPdf({
        title: session.title || sessionSummary.title || 'AEGIS Chat Export',
        sessionId: session.session_id,
        messages: turnsToMessages(session.history.turns, session.session_id),
        indexedDocuments: indexedDocumentsBySession[session.session_id] ?? [],
      });
      setStatus('Export ready');
    } catch (exportError) {
      setError(exportError instanceof Error ? exportError.message : 'Could not export the session.');
      setStatus('Export failed');
    }
  }

  function togglePinnedSession(sessionId: string) {
    setPinnedSessionIds((current) =>
      current.includes(sessionId)
        ? current.filter((pinnedSessionId) => pinnedSessionId !== sessionId)
        : [sessionId, ...current],
    );
    setSessionMenuOpenId(null);
  }

  function openSettings(tab: SettingsTab = 'general') {
    if (settingsCloseTimeoutRef.current !== null) {
      window.clearTimeout(settingsCloseTimeoutRef.current);
      settingsCloseTimeoutRef.current = null;
    }
    setSettingsTab(tab);
    setSettingsClosing(false);
    setSettingsOpen(true);
    setSettingsMessage(null);
  }

  function closeSettings() {
    if (!settingsOpen || settingsClosing) {
      return;
    }

    setSettingsClosing(true);
    if (settingsCloseTimeoutRef.current !== null) {
      window.clearTimeout(settingsCloseTimeoutRef.current);
    }
    settingsCloseTimeoutRef.current = window.setTimeout(() => {
      setSettingsOpen(false);
      setSettingsClosing(false);
      settingsCloseTimeoutRef.current = null;
    }, 200);
  }

  async function selectProvider(providerName: string) {
    setSettingsMessage(null);

    try {
      const response = await fetch(`${API_BASE}/providers/select`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name: providerName }),
      });

      if (!response.ok) {
        const body = await response.text();
        throw new Error(body || `Engine returned HTTP ${response.status} while switching provider.`);
      }

      await loadSettingsData();
      setSettingsMessage(`Inference provider switched to ${providerName}.`);
    } catch (providerError) {
      setSettingsMessage(
        providerError instanceof Error ? providerError.message : 'Could not switch provider.',
      );
    }
  }

  async function selectModel(modelName: string) {
    setSettingsMessage(null);

    try {
      const response = await fetch(`${API_BASE}/models/select`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name: modelName }),
      });

      if (!response.ok) {
        const body = await response.text();
        throw new Error(body || `Engine returned HTTP ${response.status} while switching model.`);
      }

      await loadSettingsData();
      setSettingsMessage(`Active model switched to ${modelName}.`);
    } catch (modelError) {
      setSettingsMessage(modelError instanceof Error ? modelError.message : 'Could not switch model.');
    }
  }

  async function downloadOllamaModel(modelNameOverride?: string) {
    const modelName = (modelNameOverride ?? modelSearch).trim();
    if (!modelName || modelDownloadState === 'downloading') {
      return;
    }

    const controller = new AbortController();
    modelDownloadAbortRef.current = controller;
    modelDownloadAbortReasonRef.current = null;
    setModelSearch(modelName);
    setDownloadingModel(modelName);
    setPausedModelDownload(null);
    setModelDownloadState('downloading');
    setModelDownloadProgress(0);
    setModelDownloadStatus('Starting download');
    setSettingsMessage(null);

    try {
      const response = await fetch(`${API_BASE}/models/pull`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name: modelName }),
        signal: controller.signal,
      });

      if (!response.ok || !response.body) {
        const body = await response.text();
        throw new Error(body || `Engine returned HTTP ${response.status} while downloading model.`);
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
        const parsed = extractSseEvents(pending);
        pending = parsed.remaining;

        for (const event of parsed.events) {
          const data = sseEventData(event);
          if (!data) {
            continue;
          }

          const chunk = JSON.parse(data) as PullModelChunk;
          if (chunk.error) {
            throw new Error(chunk.error);
          }

          setModelDownloadStatus(chunk.status ?? 'Downloading');
          const percent = modelDownloadPercent(chunk);
          if (percent !== null) {
            setModelDownloadProgress(percent);
          }
        }
      }

      setModelDownloadProgress(100);
      setModelDownloadStatus('Download complete');
      await loadSettingsData();
      setSettingsMessage(`${modelName} is ready in Ollama.`);
    } catch (downloadError) {
      if (controller.signal.aborted) {
        return;
      }

      setModelDownloadStatus('Download failed');
      setSettingsMessage(
        downloadError instanceof Error ? downloadError.message : 'Could not download model.',
      );
    } finally {
      const abortReason = modelDownloadAbortReasonRef.current;
      modelDownloadAbortRef.current = null;
      modelDownloadAbortReasonRef.current = null;

      if (abortReason === 'pause') {
        setPausedModelDownload(modelName);
        setDownloadingModel(null);
        setModelDownloadState('paused');
        setModelDownloadStatus('Paused');
      } else if (abortReason === 'cancel') {
        setPausedModelDownload(null);
        setDownloadingModel(null);
        setModelDownloadState('idle');
        setModelDownloadProgress(0);
        setModelDownloadStatus('');
      } else {
        setPausedModelDownload(null);
        setDownloadingModel(null);
        setModelDownloadState('idle');
      }
    }
  }

  function pauseModelDownload() {
    if (!downloadingModel || modelDownloadState !== 'downloading') {
      return;
    }

    modelDownloadAbortReasonRef.current = 'pause';
    setModelDownloadStatus('Pausing');
    modelDownloadAbortRef.current?.abort();
  }

  function cancelModelDownload() {
    if (!downloadingModel && !pausedModelDownload) {
      return;
    }

    modelDownloadAbortReasonRef.current = 'cancel';
    modelDownloadAbortRef.current?.abort();
    setPausedModelDownload(null);
    setDownloadingModel(null);
    setModelDownloadState('idle');
    setModelDownloadProgress(0);
    setModelDownloadStatus('');
    setSettingsMessage('Model download cancelled.');
  }

  function resumeModelDownload() {
    if (!pausedModelDownload) {
      return;
    }

    void downloadOllamaModel(pausedModelDownload);
  }

  async function saveProfileSettings() {
    setSettingsMessage(null);

    try {
      const response = await fetch(`${API_BASE}/profile`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ contents: profileText }),
      });

      if (!response.ok) {
        const body = await response.text();
        throw new Error(body || `Engine returned HTTP ${response.status} while saving profile.`);
      }

      const data = (await response.json()) as ProfileResponse;
      setProfileText(data.contents);
      setProfilePath(data.path);
      setSettingsMessage(
        'Personalization saved locally as markdown and will be applied to future replies.',
      );
    } catch (profileError) {
      setSettingsMessage(
        profileError instanceof Error ? profileError.message : 'Could not save profile.',
      );
    }
  }

  async function importProfileFile(event: React.ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    if (!file) {
      return;
    }

    if (!file.name.toLowerCase().endsWith('.txt') && !file.name.toLowerCase().endsWith('.md')) {
      setSettingsMessage('Only .txt and .md profile files are supported.');
      event.target.value = '';
      return;
    }

    try {
      setProfileText(await file.text());
      setSettingsMessage(
        `Imported ${file.name}. Save to store it as your local markdown personalization profile.`,
      );
    } catch {
      setSettingsMessage('Could not read the selected profile file.');
    } finally {
      event.target.value = '';
    }
  }

  async function streamPrompt(
    prompt: string,
    nextMessages: Message[],
    editFromTurnIndex?: number,
  ) {
    setError(null);
    setStatus('Inference');
    setIsStreaming(true);

    let targetSessionId: string | null = null;
    let seedMessages = nextMessages;
    const pendingAssistantSegments: string[] = [];
    let streamFlushTimer: number | null = null;
    let streamDrainResolver: (() => void) | null = null;
    let streamClosed = false;

    const updateTargetMessages = (updater: (current: Message[]) => Message[]) => {
      if (!targetSessionId) {
        return;
      }

      const sessionId = targetSessionId;
      const updatedMessages = updater(
        streamingMessagesBySessionRef.current[sessionId] ?? seedMessages,
      );

      // Automatically persist sources to localStorage so they survive page refreshes!
      updatedMessages.forEach((msg, idx) => {
        if (msg.role === 'assistant' && msg.sources && msg.sources.length > 0) {
          localStorage.setItem(`aegis-sources-${sessionId}-${idx}`, JSON.stringify(msg.sources));
        }
      });

      streamingMessagesBySessionRef.current = {
        ...streamingMessagesBySessionRef.current,
        [sessionId]: updatedMessages,
      };
      setStreamingMessagesBySession(streamingMessagesBySessionRef.current);

      if (activeSessionIdRef.current === sessionId) {
        setMessages(updatedMessages);
      }
    };
    const settleStreamDrain = () => {
      if (
        streamClosed &&
        pendingAssistantSegments.length === 0 &&
        streamFlushTimer === null &&
        streamDrainResolver
      ) {
        const resolve = streamDrainResolver;
        streamDrainResolver = null;
        resolve();
      }
    };
    const flushAssistantSegments = (forceAll = false) => {
      streamFlushTimer = null;

      if (pendingAssistantSegments.length === 0) {
        settleStreamDrain();
        return;
      }

      const segmentCount = forceAll
        ? pendingAssistantSegments.length
        : pendingAssistantSegments.length > 48
          ? 8
          : pendingAssistantSegments.length > 24
            ? 5
            : pendingAssistantSegments.length > 12
              ? 3
              : 1;
      const nextChunk = pendingAssistantSegments.splice(0, segmentCount).join('');

      updateTargetMessages((current) => {
        const next = [...current];
        const last = next[next.length - 1];

        if (last?.role === 'assistant') {
          next[next.length - 1] = {
            ...last,
            content: `${last.content}${nextChunk}`,
            timestamp: last.timestamp ?? new Date().toISOString(),
          };
        }

        return next;
      });

      if (pendingAssistantSegments.length > 0) {
        const delay =
          pendingAssistantSegments.length > 60
            ? 10
            : pendingAssistantSegments.length > 28
              ? 14
              : 18;
        streamFlushTimer = window.setTimeout(() => {
          flushAssistantSegments();
        }, delay);
        return;
      }

      settleStreamDrain();
    };
    const scheduleAssistantFlush = () => {
      if (streamFlushTimer !== null) {
        return;
      }

      streamFlushTimer = window.setTimeout(() => {
        flushAssistantSegments();
      }, 12);
    };
    const enqueueAssistantContent = (content: string) => {
      if (!content) {
        return;
      }

      pendingAssistantSegments.push(...splitAssistantStreamSegments(content));
      scheduleAssistantFlush();
    };
    const waitForAssistantDrain = () => {
      if (pendingAssistantSegments.length === 0 && streamFlushTimer === null) {
        return Promise.resolve();
      }

      return new Promise<void>((resolve) => {
        streamDrainResolver = resolve;
      });
    };
    inferenceStartTime.current = Date.now();
    setInferenceStats({
      latency: 0,
      tps: 0,
      ttft: 0,
      ragTime: 0,
      similarity: 0,
      chunks: 0,
      backend: '---',
    });

    try {
      let sessionId = activeSessionId;
      let createdSessionId: string | null = null;

      if (!sessionId) {
        const session = await createSession();
        sessionId = session.session_id;
        createdSessionId = session.session_id;
        setNewSessionPulseId(session.session_id);
        await loadSessions();
      }

      targetSessionId = sessionId;
      seedMessages = nextMessages;
      setStreamingSessionId(sessionId);
      streamingMessagesBySessionRef.current = {
        ...streamingMessagesBySessionRef.current,
        [sessionId]: seedMessages,
      };
      setStreamingMessagesBySession(streamingMessagesBySessionRef.current);

      if (activeSessionIdRef.current === sessionId) {
        setMessages(seedMessages);
      }

      const response = await fetch(`${API_BASE}/chat`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          session_id: sessionId,
          message: prompt,
          attachments: indexedDocuments.map(
            (document) => `${document.file_name} (${document.chunks_added} chunks)`,
          ),
          edit_from_turn_index: editFromTurnIndex,
          mode: chatMode,
          response_style: responseStyle,
          code_project_name: activeProject?.name,
          code_project_context: activeProject?.snapshot,
          rag_enabled: isRagEnabled,
          rag_top_k: ragTopK,
          rag_similarity_threshold: ragSimilarityThreshold,
        }),
      });

      if (!response.ok || !response.body) {
        throw new Error(`Engine returned HTTP ${response.status} while sending chat.`);
      }

      const reader = response.body.getReader();
      const decoder = new TextDecoder();
      let pending = '';
      let accumulatedResponse = '';

      while (true) {
        const { done, value } = await reader.read();
        if (done) {
          break;
        }

        pending += decoder.decode(value, { stream: true });
        const parsed = extractSseEvents(pending);
        pending = parsed.remaining;

        for (const event of parsed.events) {
          const data = sseEventData(event);
          if (!data) {
            continue;
          }

          if (data.startsWith('[RAG_METRICS] ')) {
            try {
              const metrics = JSON.parse(data.replace('[RAG_METRICS] ', ''));
              setInferenceStats((prev) => ({
                ...prev,
                ragTime: metrics.retrieval_time_ms,
                similarity: metrics.avg_similarity,
                chunks: metrics.chunk_count,
                backend: metrics.backend
              }));
            } catch (e) {
              console.error('Failed to parse RAG metrics:', e);
            }
            continue;
          }

          if (data.startsWith('[RAG_SOURCES] ')) {
            try {
              const parsedSources = JSON.parse(data.replace('[RAG_SOURCES] ', '')) as RetrievalChunk[];
              updateTargetMessages((current) => {
                const next = [...current];
                const last = next[next.length - 1];
                if (last?.role === 'assistant') {
                  next[next.length - 1] = {
                    ...last,
                    sources: parsedSources,
                  };
                }
                return next;
              });
            } catch (e) {
              console.error('Failed to parse RAG sources:', e);
            }
            continue;
          }

          if (data === '[DONE]') {
            setStatus('Complete');
            continue;
          }

          if (data.startsWith('[ERROR]')) {
            throw new Error(data);
          }

          if (accumulatedResponse === '' && inferenceStartTime.current) {
            const ttft = Date.now() - inferenceStartTime.current;
            setInferenceStats((prev) => ({ ...prev, ttft }));
          }

          accumulatedResponse += data;
          enqueueAssistantContent(data);
        }
      }

      const finalData = sseEventData(pending);
      if (finalData && finalData !== '[DONE]') {
        if (finalData.startsWith('[ERROR]')) {
          throw new Error(finalData);
        }

        if (accumulatedResponse === '' && inferenceStartTime.current) {
          const ttft = Date.now() - inferenceStartTime.current;
          setInferenceStats((prev) => ({ ...prev, ttft }));
        }

        accumulatedResponse += finalData;
        enqueueAssistantContent(finalData);
      }

      streamClosed = true;
      await waitForAssistantDrain();

      const totalLatency = Date.now() - (inferenceStartTime.current ?? Date.now());
      const charCount = accumulatedResponse.length;
      const estimatedTokens = Math.max(1, Math.floor(charCount / 4));
      const tps = totalLatency > 0 ? parseFloat(((estimatedTokens / totalLatency) * 1000).toFixed(1)) : 0;

      setInferenceStats((prev) => ({
        ...prev,
        latency: totalLatency,
        tps,
      }));

      setStatus('Complete');
      await loadSessions();

      // VOICE MODE: Read aloud
      if (isTtsEnabled && accumulatedResponse) {
        speakAssistantResponse(accumulatedResponse, false, messages.length - 1);
      }
      try {
        setContextUsage(await fetchContextUsage(sessionId));
      } catch {
        // The periodic token-meter refresh will retry without failing the chat.
      }
      if (createdSessionId) {
        window.setTimeout(() => {
          setNewSessionPulseId((current) =>
            current === createdSessionId ? null : current,
          );
        }, 1400);
      }
    } catch (sendError) {
      if (pendingAssistantSegments.length > 0) {
        flushAssistantSegments(true);
      }
      if (streamFlushTimer !== null) {
        window.clearTimeout(streamFlushTimer);
        streamFlushTimer = null;
      }
      setError(sendError instanceof Error ? sendError.message : 'Could not send chat request.');
      setStatus('Chat failed');
      updateTargetMessages((current) => current.filter((message) => message.content.length > 0));
    } finally {
      if (streamFlushTimer !== null) {
        window.clearTimeout(streamFlushTimer);
      }
      setIsStreaming(false);
      setStreamingSessionId(null);
      if (targetSessionId) {
        const finishedSessionId = targetSessionId;
        const next = { ...streamingMessagesBySessionRef.current };
        delete next[finishedSessionId];
        streamingMessagesBySessionRef.current = next;
        setStreamingMessagesBySession(next);
      }
    }
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const prompt = input.trim();
    if (!prompt || isStreaming) {
      return;
    }

    setInput('');
    const submittedAt = new Date().toISOString();
    await streamPrompt(prompt, [
      ...messages,
      { role: 'user', content: prompt, timestamp: submittedAt },
      { role: 'assistant', content: '' },
    ]);
  }

  function beginEditingMessage(index: number, content: string) {
    if (isStreaming) {
      return;
    }

    setEditingMessageIndex(index);
    setEditingMessageText(content);
    setError(null);
  }

  function cancelEditingMessage() {
    setEditingMessageIndex(null);
    setEditingMessageText('');
  }

  async function copyUserMessage(index: number, content: string) {
    await copyTextToClipboard(content);
    setCopiedMessageIndex(index);
    window.setTimeout(() => {
      setCopiedMessageIndex((current) => (current === index ? null : current));
    }, 1400);
  }

  async function resendEditedMessage(index: number) {
    const prompt = editingMessageText.trim();
    if (!prompt || isStreaming) {
      return;
    }

    const turnIndex = messages.slice(0, index).filter((message) => message.role === 'user').length;

    setEditingMessageIndex(null);
    setEditingMessageText('');
    const submittedAt = new Date().toISOString();
    await streamPrompt(prompt, [
      ...messages.slice(0, index),
      { role: 'user', content: prompt, edited: true, timestamp: submittedAt },
      { role: 'assistant', content: '' },
    ], turnIndex);
  }

  return (
    <div
      className={`aegis-shell aegis-mode-${theme} aegis-theme-${appearanceTheme} flex h-screen overflow-hidden ${isDark ? 'bg-zinc-950 text-zinc-100' : 'bg-stone-100 text-slate-900'
        }`}
      onClick={() => setSessionMenuOpenId(null)}
    >
      <nav
        aria-label="Sidebar controls"
        className={`flex w-14 shrink-0 flex-col items-center border-r ${isDark ? 'border-zinc-800 bg-zinc-950' : 'border-stone-300 bg-stone-50'
          }`}
      >
        <button
          aria-label={sidebarOpen ? 'Close sidebar' : 'Open sidebar'}
          aria-pressed={sidebarOpen}
          className={`mt-4 inline-flex h-9 w-9 items-center justify-center rounded-lg transition ${isDark
            ? 'text-zinc-400 hover:bg-zinc-900 hover:text-zinc-100'
            : 'text-slate-600 hover:bg-stone-200 hover:text-slate-950'
            }`}
          onClick={(event) => {
            event.stopPropagation();
            setSidebarOpen((current) => !current);
          }}
          title={sidebarOpen ? 'Close sidebar' : 'Open sidebar'}
          type="button"
        >
          {sidebarOpen ? <PanelLeftClose size={18} /> : <PanelLeftOpen size={18} />}
        </button>
        <button
          aria-label={isDark ? 'Switch to light mode' : 'Switch to dark mode'}
          className={`mt-2 inline-flex h-9 w-9 items-center justify-center rounded-lg transition ${isDark
            ? 'text-zinc-400 hover:bg-zinc-900 hover:text-zinc-100'
            : 'text-slate-600 hover:bg-stone-200 hover:text-slate-950'
            }`}
          onClick={(event) => {
            event.stopPropagation();
            setTheme((current) => (current === 'dark' ? 'light' : 'dark'));
          }}
          title={isDark ? 'Switch to light mode' : 'Switch to dark mode'}
          type="button"
        >
          {isDark ? <Sun size={17} /> : <Moon size={17} />}
        </button>
        <button
          aria-label="Open settings"
          className={`aegis-accent-ghost mt-2 inline-flex h-9 w-9 items-center justify-center rounded-lg border border-transparent transition ${settingsOpen
              ? isDark
                ? 'aegis-accent-subtle'
                : 'aegis-accent-subtle'
              : isDark
                ? 'text-zinc-400 hover:bg-zinc-900 hover:text-zinc-100'
                : 'text-slate-600 hover:bg-stone-200 hover:text-slate-950'
            }`}
          onClick={(event) => {
            event.stopPropagation();
            openSettings();
          }}
          title="Settings"
          type="button"
        >
          <Settings size={17} />
        </button>
      </nav>

      <aside
        aria-hidden={!sidebarOpen}
        className={`shrink-0 overflow-hidden border-r transition-[width] duration-300 ease-out ${sidebarOpen ? 'w-64' : 'w-0 pointer-events-none'
          } ${isDark ? 'border-zinc-800 bg-zinc-950' : 'border-stone-300 bg-stone-50'}`}
      >
        <div
          className={`flex h-full w-64 shrink-0 flex-col py-4 pl-2 pr-4 transition-opacity duration-150 ease-out ${sidebarOpen ? 'opacity-100 delay-100' : 'opacity-0'
            }`}
        >
          <div className="mb-6">
            <div className="aegis-wordmark">AEGIS</div>
          </div>

          <button
            className="aegis-accent-solid relative mb-4 flex items-center justify-center rounded-lg px-3 py-2 text-xs font-semibold tracking-[0.14em] text-white disabled:opacity-60"
            disabled={isStreaming}
            onClick={handleNewSession}
            type="button"
          >
            <MessageSquare className="absolute left-3" size={15} />
            <span>NEW CONVERSATION</span>
          </button>

          <div className="mb-3">
            <div className="mb-2 flex items-center justify-between">
              <button
                className={`flex min-w-0 items-center gap-1.5 text-[14px] font-semibold transition ${
                  isDark ? 'text-zinc-400 hover:text-zinc-100' : 'text-slate-600 hover:text-slate-950'
                }`}
                onClick={() => setProjectsOpen((current) => !current)}
                type="button"
              >
                <ChevronDown
                  className={`shrink-0 transition-transform ${projectsOpen ? '' : '-rotate-90'}`}
                  size={15}
                />
                <span>Projects</span>
              </button>
              <button
                aria-label="Open project folder"
                className={`rounded-lg p-1.5 transition ${
                  isDark
                    ? 'text-zinc-500 hover:bg-zinc-900 hover:text-emerald-300'
                    : 'text-slate-500 hover:bg-stone-200 hover:text-emerald-700'
                }`}
                disabled={scanningProject}
                onClick={() => void handleAddProject()}
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
                    className={`flex w-full items-center gap-2 rounded-lg border px-3 py-2 text-left text-sm transition ${
                      isDark
                        ? 'border-zinc-800 text-zinc-500 hover:bg-zinc-900'
                        : 'border-stone-300 text-slate-500 hover:bg-stone-100'
                    }`}
                    disabled={scanningProject}
                    onClick={() => void handleAddProject()}
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
                        className={`group flex items-center gap-2 rounded-lg px-2.5 py-2 transition ${
                          isActiveProject
                            ? isDark
                              ? 'bg-zinc-900 text-zinc-50 shadow-[0_3px_12px_rgba(255,255,255,0.10)]'
                              : 'bg-white text-slate-950 shadow-[0_8px_20px_rgba(120,113,108,0.12)]'
                            : isDark
                              ? 'text-zinc-400 hover:bg-zinc-900/70 hover:text-zinc-100'
                              : 'text-slate-600 hover:bg-white hover:text-slate-950'
                        }`}
                        key={project.id}
                      >
                        <button
                          className="flex min-w-0 flex-1 items-center gap-2 text-left"
                          onClick={() => {
                            setActiveProjectId(project.id);
                            setChatMode('coder');
                          }}
                          type="button"
                        >
                          <FolderOpen
                            className={isActiveProject ? 'text-emerald-400' : ''}
                            size={16}
                          />
                          <span className="min-w-0">
                            <span className="block truncate text-sm">{project.name}</span>
                            <span
                              className={`block truncate text-[11px] ${
                                isDark ? 'text-zinc-500' : 'text-slate-500'
                              }`}
                            >
                              {project.fileCount} files · {Math.ceil(project.totalBytes / 1024)} KB
                              {project.writable ? ' · editable' : ' · read-only'}
                            </span>
                          </span>
                        </button>
                        <button
                          aria-label={`Remove ${project.name}`}
                          className={`rounded-md p-1 opacity-0 transition group-hover:opacity-100 ${
                            isDark
                              ? 'text-zinc-500 hover:bg-zinc-800 hover:text-red-300'
                              : 'text-slate-500 hover:bg-stone-100 hover:text-red-600'
                          }`}
                          onClick={() => removeProject(project.id)}
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

          <div
            className="mb-2 flex items-center justify-between"
          >
            <button
              className={`flex items-center gap-1.5 text-[14px] font-semibold transition ${
                isDark ? 'text-zinc-400 hover:text-zinc-100' : 'text-slate-600 hover:text-slate-950'
              }`}
              onClick={() => setSessionsOpen((current) => !current)}
              type="button"
            >
              <ChevronDown
                className={`transition-transform ${sessionsOpen ? '' : '-rotate-90'}`}
                size={15}
              />
              <span>Sessions</span>
            </button>
          </div>

          <div
            className={`sessions-scroll -ml-1.5 -mr-3 min-h-0 flex-1 space-y-1 overflow-y-auto py-1.5 pl-2 pr-3 ${
              sessionsOpen ? '' : 'hidden'
            }`}
          >
            {sessions.length === 0 ? (
              <div
                className={`rounded-lg border p-3 text-sm ${isDark ? 'border-zinc-800 text-zinc-500' : 'border-stone-300 text-slate-500'
                  }`}
              >
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
                  ? isDark
                    ? 'border-transparent bg-red-950/40 text-red-100 opacity-0 scale-95 -translate-x-2'
                    : 'border-transparent bg-red-100 text-red-800 opacity-0 scale-95 -translate-x-2'
                  : isActive && isPinned
                    ? isDark
                      ? 'border-transparent bg-zinc-800/95 text-zinc-50 shadow-[0_3px_10px_rgba(255,255,255,0.16),inset_0_1px_0_rgba(255,255,255,0.16)] ring-1 ring-amber-500/20'
                      : 'border-transparent bg-white text-slate-950 shadow-[0_8px_22px_rgba(120,113,108,0.16)] ring-1 ring-amber-300/55'
                    : isActive
                      ? isDark
                        ? 'border-transparent bg-zinc-800/95 text-zinc-50 shadow-[0_3px_10px_rgba(255,255,255,0.16),inset_0_1px_0_rgba(255,255,255,0.16)]'
                        : 'border-transparent bg-white text-slate-950 shadow-[0_8px_22px_rgba(120,113,108,0.16)]'
                      : isPinned
                        ? isDark
                          ? 'border-transparent bg-zinc-900/75 text-zinc-100 shadow-[0_2px_8px_rgba(255,255,255,0.12)]'
                          : 'border-transparent bg-white/80 text-slate-900 shadow-[0_3px_14px_rgba(120,113,108,0.10)]'
                        : isDark
                          ? 'border-transparent text-zinc-300 shadow-[0_1px_0_rgba(255,255,255,0.09)] hover:bg-zinc-900/85 hover:shadow-[0_3px_9px_rgba(255,255,255,0.14)]'
                          : 'border-transparent text-slate-700 shadow-[0_1px_0_rgba(120,113,108,0.12)] hover:bg-white/80 hover:shadow-[0_8px_20px_rgba(120,113,108,0.12)]';

                return (
                  <div
                    className={`relative w-full rounded-lg border px-2 py-2 text-left transition-all duration-200 ease-out ${isNewSession ? 'animate-[fadeInSession_520ms_ease-out]' : ''
                      } ${cardStateClasses}`}
                    key={session.session_id}
                  >
                    <div className="flex items-center gap-1.5">
                      {editingSessionId === session.session_id ? (
                        <input
                          autoFocus
                          className={`session-title-text min-w-0 flex-1 rounded-lg border px-2 py-1.5 text-[13px] outline-none ${isDark
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
                      ) : (
                        <button
                          className="min-w-0 flex-1 py-1 text-left"
                          disabled={isDeleting}
                          onClick={() => {
                            void handleSessionSelect(session.session_id);
                          }}
                          type="button"
                        >
                          <span className="flex min-w-0 flex-col gap-0.5">
                            <span
                              className="session-title-text truncate text-[13px] leading-5"
                              onDoubleClick={(event) => {
                                event.stopPropagation();
                                beginRenamingSession(session);
                              }}
                            >
                              {session.title}
                            </span>
                            <span
                              className={`truncate text-[11px] leading-4 ${isDark ? 'text-zinc-500' : 'text-slate-500'
                                }`}
                            >
                              {lastAccessedLabel}
                            </span>
                          </span>
                        </button>
                      )}

                      {isPinned && (
                        <span
                          className={`inline-flex shrink-0 items-center justify-center rounded-lg p-1 ${isDark ? 'text-amber-300' : 'text-amber-600'
                            }`}
                          title="Pinned session"
                        >
                          <Pin fill="currentColor" size={14} />
                        </span>
                      )}

                      <button
                        aria-expanded={sessionMenuOpenId === session.session_id}
                        aria-label={`Open actions for ${session.title}`}
                        className={`rounded-lg p-1.5 transition disabled:opacity-50 ${isDark
                          ? 'text-zinc-400 hover:bg-zinc-700/80 hover:text-zinc-100'
                          : 'text-slate-500 hover:bg-stone-100 hover:text-slate-900'
                          }`}
                        disabled={isStreaming || isDeleting}
                        onClick={(event) => {
                          event.stopPropagation();
                          setSessionMenuOpenId((current) =>
                            current === session.session_id ? null : session.session_id,
                          );
                        }}
                        type="button"
                      >
                        <MoreHorizontal size={17} />
                      </button>
                    </div>

                    {sessionMenuOpenId === session.session_id && (
                      <div
                        className={`absolute right-2 z-30 w-40 rounded-xl border p-1 text-sm shadow-xl ${shouldOpenMenuUp ? 'bottom-10' : 'top-10'
                          } ${isDark
                            ? 'border-zinc-800 bg-zinc-950 text-zinc-100 shadow-white/5'
                            : 'border-stone-200 bg-white text-slate-900 shadow-stone-300/50'
                          }`}
                        onClick={(event) => event.stopPropagation()}
                      >
                        <button
                          className={`flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left transition ${isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'
                            }`}
                          onClick={() => beginRenamingSession(session)}
                          type="button"
                        >
                          <Edit3 size={14} />
                          Rename
                        </button>
                        <button
                          className={`flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left transition ${isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'
                            }`}
                          onClick={() => {
                            void exportSessionAsPdf(session);
                          }}
                          type="button"
                        >
                          <Download size={14} />
                          Export chat
                        </button>
                        <button
                          className={`flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left transition ${isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'
                            }`}
                          onClick={() => togglePinnedSession(session.session_id)}
                          type="button"
                        >
                          <Pin fill={isPinned ? 'currentColor' : 'none'} size={14} />
                          {isPinned ? 'Unpin' : 'Pin'}
                        </button>
                        <button
                          className={`flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left font-medium text-red-500 transition ${isDark ? 'hover:bg-red-950/30' : 'hover:bg-red-50'
                            }`}
                          onClick={() => {
                            void handleDeleteSession(session);
                          }}
                          type="button"
                        >
                          <Trash2 size={14} />
                          Delete
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

      <main className="relative flex min-w-0 flex-1 flex-col">
        <header
          className={`flex h-16 shrink-0 items-center justify-between border-b px-6 ${isDark ? 'border-zinc-800' : 'border-stone-300'
            }`}
        >
          <div className="flex min-w-0 items-center gap-3">
            <div className="min-w-0">
              <div className="truncate text-sm font-medium">{activeSession?.title ?? 'New chat'}</div>
              <div className={`truncate text-xs ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>
                Session: {activeSessionId ?? 'Not started yet'}
              </div>
            </div>
          </div>

          <div className="flex items-center gap-1 rounded-xl border border-zinc-800/50 bg-zinc-900/30 p-1 shadow-inner backdrop-blur-sm">
            <button
              className={`flex items-center gap-2 rounded-lg px-3 py-1.5 text-xs font-medium transition-all ${chatMode === 'general'
                ? 'aegis-accent-chip-active text-white'
                : 'text-zinc-400 hover:bg-zinc-800/50 hover:text-zinc-200'
                }`}
              onClick={() => setChatMode('general')}
              type="button"
            >
              <Bot size={14} />
              General
            </button>
            <button
              className={`flex items-center gap-2 rounded-lg px-3 py-1.5 text-xs font-medium transition-all ${chatMode === 'coder'
                ? 'aegis-accent-chip-active text-white'
                : 'text-zinc-400 hover:bg-zinc-800/50 hover:text-zinc-200'
                }`}
              onClick={() => setChatMode('coder')}
              type="button"
            >
              <Cpu size={14} />
              Coder
            </button>
            <button
              className={`flex items-center gap-2 rounded-lg px-3 py-1.5 text-xs font-medium transition-all ${chatMode === 'academic'
                  ? 'aegis-accent-chip-active text-white'
                  : 'text-zinc-400 hover:bg-zinc-800/50 hover:text-zinc-200'
                }`}
              onClick={() => setChatMode('academic')}
              type="button"
            >
              <GraduationCap size={14} />
              Academic
            </button>
          </div>

          <div className="flex items-center gap-3">
            <button
              className={`aegis-accent-ghost inline-flex items-center gap-2 rounded-lg border px-3 py-2 text-xs font-medium transition ${isMetricsOpen
                ? 'aegis-accent-subtle'
                : isDark
                  ? 'border-zinc-800 text-zinc-300 hover:bg-zinc-900'
                  : 'border-stone-300 bg-white text-slate-700 hover:bg-stone-100'
                }`}
              onClick={() => setIsMetricsOpen((current) => !current)}
              type="button"
            >
              <Activity size={14} />
              Metrics
            </button>
            <div
              className={`rounded-lg border px-3 py-1 text-xs ${isDark
                ? 'border-zinc-800 text-zinc-400'
                : 'border-stone-300 bg-white text-slate-500'
                }`}
            >
              {status}
            </div>
          </div>
        </header>

        {visibleResourceWarning && (
          <div
            className={`flex items-center justify-between gap-4 border-b px-6 py-3 text-sm font-medium ${isDark
              ? 'border-amber-900/60 bg-amber-950/30 text-amber-200'
              : 'border-amber-200 bg-amber-50 text-amber-800'
              }`}
          >
            <span className="min-w-0 flex-1">Warning: {visibleResourceWarning}</span>
            <button
              aria-label="Dismiss resource warning"
              className={`inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-md transition ${isDark
                ? 'text-amber-200/80 hover:bg-amber-900/40 hover:text-amber-100'
                : 'text-amber-700/80 hover:bg-amber-100 hover:text-amber-900'
                }`}
              onClick={() => setDismissedResourceWarning(visibleResourceWarning)}
              title="Dismiss warning"
              type="button"
            >
              <X size={15} />
            </button>
          </div>
        )}

        {error && (
          <div
            className={`flex items-center justify-between gap-4 border-b px-6 py-3 text-sm ${isDark
              ? 'border-red-900/60 bg-red-950/30 text-red-200'
              : 'border-red-200 bg-red-50 text-red-700'
              }`}
            role="alert"
          >
            <span className="min-w-0 flex-1">{error}</span>
            {errorDismissible && (
              <button
                aria-label="Dismiss error"
                className={`inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-md transition ${isDark
                  ? 'text-red-200/80 hover:bg-red-900/40 hover:text-red-100'
                  : 'text-red-700/80 hover:bg-red-100 hover:text-red-900'
                  }`}
                onClick={() => setError(null)}
                title="Dismiss error"
                type="button"
              >
                <X size={15} />
              </button>
            )}
          </div>
        )}

        <div
          ref={scrollRef}
          className={`min-h-0 flex-1 overflow-y-auto px-6 pb-12 pt-6 ${isDark
            ? 'bg-zinc-950'
            : 'bg-[radial-gradient(circle_at_top,_rgba(255,255,255,0.75),_rgba(245,245,244,0)_42%)]'
            }`}
        >
          <div className="mx-auto flex max-w-4xl flex-col gap-4">
            {messages.length === 0 ? null : (
              messages.map((message, index) => (
                <div
                  className={`flex gap-3 ${message.role === 'user' ? 'justify-end' : 'justify-start'}`}
                  key={`${message.role}-${index}`}
                >
                  {message.role === 'assistant' && (
                    <div
                      className={`mt-1 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg ${isDark
                        ? 'bg-zinc-800 text-zinc-200 shadow-sm shadow-white/5'
                        : 'bg-white text-slate-700 shadow-sm shadow-stone-300/70 ring-1 ring-stone-200'
                        }`}
                    >
                      <Bot size={16} />
                    </div>
                  )}
                  <div
                    className={`group flex max-w-[78%] flex-col ${message.role === 'user' ? 'items-end' : 'items-start'
                      }`}
                  >
                    {editingMessageIndex === index && message.role === 'user' ? (
                      <div
                        className={`w-[min(32rem,78vw)] rounded-lg border p-2.5 shadow-sm ${isDark
                          ? 'border-emerald-700 bg-zinc-900'
                          : 'border-emerald-500 bg-white'
                          }`}
                      >
                        <textarea
                          autoFocus
                          className={`mb-2 max-h-56 min-h-11 w-full resize-none overflow-hidden rounded-md border px-3 py-2.5 text-sm leading-5 outline-none focus:border-emerald-600 ${isDark
                            ? 'border-zinc-800 bg-zinc-950 text-zinc-100'
                            : 'border-stone-300 bg-white text-slate-900'
                            }`}
                          onChange={(event) => {
                            setEditingMessageText(event.target.value);
                            fitTextareaToContent(event.currentTarget);
                          }}
                          ref={(textarea) => {
                            if (textarea) {
                              fitTextareaToContent(textarea);
                            }
                          }}
                          rows={1}
                          value={editingMessageText}
                        />
                        <div className="flex justify-end gap-2">
                          <button
                            className={`rounded-md border px-3 py-1.5 text-xs ${isDark
                              ? 'border-zinc-800 text-zinc-300 hover:bg-zinc-800'
                              : 'border-stone-300 text-slate-700 hover:bg-stone-100'
                              }`}
                            onClick={cancelEditingMessage}
                            type="button"
                          >
                            Cancel
                          </button>
                          <button
                            className="rounded-md bg-emerald-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-emerald-500 disabled:opacity-60"
                            disabled={!editingMessageText.trim() || isStreaming}
                            onClick={() => void resendEditedMessage(index)}
                            type="button"
                          >
                            Resend
                          </button>
                        </div>
                      </div>
                    ) : (
                      <div
                        className={`rounded-lg px-4 py-3 text-sm leading-6 shadow-sm ${message.role === 'user'
                          ? isDark
                            ? 'bg-emerald-600 text-white shadow-[0_8px_22px_rgba(255,255,255,0.07)]'
                            : 'bg-emerald-600 text-white shadow-[0_10px_24px_rgba(16,185,129,0.24)]'
                          : isDark
                            ? 'border border-zinc-800 bg-zinc-900 text-zinc-200 shadow-[0_8px_22px_rgba(255,255,255,0.065)]'
                            : 'border border-stone-200 bg-white/95 text-slate-800 shadow-[0_10px_26px_rgba(120,113,108,0.20)]'
                          }`}
                      >
                        {message.role === 'assistant' ? (
                          message.content ? (
                            <AssistantMarkdown content={message.content} />
                          ) : (
                            <ThinkingIndicator isDark={isDark} />
                          )
                        ) : (
                          <span className="whitespace-pre-wrap">{message.content || '...'}</span>
                        )}
                      </div>
                    )}
                    {message.role === 'assistant' && message.content && (
                      <div className="mt-1 flex items-center gap-1 opacity-60 hover:opacity-100 focus-within:opacity-100 transition-all duration-150">
                        {message.sources && message.sources.length > 0 && (
                          <button
                            aria-label="Inspect retrieved sources"
                            className={`inline-flex h-7 w-7 items-center justify-center rounded-md transition ${
                              selectedMessageSourcesIndex === index
                                ? isDark
                                  ? 'text-emerald-400 bg-zinc-900 border border-emerald-500/20'
                                  : 'text-emerald-600 bg-stone-200 border border-emerald-300/30'
                                : isDark
                                  ? 'text-zinc-500 hover:bg-zinc-900 hover:text-emerald-300'
                                  : 'text-slate-500 hover:bg-stone-200 hover:text-emerald-700'
                            }`}
                            onClick={() => {
                              if (selectedMessageSourcesIndex === index) {
                                setSelectedMessageSources(null);
                                setSelectedMessageSourcesIndex(null);
                                setMetricsTab('metrics');
                              } else {
                                setSelectedMessageSourcesIndex(index);
                                setSelectedMessageSources(message.sources || null);
                                setMetricsTab('sources');
                                setIsMetricsOpen(true);
                              }
                            }}
                            title={`Inspect ${message.sources.length} retrieved sources`}
                            type="button"
                          >
                            <BookOpen size={13} className={selectedMessageSourcesIndex === index ? 'animate-pulse' : ''} />
                          </button>
                        )}
                        <button
                          aria-label={speakingMessageIndex === index ? 'Stop reading' : 'Read aloud'}
                          className={`inline-flex h-7 w-7 items-center justify-center rounded-md transition ${isDark
                            ? speakingMessageIndex === index
                              ? 'text-emerald-400 bg-zinc-900'
                              : 'text-zinc-500 hover:bg-zinc-900 hover:text-emerald-300'
                            : speakingMessageIndex === index
                              ? 'text-emerald-600 bg-stone-200'
                              : 'text-slate-500 hover:bg-stone-200 hover:text-emerald-700'
                            }`}
                          onClick={() => {
                            void speakAssistantResponse(message.content, true, index);
                          }}
                          title={speakingMessageIndex === index ? 'Stop reading' : 'Read aloud'}
                          type="button"
                        >
                          {speakingMessageIndex === index ? (
                            <VolumeX size={13} className="animate-pulse" />
                          ) : (
                            <Volume2 size={13} />
                          )}
                        </button>
                        <button
                          aria-label="Copy response"
                          className={`inline-flex h-7 w-7 items-center justify-center rounded-md transition ${isDark
                            ? 'text-zinc-500 hover:bg-zinc-900 hover:text-emerald-300'
                            : 'text-slate-500 hover:bg-stone-200 hover:text-emerald-700'
                            }`}
                          onClick={() => {
                            void copyTextToClipboard(message.content);
                            setCopiedMessageIndex(index);
                            window.setTimeout(() => {
                              setCopiedMessageIndex((current) => (current === index ? null : current));
                            }, 1400);
                          }}
                          title={copiedMessageIndex === index ? 'Copied' : 'Copy response'}
                          type="button"
                        >
                          {copiedMessageIndex === index ? <Check size={13} /> : <Copy size={13} />}
                        </button>
                      </div>
                    )}
                    {message.role === 'user' && editingMessageIndex !== index && (
                      <div className="mt-1 flex items-center gap-1 opacity-60 hover:opacity-100 focus-within:opacity-100 transition-all duration-150">
                        <button
                          aria-label="Edit message"
                          className={`inline-flex h-7 w-7 items-center justify-center rounded-md transition ${isDark
                            ? 'text-zinc-500 hover:bg-zinc-900 hover:text-emerald-300'
                            : 'text-slate-500 hover:bg-stone-200 hover:text-emerald-700'
                            }`}
                          disabled={isStreaming}
                          onClick={() => beginEditingMessage(index, message.content)}
                          title="Edit message"
                          type="button"
                        >
                          <Edit3 size={13} />
                        </button>
                        <button
                          aria-label="Copy message"
                          className={`inline-flex h-7 w-7 items-center justify-center rounded-md transition ${isDark
                            ? 'text-zinc-500 hover:bg-zinc-900 hover:text-emerald-300'
                            : 'text-slate-500 hover:bg-stone-200 hover:text-emerald-700'
                            }`}
                          onClick={() => {
                            void copyUserMessage(index, message.content);
                          }}
                          title={copiedMessageIndex === index ? 'Copied' : 'Copy message'}
                          type="button"
                        >
                          {copiedMessageIndex === index ? <Check size={13} /> : <Copy size={13} />}
                        </button>
                      </div>
                    )}
                    {message.role === 'assistant' &&
                      activeProject &&
                      Boolean(extractUnifiedDiff(message.content)) && (
                        <button
                          className={`mt-2 inline-flex items-center gap-2 rounded-lg border px-3 py-1.5 text-xs font-medium transition ${
                            activeProject.writable
                              ? isDark
                                ? 'border-emerald-700 text-emerald-200 hover:bg-emerald-950/40'
                                : 'border-emerald-300 text-emerald-700 hover:bg-emerald-50'
                              : isDark
                                ? 'border-zinc-800 text-zinc-500'
                                : 'border-stone-300 text-slate-500'
                          }`}
                          disabled={!activeProject.writable}
                          onClick={() => void applyAssistantPatch(message.content)}
                          title={
                            activeProject.writable
                              ? 'Apply the unified diff to the active project'
                              : 'Grant project edit permission before applying patches'
                          }
                          type="button"
                        >
                          <FileCode size={14} />
                          Apply suggested patch
                        </button>
                      )}

                  </div>
                  {message.role === 'user' && (
                    <div
                      className={`mt-1 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg shadow-sm ${isDark
                        ? 'bg-emerald-700 text-white shadow-white/5'
                        : 'bg-emerald-50 text-emerald-700 shadow-emerald-100 ring-1 ring-emerald-200'
                        }`}
                    >
                      <User size={16} />
                    </div>
                  )}
                </div>
              ))
            )}
          </div>
        </div>

        <footer
          className={`px-4 transition-all duration-500 ease-out ${
            showCenteredComposer
              ? 'pointer-events-none absolute inset-x-0 top-1/2 z-20 -translate-y-1/2 pb-0 pt-0'
              : `relative shrink-0 pb-4 pt-5 ${
                  isDark
                    ? 'bg-zinc-950/95 shadow-[0_-24px_42px_rgba(0,0,0,0.35)]'
                    : 'bg-stone-100/95 shadow-[0_-24px_42px_rgba(120,113,108,0.18)]'
                }`
          }`}
        >
          {!showCenteredComposer && (
            <div
              className={`pointer-events-none absolute inset-x-0 -top-8 h-8 ${isDark
                  ? 'bg-gradient-to-t from-zinc-950/95 to-transparent'
                  : 'bg-gradient-to-t from-stone-100/95 to-transparent'
                }`}
            />
          )}
          {showCenteredComposer && (
            <div
              className={`welcome-message pointer-events-auto mx-auto mb-5 max-w-2xl text-center text-xl font-semibold ${
                isDark ? 'text-zinc-100' : 'text-slate-900'
              }`}
            >
              {personalizeWelcomeMessage(activeWelcomeMessage, profileText)}
            </div>
          )}
          {showImportProgress && (
            <div
              className={`mx-auto mb-3 max-w-3xl rounded-lg border px-3 py-2 ${importPhase === 'error'
                ? isDark
                  ? 'border-red-900/70 bg-red-950/20 text-red-200'
                  : 'border-red-200 bg-red-50 text-red-800'
                : isDark
                  ? 'border-zinc-800 bg-zinc-900/80 text-zinc-200'
                  : 'border-stone-300 bg-white text-slate-700'
                }`}
            >
              <div className="mb-2 flex items-center justify-between gap-3 text-xs">
                <span className="truncate">{importPhaseLabel(importPhase, importFileLabel)}</span>
                <span className="font-mono">{importProgress}%</span>
              </div>
              <div
                className={`h-1.5 overflow-hidden rounded-full ${isDark ? 'bg-zinc-800' : 'bg-stone-200'
                  }`}
                role="progressbar"
                aria-label="Document import progress"
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={importProgress}
              >
                <div
                  className={`h-full rounded-full transition-all duration-300 ${importPhase === 'error'
                    ? 'bg-red-500'
                    : importPhase === 'complete'
                      ? 'bg-emerald-500'
                      : 'bg-emerald-400'
                    } ${importPhase === 'indexing' ? 'animate-pulse' : ''}`}
                  style={{ width: `${importProgress}%` }}
                />
              </div>
            </div>
          )}
          {indexedDocuments.length > 0 && (
            <div
              aria-busy={isClearingIndexedDocuments}
              className={`group relative mx-auto mb-3 flex max-w-3xl items-center gap-2 rounded-lg border py-2 pl-3 pr-9 text-xs transition-all duration-[1800ms] ease-out ${
                isClearingIndexedDocuments
                  ? isDark
                    ? 'border-red-900/70 bg-red-950/30 text-red-200 opacity-35'
                    : 'border-red-300 bg-red-50 text-red-700 opacity-35'
                  : isDark
                    ? 'border-emerald-900/60 bg-emerald-950/20 text-emerald-200 opacity-100'
                    : 'border-emerald-200 bg-emerald-50 text-emerald-800 opacity-100'
              }`}
            >
              <Upload className="shrink-0" size={14} />
              <span className="min-w-0 truncate">
                Document context active: {indexedDocumentLabel} indexed into {indexedChunkCount}{' '}
                chunks.
              </span>
              {!isClearingIndexedDocuments && (
                <button
                  aria-label="Remove imported document context"
                  className={`absolute right-2 top-1/2 inline-flex h-6 w-6 -translate-y-1/2 items-center justify-center rounded-md opacity-0 transition group-hover:opacity-100 group-focus-within:opacity-100 disabled:cursor-not-allowed ${
                    isDark
                      ? 'text-emerald-100/80 hover:bg-emerald-900/40 hover:text-emerald-50 disabled:text-emerald-200/45'
                      : 'text-emerald-800/70 hover:bg-emerald-100 hover:text-emerald-950 disabled:text-emerald-700/45'
                  }`}
                  disabled={isUploading || isStreaming}
                  onClick={() => {
                    void clearIndexedDocuments();
                  }}
                  title="Remove imported document context"
                  type="button"
                >
                  <X size={14} />
                </button>
              )}
            </div>
          )}
          {documentContextNotice && (
            <div
              className={`mx-auto mb-3 flex max-w-3xl items-start gap-2 rounded-lg border px-3 py-2 text-xs ${
                isDark
                  ? 'border-zinc-800 bg-zinc-900/70 text-zinc-300'
                  : 'border-stone-300 bg-white text-slate-600'
              }`}
            >
              <Check className="mt-0.5 shrink-0" size={14} />
              <span className="min-w-0">{documentContextNotice}</span>
            </div>
          )}
          {activeProject && (
            <div
              className={`mx-auto mb-3 flex max-w-3xl items-center justify-between gap-3 rounded-lg border px-3 py-2 text-xs ${
                isDark
                  ? 'border-sky-900/60 bg-sky-950/20 text-sky-200'
                  : 'border-sky-200 bg-sky-50 text-sky-800'
              }`}
            >
              <span className="flex min-w-0 items-center gap-2">
                <FolderOpen size={14} />
                <span className="truncate">
                  Project context active: {activeProject.name} · {activeProject.fileCount} files ·{' '}
                  {activeProject.writable ? 'edits require patch approval' : 'read-only'}
                </span>
              </span>
              <button
                aria-label="Detach project context"
                className={`shrink-0 rounded-md p-1 transition ${
                  isDark ? 'hover:bg-sky-900/40' : 'hover:bg-sky-100'
                }`}
                onClick={() => setActiveProjectId(null)}
                type="button"
              >
                <X size={14} />
              </button>
            </div>
          )}
          {projectEditMessage && (
            <div
              className={`mx-auto mb-3 max-w-3xl rounded-lg border px-3 py-2 text-xs ${
                isDark
                  ? 'border-zinc-800 bg-zinc-900/70 text-zinc-300'
                  : 'border-stone-300 bg-white text-slate-600'
              }`}
            >
              {projectEditMessage}
            </div>
          )}
          <form
            className={`pointer-events-auto mx-auto transition-all duration-500 ease-out ${
              showCenteredComposer ? 'max-w-2xl' : 'max-w-3xl'
            }`}
            onSubmit={handleSubmit}
          >
            <input
              accept=".pdf,.txt"
              className="hidden"
              disabled={isStreaming || isUploading}
              multiple
              onChange={(event) => void handleFileUpload(event)}
              ref={fileInputRef}
              title="Supported files: PDF, TXT"
              type="file"
            />
            <div
              className={`border shadow-sm transition-all duration-500 ease-out ${
                showCenteredComposer
                  ? 'rounded-[1.75rem] px-4 pb-3 pt-3'
                  : 'rounded-xl px-3 pb-2.5 pt-3'
              } ${
                isDark
                  ? 'border-zinc-800 bg-zinc-950/92 text-zinc-100 shadow-black/30'
                  : 'border-stone-300 bg-white text-slate-900 shadow-stone-300/30'
                }`}
            >
              <textarea
                className={`w-full resize-none bg-transparent text-sm leading-6 outline-none ${
                  showCenteredComposer ? 'max-h-28 min-h-[30px]' : 'max-h-44 min-h-[38px]'
                } ${
                  isDark
                    ? 'placeholder:text-zinc-500'
                    : 'placeholder:text-slate-400'
                  }`}
                disabled={isStreaming}
                onChange={(event) => setInput(event.target.value)}
                onInput={(event) => fitTextareaToContent(event.currentTarget)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter' && !event.shiftKey) {
                    event.preventDefault();
                    event.currentTarget.form?.requestSubmit();
                  }
                }}
                placeholder="Message your model..."
                ref={composerTextareaRef}
                rows={1}
                value={input}
              />

              <div className="mt-2 flex items-center justify-between gap-3">
                <div className="relative">
                  <button
                    aria-expanded={toolsOpen}
                    className={`aegis-accent-ghost inline-flex items-center gap-2 rounded-lg border px-2.5 py-2 text-[11px] font-semibold uppercase tracking-[0.16em] transition-all duration-200 ${isStreaming ? 'cursor-not-allowed opacity-60' : ''
                      } ${toolsOpen ? '-translate-y-0.5 scale-[0.98]' : 'translate-y-0 scale-100'} ${toolsOpen
                        ? 'aegis-accent-subtle'
                        : isDark
                          ? 'border-transparent text-zinc-500 hover:bg-zinc-800'
                          : 'border-transparent text-slate-500 hover:bg-stone-100'
                      }`}
                    disabled={isStreaming}
                    onClick={() => setToolsOpen((current) => !current)}
                    type="button"
                  >
                    <Wrench
                      className={toolsOpen ? 'rotate-12 transition-transform' : 'transition-transform'}
                      size={15}
                    />
                    <span>Tools</span>
                    <ChevronDown
                      className={`transition-transform duration-200 ${toolsOpen ? 'rotate-180' : 'rotate-0'}`}
                      size={13}
                    />
                  </button>
                  {toolsOpen && (
                    <div
                      className={`absolute bottom-full left-0 z-20 mb-2 w-48 animate-[toolsMenuIn_160ms_ease-out] rounded-lg border p-1 shadow-xl ${isDark
                          ? 'border-zinc-800 bg-zinc-950 text-zinc-100'
                          : 'border-stone-300 bg-white text-slate-900'
                        }`}
                    >
                      <button
                        className={`flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm ${isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'
                          }`}
                        disabled={isStreaming || isUploading}
                        onClick={handleImportToolClick}
                        type="button"
                      >
                        <Upload size={15} />
                        Import
                      </button>
                      <button
                        className={`flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm disabled:opacity-50 ${isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'
                          }`}
                        onClick={openCalendarTool}
                        type="button"
                      >
                        <Calendar size={15} />
                        Calendar
                      </button>
                      <button
                        className={`flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm disabled:opacity-50 ${isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'
                          }`}
                        disabled={messages.length === 0}
                        onClick={exportChatAsPdf}
                        type="button"
                      >
                        <Download size={15} />
                        Export Chat
                      </button>
                    </div>
                  )}
                </div>

                <div className="flex items-center gap-3">
                  <span
                    className={`font-mono text-[11px] ${isDark ? 'text-zinc-600' : 'text-slate-400'
                      }`}
                    title={`${contextUsage.model || 'Active model'} context usage from the last completed inference`}
                  >
                    {tokenMeterLabel}
                  </span>
                  <button
                    className={`inline-flex h-9 w-9 items-center justify-center rounded-lg transition-all duration-200 ${isVoiceMode
                        ? 'aegis-accent-chip-active text-white'
                        : isDark
                          ? 'text-zinc-500 hover:bg-zinc-800 hover:text-emerald-400'
                          : 'text-slate-500 hover:bg-stone-100 hover:text-emerald-600'
                      }`}
                    onClick={() => setIsVoiceMode(true)}
                    type="button"
                    title="Voice Mode"
                  >
                    <Mic size={19} />
                  </button>
                  <button
                    className="aegis-accent-solid inline-flex items-center gap-2 rounded-lg px-3.5 py-2 text-xs font-semibold uppercase tracking-[0.12em] text-white disabled:opacity-60"
                    disabled={isStreaming || !input.trim() || isUploading}
                    type="submit"
                  >
                    <span>Send</span>
                    <Send size={15} />
                  </button>
                </div>
              </div>
            </div>
          </form>
        </footer>

        {/* VOICE MODE OVERLAY */}
        {isVoiceMode && (
          <div className={`fixed inset-0 z-50 flex flex-col items-center justify-center p-6 backdrop-blur-xl transition-all duration-500 ${isDark ? 'bg-zinc-950/80' : 'bg-white/80'
            }`}>
            <button
              onClick={() => setIsVoiceMode(false)}
              className={`absolute top-6 right-6 p-2 rounded-full transition ${isDark ? 'text-zinc-500 hover:bg-zinc-900 hover:text-zinc-100' : 'text-slate-400 hover:bg-stone-100 hover:text-slate-800'
                }`}
            >
              <X size={24} />
            </button>

            <VoiceOrb
              isListening={isRecording}
              isSpeaking={isSpeaking}
              isProcessing={isTranscribing || isStreaming}
              analyser={analyser}
              isDark={isDark}
            />

            {/* Real-time speech and query display */}
            {(() => {
              const lastUserMessage = [...messages].reverse().find((m) => m.role === 'user');
              const lastAssistantMessage = [...messages].reverse().find((m) => m.role === 'assistant');
              
              return (
                <div className="mt-4 flex flex-col items-center gap-4 max-w-2xl px-4 text-center">
                  {lastUserMessage && (
                    <p className={`text-sm italic font-medium px-4 py-2 rounded-lg max-w-lg ${
                      isDark ? 'text-zinc-300 bg-zinc-900/40' : 'text-slate-700 bg-stone-100/40'
                    }`}>
                      "{lastUserMessage.content}"
                    </p>
                  )}
                  {lastAssistantMessage && lastAssistantMessage.content && (
                    <div className={`w-full max-h-48 overflow-y-auto rounded-xl p-4 text-left text-sm border shadow-inner ${
                      isDark 
                        ? 'bg-zinc-900/60 border-zinc-800 text-zinc-200' 
                        : 'bg-stone-50/60 border-stone-200 text-slate-800'
                    }`}>
                      <p className="whitespace-pre-wrap leading-relaxed">
                        {lastAssistantMessage.content}
                      </p>
                    </div>
                  )}
                </div>
              );
            })()}

            <div className="mt-8 flex flex-col items-center gap-6">
              <button
                onClick={() => {
                  if (isRecording) {
                    void handleStopDictation();
                  } else {
                    void startRecording();
                  }
                }}
                className={`group relative flex h-20 w-20 items-center justify-center rounded-full transition-all duration-300 ${isRecording
                    ? 'bg-red-500 shadow-[0_0_40px_rgba(239,68,68,0.4)] scale-110'
                    : 'bg-emerald-600 shadow-[0_0_30px_rgba(16,185,129,0.3)] hover:scale-105'
                  }`}
              >
                {isRecording ? <X size={32} className="text-white" /> : <Mic size={32} className="text-white" />}
                {isRecording && (
                  <span className="absolute inset-0 animate-ping rounded-full bg-red-500/40" />
                )}
              </button>

              <div className="flex items-center gap-4">
                <button
                  onClick={() => setIsTtsEnabled(!isTtsEnabled)}
                  className={`flex items-center gap-2 rounded-lg px-4 py-2 text-xs font-medium transition ${isDark ? 'bg-zinc-900 text-zinc-400 hover:text-zinc-200' : 'bg-stone-100 text-slate-500 hover:text-slate-800'
                    }`}
                >
                  {isTtsEnabled ? <Volume2 size={16} /> : <VolumeX size={16} />}
                  {isTtsEnabled ? 'Speech On' : 'Speech Off'}
                </button>
              </div>
            </div>
          </div>
        )}
      </main>

      {projectPermissionRequestId && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
          onClick={() => setProjectPermissionRequestId(null)}
        >
          <div
            className={`w-full max-w-md rounded-2xl border p-6 shadow-2xl ${
              isDark
                ? 'border-zinc-800 bg-zinc-950 text-zinc-100'
                : 'border-stone-300 bg-white text-slate-900'
            }`}
            onClick={(event) => event.stopPropagation()}
          >
            <div className="mb-3 flex items-center gap-2 text-lg font-semibold">
              <FolderOpen size={18} />
              Allow Project Edits?
            </div>
            <p className={`text-sm leading-6 ${isDark ? 'text-zinc-400' : 'text-slate-600'}`}>
              AEGIS has scanned this project for context. To edit files, it must request browser
              write permission, and patches will still require your explicit approval before they
              are applied.
            </p>
            <div className="mt-6 flex justify-end gap-2">
              <button
                className={`rounded-lg border px-4 py-2 text-sm transition ${
                  isDark
                    ? 'border-zinc-800 text-zinc-300 hover:bg-zinc-900'
                    : 'border-stone-300 text-slate-700 hover:bg-stone-100'
                }`}
                onClick={() => {
                  setProjectPermissionRequestId(null);
                  setProjectEditMessage('Project attached in read-only mode.');
                }}
                type="button"
              >
                Keep read-only
              </button>
              <button
                className="rounded-lg bg-emerald-600 px-4 py-2 text-sm font-medium text-white transition hover:bg-emerald-500"
                onClick={() => void requestProjectWritePermission(projectPermissionRequestId)}
                type="button"
              >
                Request edit access
              </button>
            </div>
          </div>
        </div>
      )}

      {sessionPendingDeletion && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
          onClick={() => setSessionPendingDeletion(null)}
        >
          <div
            className={`w-full max-w-md rounded-2xl border p-6 shadow-2xl ${
              isDark
                ? 'border-zinc-800 bg-zinc-950 text-zinc-100'
                : 'border-stone-300 bg-white text-slate-900'
            }`}
            onClick={(event) => event.stopPropagation()}
          >
            <div className="mb-3 flex items-center justify-between gap-4">
              <div className="text-lg font-semibold">Delete Conversation</div>
              <button
                aria-label="Cancel deletion"
                className={`rounded-md p-1 transition ${
                  isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'
                }`}
                onClick={() => setSessionPendingDeletion(null)}
                type="button"
              >
                <X size={18} />
              </button>
            </div>
            <p className={`text-sm leading-6 ${isDark ? 'text-zinc-400' : 'text-slate-600'}`}>
              This will permanently delete "{sessionPendingDeletion.title}" and its saved
              conversation history. This action cannot be undone.
            </p>
            <div className="mt-6 flex justify-end gap-2">
              <button
                className={`rounded-lg border px-4 py-2 text-sm transition ${
                  isDark
                    ? 'border-zinc-800 text-zinc-300 hover:bg-zinc-900'
                    : 'border-stone-300 text-slate-700 hover:bg-stone-100'
                }`}
                onClick={() => setSessionPendingDeletion(null)}
                type="button"
              >
                Cancel
              </button>
              <button
                className="rounded-lg bg-red-600 px-4 py-2 text-sm font-medium text-white transition hover:bg-red-500"
                onClick={() => void confirmDeleteSession()}
                type="button"
              >
                Delete permanently
              </button>
            </div>
          </div>
        </div>
      )}

      {settingsOpen && (
        <div
          className={`fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4 ${
            settingsClosing ? 'aegis-modal-backdrop-out' : 'aegis-modal-backdrop'
          }`}
          onClick={closeSettings}
        >
          <div
            className={`flex h-[64vh] min-h-[420px] w-full max-w-4xl overflow-hidden rounded-2xl border shadow-2xl ${
              settingsClosing ? 'aegis-modal-panel-out' : 'aegis-modal-panel'
            } ${
              isDark
                ? 'border-zinc-800 bg-zinc-950 text-zinc-100'
                : 'border-stone-300 bg-white text-slate-900'
            }`}
            onClick={(event) => event.stopPropagation()}
          >
            <aside
              className={`w-48 shrink-0 border-r p-4 ${
                isDark ? 'border-zinc-800 bg-zinc-950' : 'border-stone-200 bg-stone-50'
              }`}
            >
              <div className="mb-4 flex items-center gap-2 text-sm font-semibold">
                <Settings size={16} />
                Settings
              </div>
              {[
                ['general', 'General'],
                ['inference', 'Inference'],
                ['models', 'Models'],
                ['voice', 'Voice'],
                ['rag', 'RAG'],
                ['personalize', 'Personalize'],
              ].map(([value, label]) => (
                <button
                  className={`mb-1 flex w-full items-center rounded-lg px-3 py-2 text-left text-sm transition ${
                    settingsTab === value
                      ? 'aegis-accent-solid text-white'
                      : isDark
                        ? 'text-zinc-400 hover:bg-zinc-900 hover:text-zinc-100'
                        : 'text-slate-600 hover:bg-stone-200 hover:text-slate-950'
                  }`}
                  key={value}
                  onClick={() => setSettingsTab(value as SettingsTab)}
                  type="button"
                >
                  {label}
                </button>
              ))}
            </aside>

            <section className="flex min-w-0 flex-1 flex-col">
              <div
                className={`flex h-14 shrink-0 items-center justify-between px-5 ${
                  isDark ? 'border-zinc-800' : 'border-stone-200'
                }`}
              >
                <div>
                  <div className="text-sm font-semibold capitalize">{settingsTab}</div>
                  <div className={`text-xs ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>
                    {settingsLoading ? 'Loading settings...' : 'Local AEGIS preferences'}
                  </div>
                </div>
                <button
                  aria-label="Close settings"
                  className={`rounded-md p-1 transition ${
                    isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'
                  }`}
                  onClick={closeSettings}
                  type="button"
                >
                  <X size={18} />
                </button>
              </div>

              {settingsMessage && (
                <div
                  className={`mx-5 mb-2 rounded-lg border px-3 py-2 text-xs ${
                    settingsMessage.toLowerCase().includes('could not') ||
                    settingsMessage.toLowerCase().includes('failed') ||
                    settingsMessage.toLowerCase().includes('only')
                      ? isDark
                        ? 'border-red-900/60 bg-red-950/30 text-red-200'
                        : 'border-red-200 bg-red-50 text-red-700'
                      : isDark
                        ? 'border-emerald-900/60 bg-emerald-950/20 text-emerald-200'
                        : 'border-emerald-200 bg-emerald-50 text-emerald-800'
                  }`}
                >
                  {settingsMessage}
                </div>
              )}

              <div className="settings-scroll min-h-0 flex-1 overflow-y-auto px-5 pb-5">
                {settingsTab === 'general' && (
                  <div className="space-y-5">
                    <div>
                      <label className="mb-2 block text-sm font-semibold" htmlFor="general-model">
                        Active Model
                      </label>
                      <select
                        className={`w-full rounded-lg border px-3 py-2 text-sm outline-none focus:border-emerald-600 ${
                          isDark
                            ? 'border-zinc-800 bg-zinc-900 text-zinc-100'
                            : 'border-stone-300 bg-white text-slate-900'
                        }`}
                        disabled={availableModels.length === 0 || Boolean(downloadingModel)}
                        id="general-model"
                        onChange={(event) => void selectModel(event.target.value)}
                        value={availableModels.find((model) => model.active)?.name ?? ''}
                      >
                        <option value="" disabled>
                          {availableModels.length === 0
                            ? 'No installed models found'
                            : 'Choose active model'}
                        </option>
                        {availableModels.map((model) => (
                          <option key={model.name} value={model.name}>
                            {model.name}
                          </option>
                        ))}
                      </select>
                      <div className={`mt-1 text-xs ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>
                        Switching warms the selected model before the engine commits to it.
                      </div>
                    </div>

                    <div>
                      <div className="mb-2 text-sm font-semibold">Appearance</div>
                      <div className="mb-3 flex flex-wrap gap-2">
                        {(['dark', 'light'] as ThemeMode[]).map((mode) => (
                          <button
                            className={`rounded-lg border px-3 py-2 text-sm transition ${
                              theme === mode
                                ? 'aegis-accent-selected'
                                : isDark
                                  ? 'border-zinc-800 text-zinc-300 hover:bg-zinc-900'
                                  : 'border-stone-300 text-slate-700 hover:bg-stone-100'
                            }`}
                            key={mode}
                            onClick={() => setTheme(mode)}
                            type="button"
                          >
                            {mode === 'dark' ? 'Dark mode' : 'Light mode'}
                          </button>
                        ))}
                      </div>
                      <div className={`mb-3 text-xs ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>
                        Pick a base mode and a color profile for the overall interface.
                      </div>
                      <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
                        {APPEARANCE_THEME_OPTIONS.map((option) => (
                          <button
                            className={`rounded-xl border p-3 text-left transition ${
                              appearanceTheme === option.value
                                ? 'aegis-accent-selected shadow-lg'
                                : isDark
                                  ? 'border-zinc-800 hover:bg-zinc-900'
                                  : 'border-stone-300 hover:bg-stone-50'
                            }`}
                            key={option.value}
                            onClick={() => setAppearanceTheme(option.value)}
                            type="button"
                          >
                            <span
                              className={`mb-3 block h-14 rounded-lg border ${
                                isDark ? 'border-white/10' : 'border-black/5'
                              }`}
                              style={{ background: option.preview }}
                            />
                            <div className="flex items-center justify-between gap-2">
                              <div className="text-sm font-semibold">{option.label}</div>
                              {appearanceTheme === option.value && (
                                <span
                                  className={`rounded-full px-2 py-0.5 text-[10px] uppercase tracking-[0.12em] ${
                                    isDark ? 'bg-white/10 text-zinc-100' : 'bg-black/5 text-slate-700'
                                  }`}
                                >
                                  Active
                                </span>
                              )}
                            </div>
                            <div
                              className={`mt-1 text-xs leading-5 ${
                                isDark ? 'text-zinc-400' : 'text-slate-500'
                              }`}
                            >
                              {option.description}
                            </div>
                          </button>
                        ))}
                      </div>
                      <div className={`mt-2 text-xs ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>
                        Current appearance: {activeAppearanceTheme.label}.
                      </div>
                    </div>

                    <div>
                      <div className="mb-2 text-sm font-semibold">Response Style</div>
                      <div className="grid gap-2 sm:grid-cols-2">
                        {RESPONSE_STYLE_OPTIONS.map((option) => (
                          <button
                            className={`rounded-xl border p-3 text-left transition ${
                              responseStyle === option.value
                                ? 'aegis-accent-selected'
                                : isDark
                                  ? 'border-zinc-800 hover:bg-zinc-900'
                                  : 'border-stone-300 hover:bg-stone-50'
                            }`}
                            key={option.value}
                            onClick={() => setResponseStyle(option.value)}
                            type="button"
                          >
                            <div className="text-sm font-semibold">{option.label}</div>
                            <div
                              className={`mt-1 text-xs leading-5 ${
                                isDark ? 'text-zinc-400' : 'text-slate-500'
                              }`}
                            >
                              {option.description}
                            </div>
                          </button>
                        ))}
                      </div>
                    </div>
                  </div>
                )}

                {settingsTab === 'voice' && (
                  <div className="space-y-5">
                    <div>
                      <div className="mb-2 text-sm font-semibold">Voice Caching & Performance</div>
                      <div className="flex flex-col gap-3">
                        <label className={`flex items-start justify-between rounded-xl border p-4 cursor-pointer transition ${
                          isVoiceLowRamMode
                            ? isDark
                              ? 'border-emerald-500 bg-emerald-950/25 text-emerald-100'
                              : 'border-emerald-500 bg-emerald-50 text-emerald-900'
                            : isDark
                              ? 'border-zinc-800 hover:bg-zinc-900/60'
                              : 'border-stone-300 hover:bg-stone-50'
                        }`}>
                          <div className="flex flex-col gap-1 pr-4">
                            <span className="text-sm font-semibold">Low RAM Mode</span>
                            <span className={`text-xs leading-5 ${isDark ? 'text-zinc-400' : 'text-slate-500'}`}>
                              Automatically unloads Whisper (STT) and Kokoro (TTS) models from system memory immediately after processing each voice prompt.
                              Reduces RAM usage by up to ~470 MB, but slightly increases latency on the next voice input as models must reload.
                            </span>
                          </div>
                          <input
                            type="checkbox"
                            checked={isVoiceLowRamMode}
                            onChange={(event) => void toggleVoiceLowRamMode(event.target.checked)}
                            className="mt-1 h-4 w-4 shrink-0 rounded border-stone-300 text-emerald-600 focus:ring-emerald-500 cursor-pointer"
                          />
                        </label>

                        <label className={`flex items-start justify-between rounded-xl border p-4 cursor-pointer transition ${
                          isTtsEnabled
                            ? isDark
                              ? 'border-emerald-500 bg-emerald-950/25 text-emerald-100'
                              : 'border-emerald-500 bg-emerald-50 text-emerald-900'
                            : isDark
                              ? 'border-zinc-800 hover:bg-zinc-900/60'
                              : 'border-stone-300 hover:bg-stone-50'
                        }`}>
                          <div className="flex flex-col gap-1 pr-4">
                            <span className="text-sm font-semibold">Read Aloud by Default</span>
                            <span className={`text-xs leading-5 ${isDark ? 'text-zinc-400' : 'text-slate-500'}`}>
                              Automatically speak assistant responses out loud using the local high-quality voice agent.
                            </span>
                          </div>
                          <input
                            type="checkbox"
                            checked={isTtsEnabled}
                            onChange={(event) => changeTtsEnabled(event.target.checked)}
                            className="mt-1 h-4 w-4 shrink-0 rounded border-stone-300 text-emerald-600 focus:ring-emerald-500 cursor-pointer"
                          />
                        </label>
                      </div>
                    </div>
                  </div>
                )}

                {settingsTab === 'rag' && (
                  <div className="space-y-5">
                    <div>
                      <div className="mb-2 text-sm font-semibold">Document Context (RAG)</div>
                      <div className="flex flex-col gap-3">
                        <label className={`flex items-start justify-between rounded-xl border p-4 cursor-pointer transition ${
                          isRagEnabled
                            ? isDark
                              ? 'border-emerald-500 bg-emerald-950/25 text-emerald-100'
                              : 'border-emerald-500 bg-emerald-50 text-emerald-900'
                            : isDark
                              ? 'border-zinc-800 hover:bg-zinc-900/60'
                              : 'border-stone-300 hover:bg-stone-50'
                        }`}>
                          <div className="flex flex-col gap-1 pr-4">
                            <span className="text-sm font-semibold">Enable Retrieval-Augmented Generation</span>
                            <span className={`text-xs leading-5 ${isDark ? 'text-zinc-400' : 'text-slate-500'}`}>
                              Inject relevant document excerpts from imported files into the LLM context to answer your questions.
                              If disabled, the model will not read from your document library during chat conversations.
                            </span>
                          </div>
                          <input
                            type="checkbox"
                            checked={isRagEnabled}
                            onChange={(event) => toggleRagEnabled(event.target.checked)}
                            className="mt-1 h-4 w-4 shrink-0 rounded border-stone-300 text-emerald-600 focus:ring-emerald-500 cursor-pointer"
                          />
                        </label>

                        {isRagEnabled && (
                          <>
                            <div className={`rounded-xl border p-4 ${isDark ? 'border-zinc-800' : 'border-stone-200'}`}>
                              <div className="mb-1 flex items-center justify-between">
                                <span className="text-sm font-semibold">Retrieve Limit (Top-K)</span>
                                <span className="text-sm font-bold text-emerald-600">{ragTopK} chunks</span>
                              </div>
                              <span className={`block mb-3 text-xs leading-5 ${isDark ? 'text-zinc-400' : 'text-slate-500'}`}>
                                The maximum number of document passages to retrieve and supply to the AI model per message. Higher values provide more context but consume more memory and tokens.
                              </span>
                              <input
                                type="range"
                                min="1"
                                max="10"
                                step="1"
                                value={ragTopK}
                                onChange={(event) => changeRagTopK(Number(event.target.value))}
                                className="h-2 w-full cursor-pointer appearance-none rounded-lg bg-stone-200 dark:bg-zinc-800 accent-emerald-600"
                              />
                            </div>

                            <div className={`rounded-xl border p-4 ${isDark ? 'border-zinc-800' : 'border-stone-200'}`}>
                              <div className="mb-1 flex items-center justify-between">
                                <span className="text-sm font-semibold">Similarity Cutoff Score</span>
                                <span className="text-sm font-bold text-emerald-600">
                                  {ragSimilarityThreshold === 0.0 ? 'None (Retrieve all)' : `≥ ${ragSimilarityThreshold.toFixed(2)}`}
                                </span>
                              </div>
                              <span className={`block mb-3 text-xs leading-5 ${isDark ? 'text-zinc-400' : 'text-slate-500'}`}>
                                Only inject retrieved passages whose similarity scores exceed this cutoff. Helps filter out irrelevant text noise. A setting of 0.0 disables cutoff filtering.
                              </span>
                              <input
                                type="range"
                                min="0.0"
                                max="0.9"
                                step="0.05"
                                value={ragSimilarityThreshold}
                                onChange={(event) => changeRagThreshold(Number(event.target.value))}
                                className="h-2 w-full cursor-pointer appearance-none rounded-lg bg-stone-200 dark:bg-zinc-800 accent-emerald-600"
                              />
                            </div>
                          </>
                        )}
                      </div>
                    </div>
                  </div>
                )}

                {settingsTab === 'inference' && (
                  <div className="space-y-4">
                    <div>
                      <label className="mb-2 block text-sm font-semibold" htmlFor="provider-select">
                        Inference Provider
                      </label>
                      <select
                        className={`w-full rounded-lg border px-3 py-2 text-sm outline-none focus:border-emerald-600 ${
                          isDark
                            ? 'border-zinc-800 bg-zinc-900 text-zinc-100'
                            : 'border-stone-300 bg-white text-slate-900'
                        }`}
                        disabled={availableProviders.length === 0}
                        id="provider-select"
                        onChange={(event) => void selectProvider(event.target.value)}
                        value={activeProvider?.name ?? ''}
                      >
                        <option value="" disabled>
                          {availableProviders.length === 0
                            ? 'No providers available'
                            : 'Choose provider'}
                        </option>
                        {availableProviders.map((provider) => (
                          <option key={provider.name} value={provider.name}>
                            {provider.name}
                          </option>
                        ))}
                      </select>
                    </div>

                    {activeProvider && (
                      <div
                        className={`rounded-xl border p-4 text-sm ${
                          isDark
                            ? 'border-zinc-800 bg-zinc-900/40 text-zinc-300'
                            : 'border-stone-300 bg-stone-50 text-slate-600'
                        }`}
                      >
                        {activeProvider.description}
                      </div>
                    )}
                  </div>
                )}

                {settingsTab === 'models' && (
                  <div className="space-y-4">
                    <div>
                      <label className="mb-2 block text-sm font-semibold" htmlFor="model-search">
                        Search or Download Ollama Model
                      </label>
                      <div className="flex gap-2">
                        <input
                          className={`min-w-0 flex-1 rounded-lg border px-3 py-2 text-sm outline-none focus:border-emerald-600 ${
                            isDark
                              ? 'border-zinc-800 bg-zinc-900 text-zinc-100 placeholder:text-zinc-500'
                              : 'border-stone-300 bg-white text-slate-900 placeholder:text-slate-400'
                          }`}
                          id="model-search"
                          onChange={(event) => setModelSearch(event.target.value)}
                          placeholder="Search catalog or enter an exact model tag"
                          value={modelSearch}
                        />
                        <button
                          className="rounded-lg bg-emerald-600 px-4 py-2 text-sm font-medium text-white transition hover:bg-emerald-500 disabled:opacity-60"
                          disabled={!modelSearch.trim() || modelDownloadState === 'downloading'}
                          onClick={() => void downloadOllamaModel()}
                          type="button"
                        >
                          Download
                        </button>
                      </div>
                    </div>

                    <div className="space-y-2">
                      <div className="flex flex-wrap gap-2">
                        {MODEL_PROVIDER_TAGS.map((tag) => (
                          <button
                            className={`rounded-full border px-3 py-1.5 text-xs transition ${
                              selectedModelProviderTag === tag
                                ? 'border-emerald-500 bg-emerald-600 text-white'
                                : isDark
                                  ? 'border-zinc-800 text-zinc-400 hover:bg-zinc-900 hover:text-zinc-100'
                                  : 'border-stone-300 text-slate-600 hover:bg-stone-100 hover:text-slate-950'
                            }`}
                            key={tag}
                            onClick={() => setSelectedModelProviderTag(tag)}
                            type="button"
                          >
                            {tag}
                          </button>
                        ))}
                      </div>

                      <div
                        className={`settings-scroll max-h-56 space-y-2 overflow-y-auto rounded-xl border p-2 ${
                          isDark
                            ? 'border-zinc-800 bg-zinc-950/40'
                            : 'border-stone-300 bg-stone-50'
                        }`}
                      >
                        {filteredCatalogModels.length === 0 ? (
                          <div className={`p-3 text-sm ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>
                            No catalog models match this filter.
                          </div>
                        ) : (
                          filteredCatalogModels.map((model) => (
                            <div
                              className={`flex w-full items-start justify-between gap-3 rounded-lg p-3 text-left transition ${
                                modelSearch.trim() === model.name
                                  ? isDark
                                    ? 'bg-emerald-950/30 text-emerald-100'
                                    : 'bg-emerald-50 text-emerald-900'
                                  : isDark
                                    ? 'hover:bg-zinc-900'
                                    : 'hover:bg-white'
                              }`}
                              key={model.name}
                            >
                              <span className="min-w-0">
                                <span className="block truncate font-mono text-sm">{model.name}</span>
                                <span className={`mt-1 block text-xs leading-5 ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>
                                  {model.description}
                                </span>
                                <span className="mt-2 flex flex-wrap gap-1.5">
                                  {[model.provider, ...model.tags].map((tag) => (
                                    <span
                                      className={`rounded-full px-2 py-0.5 text-[10px] ${
                                        isDark
                                          ? 'bg-zinc-800 text-zinc-400'
                                          : 'bg-stone-200 text-slate-600'
                                      }`}
                                      key={`${model.name}-${tag}`}
                                    >
                                      {tag}
                                    </span>
                                  ))}
                                </span>
                              </span>
                              <button
                                aria-label={`Download ${model.name}`}
                                className={`aegis-accent-ghost mt-0.5 inline-flex shrink-0 items-center justify-center rounded-md border border-transparent p-2 transition ${
                                  modelDownloadState === 'downloading'
                                    ? 'cursor-not-allowed opacity-45'
                                    : isDark
                                      ? 'text-zinc-400'
                                      : 'text-slate-500'
                                }`}
                                disabled={modelDownloadState === 'downloading'}
                                onClick={() => void downloadOllamaModel(model.name)}
                                type="button"
                              >
                                <Download size={15} />
                              </button>
                            </div>
                          ))
                        )}
                      </div>
                    </div>

                    {(downloadingModel || pausedModelDownload) && (
                      <div
                        className={`rounded-xl border p-3 ${
                          isDark
                            ? 'border-zinc-800 bg-zinc-900/50'
                            : 'border-stone-300 bg-stone-50'
                        }`}
                      >
                        <div className="mb-2 flex items-center justify-between text-xs">
                          <span className="truncate">
                            {downloadingModel ?? pausedModelDownload}: {modelDownloadStatus}
                          </span>
                          <span className="font-mono">{modelDownloadProgress}%</span>
                        </div>
                        <div className={`h-1.5 rounded-full ${isDark ? 'bg-zinc-800' : 'bg-stone-200'}`}>
                          <div
                            className="h-full rounded-full bg-emerald-500 transition-all duration-300"
                            style={{ width: `${modelDownloadProgress}%` }}
                          />
                        </div>
                        <div className="mt-3 flex justify-end gap-2">
                          {modelDownloadState === 'downloading' ? (
                            <button
                              className={`inline-flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-xs transition ${
                                isDark
                                  ? 'border-zinc-800 text-zinc-300 hover:bg-zinc-900'
                                  : 'border-stone-300 text-slate-700 hover:bg-stone-100'
                              }`}
                              onClick={pauseModelDownload}
                              type="button"
                            >
                              <Pause size={13} />
                              Pause
                            </button>
                          ) : (
                            <button
                              className="inline-flex items-center gap-1.5 rounded-lg bg-emerald-600 px-3 py-1.5 text-xs text-white transition hover:bg-emerald-500"
                              onClick={resumeModelDownload}
                              type="button"
                            >
                              <Play size={13} />
                              Resume
                            </button>
                          )}
                          <button
                            className={`inline-flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-xs transition ${
                              isDark
                                ? 'border-red-900/70 text-red-300 hover:bg-red-950/30'
                                : 'border-red-200 text-red-700 hover:bg-red-50'
                            }`}
                            onClick={cancelModelDownload}
                            type="button"
                          >
                            <X size={13} />
                            Cancel
                          </button>
                        </div>
                      </div>
                    )}

                    <div>
                      <label className="mb-2 block text-sm font-semibold" htmlFor="installed-model-select">
                        Installed Ollama Models
                      </label>
                      <select
                        className={`w-full rounded-lg border px-3 py-2 text-sm outline-none focus:border-emerald-600 ${
                          isDark
                            ? 'border-zinc-800 bg-zinc-900 text-zinc-100'
                            : 'border-stone-300 bg-white text-slate-900'
                        }`}
                        disabled={availableModels.length === 0 || modelDownloadState === 'downloading'}
                        id="installed-model-select"
                        onChange={(event) => void selectModel(event.target.value)}
                        value={availableModels.find((model) => model.active)?.name ?? ''}
                      >
                        <option value="" disabled>
                          {availableModels.length === 0
                            ? 'No installed models found'
                            : 'Choose installed model'}
                        </option>
                        {availableModels.map((model) => (
                          <option key={model.name} value={model.name}>
                            {model.active ? `${model.name} (active)` : model.name}
                          </option>
                        ))}
                      </select>
                      <div className={`mt-1 text-xs ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>
                        Selecting an installed model warms it before making it active.
                      </div>
                    </div>
                  </div>
                )}

                {settingsTab === 'personalize' && (
                  <div className="space-y-3">
                    <div>
                      <div className="text-sm font-semibold">Local Personalization Profile</div>
                      <div className={`mt-1 text-xs ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>
                        {profilePath || 'Markdown save path will appear after the engine responds.'}
                      </div>
                      <div className={`mt-2 text-xs leading-5 ${isDark ? 'text-zinc-400' : 'text-slate-600'}`}>
                        Add identity details, preferences, writing style notes, goals, or context
                        about how you want AEGIS to respond. This is stored locally as a markdown
                        file and injected into model context during inference so replies stay more
                        aligned to you.
                      </div>
                    </div>
                    <input
                      accept=".txt,.md"
                      className="hidden"
                      onChange={(event) => void importProfileFile(event)}
                      ref={profileImportInputRef}
                      type="file"
                    />
                    <textarea
                      className={`min-h-52 w-full resize-none rounded-xl border p-3 text-sm leading-6 outline-none focus:border-emerald-600 ${
                        isDark
                          ? 'border-zinc-800 bg-zinc-900 text-zinc-100 placeholder:text-zinc-500'
                          : 'border-stone-300 bg-white text-slate-900 placeholder:text-slate-400'
                      }`}
                      onChange={(event) => setProfileText(event.target.value)}
                      placeholder={'Examples:\n- My name is Mohammed.\n- I prefer concise but technically precise answers.\n- I am working on AEGIS and usually want practical implementation help.\n- When explaining code, prioritize architecture before syntax details.'}
                      value={profileText}
                    />
                    <div className="flex justify-end gap-2">
                      <button
                        className={`rounded-lg border px-4 py-2 text-sm transition ${
                          isDark
                            ? 'border-zinc-800 text-zinc-300 hover:bg-zinc-900'
                            : 'border-stone-300 text-slate-700 hover:bg-stone-100'
                        }`}
                        onClick={() => profileImportInputRef.current?.click()}
                        type="button"
                      >
                        Import .txt/.md
                      </button>
                      <button
                        className="rounded-lg bg-emerald-600 px-4 py-2 text-sm font-medium text-white transition hover:bg-emerald-500"
                        onClick={() => void saveProfileSettings()}
                        type="button"
                      >
                        Save Profile
                      </button>
                    </div>
                  </div>
                )}
              </div>
            </section>
          </div>
        </div>
      )}

      {calendarOpen && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
          onClick={() => setCalendarOpen(false)}
        >
          <div
            className={`w-full max-w-lg rounded-xl border p-6 shadow-2xl ${isDark
                ? 'border-zinc-800 bg-zinc-950 text-zinc-100'
                : 'border-stone-300 bg-white text-slate-900'
              }`}
            onClick={(event) => event.stopPropagation()}
          >
            <div className="mb-4 flex items-center justify-between">
              <div className="flex items-center gap-2 text-lg font-semibold">
                <Calendar size={18} />
                Create Calendar Event
              </div>
              <button
                className={`rounded-md p-1 ${isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'
                  }`}
                onClick={() => setCalendarOpen(false)}
                type="button"
              >
                <X size={18} />
              </button>
            </div>

            <div className="mb-4 space-y-2">
              <label className="text-xs font-semibold uppercase tracking-wide opacity-70">
                Local Outlook calendar
              </label>
              <select
                className={`w-full rounded-lg border px-3 py-2 text-sm outline-none focus:border-emerald-600 ${isDark
                    ? 'border-zinc-800 bg-zinc-900 text-zinc-100'
                    : 'border-stone-300 bg-white text-slate-900'
                  }`}
                disabled={
                  creatingCalendarEvent ||
                  loadingOutlookCalendars ||
                  outlookCalendars.length === 0
                }
                onChange={(event) => void selectOutlookCalendar(event.target.value)}
                value={selectedOutlookCalendarId}
              >
                <option value="">
                  {loadingOutlookCalendars
                    ? 'Loading Outlook calendars...'
                    : outlookCalendars.length === 0
                      ? 'Default Outlook calendar / ICS fallback'
                      : 'Choose an Outlook calendar'}
                </option>
                {outlookCalendars.map((calendar) => (
                  <option key={`${calendar.store_name}-${calendar.id}`} value={calendar.id}>
                    {outlookCalendarLabel(calendar)}
                  </option>
                ))}
              </select>
              <p className="text-xs opacity-60">AEGIS uses local Outlook only.</p>
              {calendarMessage && !calendarResult && (
                <div
                  className={`rounded-lg border px-3 py-2 text-xs ${isDark
                      ? 'border-emerald-800 bg-emerald-950/40 text-emerald-200'
                      : 'border-emerald-300 bg-emerald-50 text-emerald-800'
                    }`}
                >
                  {calendarMessage}
                </div>
              )}
            </div>

            <textarea
              className={`mb-4 w-full rounded-lg border px-4 py-3 text-sm outline-none focus:border-emerald-600 ${isDark
                  ? 'border-zinc-800 bg-zinc-900 text-zinc-100 placeholder:text-zinc-500'
                  : 'border-stone-300 bg-white text-slate-900 placeholder:text-slate-400'
                }`}
              disabled={creatingCalendarEvent}
              onChange={(event) => setCalendarPrompt(event.target.value)}
              placeholder='e.g. "Meeting with Jasser tomorrow at 3pm for 1 hour"'
              rows={3}
              value={calendarPrompt}
            />

            <button
              className="flex w-full items-center justify-center gap-2 rounded-lg bg-emerald-600 px-4 py-3 text-sm font-medium text-white hover:bg-emerald-500 disabled:opacity-60"
              disabled={creatingCalendarEvent || !calendarPrompt.trim()}
              onClick={() => void createCalendarEvent()}
              type="button"
            >
              <Calendar size={16} />
              {creatingCalendarEvent ? 'Creating...' : 'Create Event'}
            </button>

            {(calendarMessage || calendarResult) && (
              <div
                className={`mt-4 rounded-lg border p-4 text-sm ${isDark
                    ? 'border-emerald-800 bg-emerald-950/40 text-emerald-200'
                    : 'border-emerald-300 bg-emerald-50 text-emerald-800'
                  }`}
              >
                {calendarMessage && <div className="mb-2 font-semibold">{calendarMessage}</div>}
                {calendarResult && (
                  <>
                    <div className="mb-1 font-semibold">{calendarResult.title}</div>
                    <div className="opacity-80">Start: {calendarResult.start}</div>
                    <div className="opacity-80">End: {calendarResult.end}</div>
                    {calendarResult.location && (
                      <div className="opacity-80">Location: {calendarResult.location}</div>
                    )}
                    {calendarResult.description && (
                      <div className="mt-1 opacity-80">{calendarResult.description}</div>
                    )}
                  </>
                )}
              </div>
            )}
          </div>
        </div>
      )}

      {/* PERFORMANCE METRICS SIDEBAR */}
      <aside
        className={`flex shrink-0 flex-col border-l transition-all duration-300 ease-in-out ${isMetricsOpen ? 'w-72' : 'w-0 border-transparent p-0'
          } ${isDark ? 'border-zinc-800 bg-zinc-950' : 'border-stone-300 bg-stone-50'}`}
      >
        <div
          className={`flex h-full flex-col overflow-hidden ${isMetricsOpen ? 'opacity-100' : 'pointer-events-none opacity-0'}`}
        >
          <div
            className="flex h-16 shrink-0 items-center justify-between px-6 border-b dark:border-zinc-900 border-stone-200"
          >
            <div className="text-xs font-bold uppercase tracking-wider text-zinc-500">
              {metricsTab === 'sources' ? 'Context Sources' : 'Performance Info'}
            </div>
            <button
              className={`rounded-md p-1 transition ${isDark ? 'text-zinc-500 hover:bg-zinc-900 hover:text-zinc-300' : 'text-slate-400 hover:bg-stone-200 hover:text-slate-600'}`}
              onClick={() => setIsMetricsOpen(false)}
              type="button"
            >
              <PanelLeftClose className="rotate-180" size={16} />
            </button>
          </div>

          {/* TAB SELECTOR */}
          <div className={`flex border-b shrink-0 ${isDark ? 'border-zinc-800' : 'border-stone-200'}`}>
            <button
              className={`flex-1 py-3 text-center text-[10px] font-bold uppercase tracking-wider transition ${
                metricsTab === 'metrics'
                  ? 'border-b-2 border-emerald-500 text-emerald-500'
                  : isDark
                    ? 'text-zinc-500 hover:text-zinc-300'
                    : 'text-slate-500 hover:text-slate-700'
              }`}
              onClick={() => setMetricsTab('metrics')}
              type="button"
            >
              Live Stats
            </button>
            <button
              className={`flex-1 py-3 text-center text-[10px] font-bold uppercase tracking-wider transition relative ${
                metricsTab === 'sources'
                  ? 'border-b-2 border-emerald-500 text-emerald-500'
                  : isDark
                    ? 'text-zinc-500 hover:text-zinc-300'
                    : 'text-slate-500 hover:text-slate-700'
              }`}
              onClick={() => setMetricsTab('sources')}
              type="button"
            >
              Sources
              {selectedMessageSources && selectedMessageSources.length > 0 && (
                <span className="absolute right-3.5 top-2.5 flex h-4 w-4 items-center justify-center rounded-full bg-emerald-500 text-[9px] font-extrabold text-white">
                  {selectedMessageSources.length}
                </span>
              )}
            </button>
          </div>

          {/* SIDEBAR MAIN SWITCHABLE CONTENT */}
          {metricsTab === 'sources' ? (
            <div className="flex-1 overflow-y-auto p-5 space-y-4">
              {selectedMessageSources && selectedMessageSources.length > 0 ? (
                <>
                  <div className="flex items-center justify-between">
                    <span className="text-[10px] font-bold uppercase tracking-wider text-zinc-500">
                      Retrieved Excerpts
                    </span>
                    <button
                      className={`text-[9px] font-bold uppercase tracking-wider transition hover:text-emerald-500 ${
                        isDark ? 'text-zinc-400' : 'text-slate-500'
                      }`}
                      onClick={() => {
                        setSelectedMessageSources(null);
                        setSelectedMessageSourcesIndex(null);
                      }}
                    >
                      Clear Selection
                    </button>
                  </div>

                  <div className="space-y-3.5">
                    {selectedMessageSources.map((src, sIdx) => {
                      const isLegacyString = typeof src === 'string';
                      const rawSource = isLegacyString ? (src as unknown as string) : (src.source || '');
                      const filename = rawSource.split(/[/\\]/).pop() || rawSource;
                      const page = isLegacyString ? undefined : src.page;
                      const score = isLegacyString ? 0.0 : (src.score || 0.0);
                      const text = isLegacyString ? '' : (src.text || '');

                      return (
                        <div
                          key={sIdx}
                          className={`rounded-lg border p-3.5 space-y-2.5 text-xs transition duration-200 ${
                            isDark
                              ? 'border-zinc-800 bg-zinc-900/30 hover:bg-zinc-900/50 text-zinc-300'
                              : 'border-stone-200 bg-white hover:bg-stone-50 text-slate-800 shadow-sm'
                          }`}
                        >
                          {/* CHUNK META HEADER */}
                          <div className="flex flex-wrap items-center justify-between gap-1.5 border-b pb-2 border-dashed border-stone-200 dark:border-zinc-800/60">
                            <div className="flex items-center gap-1.5 font-bold text-emerald-600 dark:text-emerald-400 truncate max-w-[70%]">
                              <FileText size={12} className="shrink-0" />
                              <span className="truncate" title={filename}>{filename}</span>
                            </div>
                            <div className="flex items-center gap-1 shrink-0">
                              {page !== undefined && page !== null && (
                                <span className={`px-1.5 py-0.5 rounded text-[9px] font-extrabold uppercase tracking-wider ${
                                  isDark ? 'bg-zinc-800 text-zinc-400' : 'bg-stone-100 text-slate-500'
                                }`}>
                                  Pg {page}
                                </span>
                              )}
                              {!isLegacyString && (
                                <span className={`font-mono text-[9px] px-1.5 py-0.5 rounded font-extrabold uppercase tracking-wider ${
                                  isDark ? 'bg-emerald-950/40 text-emerald-400' : 'bg-emerald-50 text-emerald-700'
                                }`}>
                                  {(score * 100).toFixed(0)}%
                                </span>
                              )}
                            </div>
                          </div>

                          {/* CHUNK TEXT EXCERPT */}
                          {text ? (
                            <div className={`font-serif leading-relaxed p-2.5 rounded border border-dashed text-[11px] overflow-y-auto max-h-48 whitespace-pre-wrap ${
                              isDark
                                ? 'border-zinc-800/80 bg-zinc-950/50 text-zinc-400'
                                : 'border-stone-200 bg-stone-50/50 text-slate-600'
                            }`}>
                              {text}
                            </div>
                          ) : (
                            <div className="text-[11px] italic text-zinc-500">
                              No excerpt available for legacy reference format.
                            </div>
                          )}
                        </div>
                      );
                    })}
                  </div>
                </>
              ) : (
                <div className="flex flex-col items-center justify-center py-16 px-4 text-center space-y-4">
                  <div className={`p-4 rounded-full ${isDark ? 'bg-zinc-900/60' : 'bg-stone-100'}`}>
                    <BookOpen size={24} className="text-emerald-500 opacity-60" />
                  </div>
                  <div className="space-y-1.5">
                    <h3 className={`text-xs font-bold uppercase tracking-wider ${isDark ? 'text-zinc-300' : 'text-slate-800'}`}>
                      No Turn Selected
                    </h3>
                    <p className="text-[11px] leading-relaxed text-zinc-500">
                      Click the <span className="inline-flex items-center align-middle font-bold text-emerald-500">📖 sources</span> button on any AI response to inspect retrieved context excerpts in detail.
                    </p>
                  </div>
                </div>
              )}
            </div>
          ) : (
            <div className="flex-1 space-y-8 overflow-y-auto p-6">
              {/* SYSTEM RESOURCE UTILIZATION */}
              <div className="space-y-4">
                <div className="text-xs font-semibold uppercase tracking-wider text-zinc-500">
                  System Resources
                </div>

                {/* CPU USAGE */}
                <div>
                  <div className="mb-2 flex justify-between text-xs">
                    <span className={isDark ? 'text-zinc-400' : 'text-slate-500'}>CPU Usage</span>
                    <span className="font-mono font-medium">{systemStats.cpu}%</span>
                  </div>
                  <div
                    className={`h-1.5 w-full overflow-hidden rounded-full ${isDark ? 'bg-zinc-800' : 'bg-stone-200'}`}
                  >
                    <div
                      className={`h-full transition-all duration-500 ${systemStats.cpu > 85
                        ? 'bg-red-500'
                        : systemStats.cpu > 60
                          ? 'bg-amber-500'
                          : 'bg-emerald-500'
                        }`}
                      style={{ width: `${systemStats.cpu}%` }}
                    />
                  </div>
                </div>

                {/* RAM USAGE */}
                <div>
                  <div className="mb-2 flex justify-between text-xs">
                    <span className={isDark ? 'text-zinc-400' : 'text-slate-500'}>RAM Usage</span>
                    <span className="font-mono font-medium">{systemStats.ram}%</span>
                  </div>
                  <div
                    className={`h-1.5 w-full overflow-hidden rounded-full ${isDark ? 'bg-zinc-800' : 'bg-stone-200'}`}
                  >
                    <div
                      className={`h-full transition-all duration-500 ${systemStats.ram > 85
                        ? 'bg-red-500'
                        : systemStats.ram > 60
                          ? 'bg-amber-500'
                          : 'bg-emerald-500'
                        }`}
                      style={{ width: `${systemStats.ram}%` }}
                    />
                  </div>
                </div>
              </div>

              <div className={`h-px w-full ${isDark ? 'bg-zinc-800' : 'bg-stone-200'}`} />

              {/* INFERENCE STATS */}
              <div>
                <div className="mb-4 text-xs font-semibold uppercase tracking-wider text-zinc-500">
                  Inference Engine
                </div>
                <div className="grid grid-cols-2 gap-3">
                  <div
                    className={`rounded-lg border p-3 ${isDark ? 'border-zinc-800 bg-zinc-900/40' : 'border-stone-300 bg-white'}`}
                  >
                    <div className="mb-1 text-[10px] uppercase text-zinc-500">Total Latency</div>
                    <div className="font-mono text-sm font-semibold">
                      {inferenceStats.latency > 0 ? `${(inferenceStats.latency / 1000).toFixed(2)}s` : '---'}
                    </div>
                  </div>
                  <div
                    className={`rounded-lg border p-3 ${isDark ? 'border-zinc-800 bg-zinc-900/40' : 'border-stone-300 bg-white'}`}
                  >
                    <div className="mb-1 text-[10px] uppercase text-zinc-500">Speed (TPS)</div>
                    <div className="font-mono text-sm font-semibold">
                      {inferenceStats.tps > 0 ? `${inferenceStats.tps}` : '---'}
                    </div>
                  </div>
                  <div
                    className={`rounded-lg border p-3 ${isDark ? 'border-zinc-800 bg-zinc-900/40' : 'border-stone-300 bg-white'}`}
                  >
                    <div className="mb-1 text-[10px] uppercase text-zinc-500">TTFT</div>
                    <div className="font-mono text-sm font-semibold">
                      {inferenceStats.ttft > 0 ? `${inferenceStats.ttft}ms` : '---'}
                    </div>
                  </div>
                  <div
                    className={`rounded-lg border p-3 ${isDark ? 'border-zinc-800 bg-zinc-900/40' : 'border-stone-300 bg-white'}`}
                  >
                    <div className="mb-1 text-[10px] uppercase text-zinc-500">RAG Delay</div>
                    <div className="font-mono text-sm font-semibold">
                      {inferenceStats.ragTime > 0 ? `${inferenceStats.ragTime}ms` : '---'}
                    </div>
                  </div>
                </div>
              </div>

              <div className={`h-px w-full ${isDark ? 'bg-zinc-800' : 'bg-stone-200'}`} />

              {/* RAG ANALYSIS */}
              <div>
                <div className="mb-4 text-xs font-semibold uppercase tracking-wider text-zinc-500">
                  RAG Engine Analysis
                </div>
                <div className="space-y-4">
                  <div>
                    <div className="mb-1.5 flex justify-between text-[11px]">
                      <span className={isDark ? 'text-zinc-400' : 'text-slate-500'}>
                        Semantic Similarity
                      </span>
                      <span className="font-mono font-medium">
                        {inferenceStats.similarity > 0
                          ? `${(inferenceStats.similarity * 100).toFixed(0)}%`
                          : '---'}
                      </span>
                    </div>
                    <div
                      className={`h-1 w-full overflow-hidden rounded-full ${isDark ? 'bg-zinc-800' : 'bg-stone-200'}`}
                    >
                      <div
                        className="h-full bg-emerald-500 opacity-60 transition-all duration-500"
                        style={{ width: `${inferenceStats.similarity * 100}%` }}
                      />
                    </div>
                  </div>

                  <div className="grid grid-cols-2 gap-3">
                    <div className={`rounded-lg border p-2 ${isDark ? 'border-zinc-800 bg-zinc-900/40' : 'border-stone-300 bg-white'}`}>
                      <div className="mb-0.5 text-[9px] uppercase text-zinc-500">Chunks</div>
                      <div className="font-mono text-xs font-semibold">
                        {inferenceStats.chunks || '0'}
                      </div>
                    </div>
                    <div className={`rounded-lg border p-2 ${isDark ? 'border-zinc-800 bg-zinc-900/40' : 'border-stone-300 bg-white'}`}>
                      <div className="mb-0.5 text-[9px] uppercase text-zinc-500">Backend</div>
                      <div className="truncate font-mono text-[10px] font-semibold">
                        {inferenceStats.backend}
                      </div>
                    </div>
                  </div>
                </div>
              </div>

              <div
                className={`rounded-lg p-3 text-[11px] leading-relaxed ${isDark ? 'bg-zinc-900/60 text-zinc-500' : 'bg-stone-100 text-slate-500'}`}
              >
                Generation speed is estimated based on the average character count per token (approx. 4
                chars/token).
              </div>
            </div>
          )}
        </div>
      </aside>
    </div>
  );
}
