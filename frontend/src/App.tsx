import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { FormEvent, ReactNode } from 'react';
import {
  Bot,
  Calendar,
  ChevronDown,
  Cpu,
  Download,
  Edit3,
  HardDrive,
  MessageSquare,
  Moon,
  Plus,
  RefreshCw,
  Send,
  Sun,
  Trash2,
  Upload,
  User,
  Wrench,
  X,
} from 'lucide-react';

type Role = 'user' | 'assistant';
type ThemeMode = 'dark' | 'light';
type MarkdownBlock =
  | { type: 'paragraph'; text: string }
  | { type: 'ordered'; items: string[] }
  | { type: 'unordered'; items: string[] }
  | { type: 'code'; text: string };

interface Message {
  role: Role;
  content: string;
  edited?: boolean;
  timestamp?: string;
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

interface IndexedDocument {
  file_name: string;
  stored_path: string;
  chunks_added: number;
}

interface IngestResponse {
  status: string;
  total_chunks: number;
  documents: IndexedDocument[];
}

type ImportPhase = 'idle' | 'uploading' | 'indexing' | 'complete' | 'error';

const API_BASE = '/api';
const THEME_STORAGE_KEY = 'aegis-ui-theme';
const INDEXED_DOCUMENTS_STORAGE_KEY = 'aegis-indexed-documents';

function sessionDescription(session: EngineSessionSummary) {
  const turnLabel = session.turn_count === 1 ? 'turn' : 'turns';
  return `${session.turn_count} ${turnLabel}`;
}

function turnsToMessages(turns: EngineTurn[]): Message[] {
  return turns.flatMap((turn) => [
    { role: 'user' as const, content: turn.query, edited: turn.edited, timestamp: turn.created_at },
    { role: 'assistant' as const, content: turn.response, timestamp: turn.created_at },
  ]);
}

function cleanOutlookCalendarName(name: string) {
  return name.replace(/\s*\(this computer only\)\s*/gi, ' ').replace(/\s+/g, ' ').trim();
}

function isEmailBackedOutlookCalendar(calendar: OutlookCalendar) {
  return Boolean(calendar.email_address?.trim());
}

function outlookCalendarLabel(calendar: OutlookCalendar) {
  const calendarName = cleanOutlookCalendarName(calendar.name);
  const emailAddress = calendar.email_address?.trim();

  return emailAddress ? `${calendarName} (${emailAddress})` : calendarName;
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

function loadStoredIndexedDocuments() {
  if (typeof window === 'undefined') {
    return [];
  }

  try {
    const raw = window.localStorage.getItem(INDEXED_DOCUMENTS_STORAGE_KEY);
    if (!raw) {
      return [];
    }

    const parsed = JSON.parse(raw) as IndexedDocument[];
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
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

function normalizeAssistantMarkdown(content: string) {
  return content
    .replace(/\r\n/g, '\n')
    .replace(/([:.!?])\s*(\d+\.\s+)/g, '$1\n$2')
    .replace(/([:.!?])\s*([*+-]\s+)/g, '$1\n$2')
    .replace(/([A-Za-z0-9)])\s*([*+-]\s+)/g, '$1\n$2')
    .replace(/([A-Za-z0-9)])\s+(\d+\.\s+)/g, '$1\n$2')
    .replace(/([^\n])(\d+\.\s+\*\*)/g, '$1\n$2')
    .replace(/\n{3,}/g, '\n\n');
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

function parseMarkdownBlocks(content: string): MarkdownBlock[] {
  const normalized = normalizeAssistantMarkdown(content);
  const lines = normalized.split('\n');
  const blocks: MarkdownBlock[] = [];
  let paragraph: string[] = [];
  let codeLines: string[] = [];
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

    if (line.startsWith('```')) {
      if (inCode) {
        blocks.push({ type: 'code', text: codeLines.join('\n') });
        codeLines = [];
        inCode = false;
      } else {
        flushParagraph();
        inCode = true;
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
    blocks.push({ type: 'code', text: codeLines.join('\n') });
  }
  flushParagraph();

  return blocks.length > 0 ? blocks : [{ type: 'paragraph', text: content }];
}

function renderInlineMarkdown(text: string) {
  const parts: ReactNode[] = [];
  const pattern = /(\*\*[^*]+\*\*|\*[^*\s][^*]*\*)/g;
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = pattern.exec(text)) !== null) {
    if (match.index > lastIndex) {
      parts.push(text.slice(lastIndex, match.index));
    }

    const value = match[0];
    if (value.startsWith('**')) {
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

function AssistantMarkdown({ content }: { content: string }) {
  const blocks = parseMarkdownBlocks(content || '...');

  return (
    <div className="space-y-3">
      {blocks.map((block, blockIndex) => {
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
          return (
            <pre
              className="overflow-x-auto rounded-md bg-black/30 p-3 font-mono text-xs leading-5"
              key={`code-${blockIndex}`}
            >
              <code>{block.text}</code>
            </pre>
          );
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
  addLine('Format: speaker label, timestamp, message body');
  addLine('------------------------------------------------------------');
  addLine('');

  options.messages.forEach((message) => {
    const label = `${speakerLabel(message.role)} | ${formatExportTimestamp(message.timestamp)}${
      message.edited ? ' | edited' : ''
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
  const [importProgress, setImportProgress] = useState(0);
  const [importPhase, setImportPhase] = useState<ImportPhase>('idle');
  const [importFileLabel, setImportFileLabel] = useState('');
  const [indexedDocuments, setIndexedDocuments] = useState<IndexedDocument[]>(
    loadStoredIndexedDocuments,
  );
  const [status, setStatus] = useState('Ready');
  const [error, setError] = useState<string | null>(null);
  const [toolsOpen, setToolsOpen] = useState(false);
  const [calendarOpen, setCalendarOpen] = useState(false);
  const [calendarPrompt, setCalendarPrompt] = useState('');
  const [creatingCalendarEvent, setCreatingCalendarEvent] = useState(false);
  const [calendarResult, setCalendarResult] = useState<CalendarResult | null>(null);
  const [calendarMessage, setCalendarMessage] = useState<string | null>(null);
  const [outlookCalendars, setOutlookCalendars] = useState<OutlookCalendar[]>([]);
  const [selectedOutlookCalendarId, setSelectedOutlookCalendarId] = useState('');
  const [loadingOutlookCalendars, setLoadingOutlookCalendars] = useState(false);
  const [systemStats, setSystemStats] = useState<SystemStats>({ cpu: 0, ram: 0 });
  const [editingMessageIndex, setEditingMessageIndex] = useState<number | null>(null);
  const [editingMessageText, setEditingMessageText] = useState('');
  const [editingSessionId, setEditingSessionId] = useState<string | null>(null);
  const [editingTitle, setEditingTitle] = useState('');
  const [deletingSessionIds, setDeletingSessionIds] = useState<string[]>([]);
  const scrollRef = useRef<HTMLDivElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const isDark = theme === 'dark';
  const resourceWarning =
    systemStats.cpu > 80 || systemStats.ram > 80
      ? `${[
          systemStats.cpu > 80 ? 'CPU' : null,
          systemStats.ram > 80 ? 'RAM' : null,
        ]
          .filter(Boolean)
          .join(' and ')} ${systemStats.cpu > 80 && systemStats.ram > 80 ? 'are' : 'is'} almost at full capacity.`
      : null;

  const activeSession = useMemo(
    () => sessions.find((session) => session.session_id === activeSessionId),
    [activeSessionId, sessions],
  );
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
    if (typeof window === 'undefined') {
      return;
    }

    window.localStorage.setItem(THEME_STORAGE_KEY, theme);
  }, [theme]);

  useEffect(() => {
    if (typeof window === 'undefined') {
      return;
    }

    window.localStorage.setItem(
      INDEXED_DOCUMENTS_STORAGE_KEY,
      JSON.stringify(indexedDocuments),
    );
  }, [indexedDocuments]);

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
    setImportPhase('uploading');
    setImportProgress(3);
    setImportFileLabel(
      files.length === 1 ? files[0].name : `${files.length} documents`,
    );
    setStatus('Indexing documents');
    setError(null);

    try {
      const formData = new FormData();
      for (let i = 0; i < files.length; i++) {
        formData.append('file', files[i]);
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

      setIndexedDocuments((current) =>
        mergeIndexedDocuments(current, ingestResponse.documents),
      );
      setImportFileLabel(
        ingestResponse.documents.length === 1
          ? ingestResponse.documents[0].file_name
          : `${ingestResponse.documents.length} documents`,
      );
      setImportPhase('complete');
      setImportProgress(100);
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

  async function loadOutlookCalendars() {
    setLoadingOutlookCalendars(true);

    try {
      const response = await fetch(`${API_BASE}/calendar/outlook/calendars`);
      if (!response.ok) {
        const body = await response.text();
        throw new Error(body || `Engine returned HTTP ${response.status} while loading Outlook calendars.`);
      }

      const data = (await response.json()) as OutlookCalendarsResponse;
      const emailBackedCalendars = data.calendars.filter(isEmailBackedOutlookCalendar);
      setOutlookCalendars(emailBackedCalendars);
      setSelectedOutlookCalendarId(
        emailBackedCalendars.find((calendar) => calendar.is_selected)?.id ?? '',
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

    const safeTitle = (activeSession?.title ?? 'aegis-chat')
      .trim()
      .replace(/[\\/:*?"<>|]+/g, '-')
      .replace(/\s+/g, '-')
      .toLowerCase();
    const blob = createConversationPdf({
      title: activeSession?.title ?? 'AEGIS Chat Export',
      sessionId: activeSession?.session_id,
      messages,
    });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = `${safeTitle || 'aegis-chat'}.pdf`;
    anchor.click();
    URL.revokeObjectURL(url);
    setToolsOpen(false);
  }

  async function streamPrompt(
    prompt: string,
    nextMessages: Message[],
    editFromTurnIndex?: number,
  ) {
    setError(null);
    setStatus('Inference');
    setIsStreaming(true);
    setMessages(nextMessages);

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
          attachments: indexedDocuments.map(
            (document) => `${document.file_name} (${document.chunks_added} chunks)`,
          ),
          edit_from_turn_index: editFromTurnIndex,
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
        const parsed = extractSseEvents(pending);
        pending = parsed.remaining;

        for (const event of parsed.events) {
          const data = sseEventData(event);
          if (!data) {
            continue;
          }

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
                timestamp: last.timestamp ?? new Date().toISOString(),
              };
            }

            return next;
          });
        }
      }

      const finalData = sseEventData(pending);
      if (finalData && finalData !== '[DONE]') {
        if (finalData.startsWith('[ERROR]')) {
          throw new Error(finalData);
        }

        setMessages((current) => {
          const next = [...current];
          const last = next[next.length - 1];

          if (last?.role === 'assistant') {
            next[next.length - 1] = {
              ...last,
              content: `${last.content}${finalData}`,
              timestamp: last.timestamp ?? new Date().toISOString(),
            };
          }

          return next;
        });
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
      className={`flex h-screen overflow-hidden ${
        isDark ? 'bg-zinc-950 text-zinc-100' : 'bg-stone-100 text-slate-900'
      }`}
    >
      <aside
        className={`flex w-72 shrink-0 flex-col border-r p-4 ${
          isDark ? 'border-zinc-800 bg-zinc-950' : 'border-stone-300 bg-stone-50'
        }`}
      >
        <div className="mb-6 flex items-start justify-between gap-4">
          <div>
            <div className="text-xl font-semibold tracking-wide">AEGIS</div>
          </div>
          <div className="space-y-1 text-right font-mono text-[11px] text-violet-300">
            <div className="flex items-center justify-end gap-1.5">
              <Cpu size={12} />
              <span>{systemStats.cpu}%</span>
            </div>
            <div className="flex items-center justify-end gap-1.5">
              <HardDrive size={12} />
              <span>{systemStats.ram}%</span>
            </div>
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

        {resourceWarning && (
          <div
            className={`border-b px-6 py-3 text-sm font-medium ${
              isDark
                ? 'border-amber-900/60 bg-amber-950/30 text-amber-200'
                : 'border-amber-200 bg-amber-50 text-amber-800'
            }`}
          >
            Warning: {resourceWarning}
          </div>
        )}

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
                    className={`group flex max-w-[78%] flex-col ${
                      message.role === 'user' ? 'items-end' : 'items-start'
                    }`}
                  >
                    {editingMessageIndex === index && message.role === 'user' ? (
                      <div
                        className={`w-80 rounded-lg border p-3 ${
                          isDark
                            ? 'border-emerald-700 bg-zinc-900'
                            : 'border-emerald-500 bg-white'
                        }`}
                      >
                        <textarea
                          autoFocus
                          className={`mb-3 min-h-24 w-full resize-y rounded-md border px-3 py-2 text-sm outline-none focus:border-emerald-600 ${
                            isDark
                              ? 'border-zinc-800 bg-zinc-950 text-zinc-100'
                              : 'border-stone-300 bg-white text-slate-900'
                          }`}
                          onChange={(event) => setEditingMessageText(event.target.value)}
                          value={editingMessageText}
                        />
                        <div className="flex justify-end gap-2">
                          <button
                            className={`rounded-md border px-3 py-1.5 text-xs ${
                              isDark
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
                        className={`rounded-lg px-4 py-3 text-sm leading-6 ${
                          message.role === 'user'
                            ? 'bg-emerald-600 text-white'
                            : isDark
                              ? 'border border-zinc-800 bg-zinc-900 text-zinc-200'
                              : 'border border-stone-300 bg-white text-slate-800'
                        }`}
                      >
                        {message.role === 'assistant' ? (
                          <AssistantMarkdown content={message.content} />
                        ) : (
                          <span className="whitespace-pre-wrap">{message.content || '...'}</span>
                        )}
                      </div>
                    )}
                    {message.role === 'user' && editingMessageIndex !== index && (
                      <div className="mt-1 flex items-center gap-2">
                        <button
                          className={`flex items-center gap-1 rounded-md px-2 py-1 text-xs opacity-0 transition group-hover:opacity-100 ${
                            isDark
                              ? 'text-zinc-500 hover:bg-zinc-900 hover:text-emerald-300'
                              : 'text-slate-500 hover:bg-stone-200 hover:text-emerald-700'
                          }`}
                          disabled={isStreaming}
                          onClick={() => beginEditingMessage(index, message.content)}
                          type="button"
                        >
                          <Edit3 size={12} />
                          Edit
                        </button>
                      </div>
                    )}
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
          {showImportProgress && (
            <div
              className={`mx-auto mb-3 max-w-3xl rounded-lg border px-3 py-2 ${
                importPhase === 'error'
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
                className={`h-1.5 overflow-hidden rounded-full ${
                  isDark ? 'bg-zinc-800' : 'bg-stone-200'
                }`}
                role="progressbar"
                aria-label="Document import progress"
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={importProgress}
              >
                <div
                  className={`h-full rounded-full transition-all duration-300 ${
                    importPhase === 'error'
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
              className={`mx-auto mb-3 flex max-w-3xl items-center gap-2 rounded-lg border px-3 py-2 text-xs ${
                isDark
                  ? 'border-emerald-900/60 bg-emerald-950/20 text-emerald-200'
                  : 'border-emerald-200 bg-emerald-50 text-emerald-800'
              }`}
            >
              <Upload size={14} />
              <span className="truncate">
                Document context active: {indexedDocumentLabel} indexed into {indexedChunkCount}{' '}
                chunks.
              </span>
            </div>
          )}
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
            <div className="relative">
              <button
                className={`flex items-center justify-center gap-2 rounded-lg border px-4 py-3 text-sm transition ${
                  isStreaming ? 'cursor-not-allowed opacity-60' : ''
                } ${
                  isDark
                    ? 'border-zinc-800 bg-zinc-900 text-zinc-300 hover:bg-zinc-800'
                    : 'border-stone-300 bg-white text-slate-700 hover:bg-stone-50'
                }`}
                disabled={isStreaming}
                onClick={() => setToolsOpen((current) => !current)}
                type="button"
              >
                <Wrench size={16} />
                <span>Tools</span>
                <ChevronDown size={14} />
              </button>
              {toolsOpen && (
                <div
                  className={`absolute bottom-full right-0 z-20 mb-2 w-48 rounded-lg border p-1 shadow-xl ${
                    isDark
                      ? 'border-zinc-800 bg-zinc-950 text-zinc-100'
                      : 'border-stone-300 bg-white text-slate-900'
                  }`}
                >
                  <button
                    className={`flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm ${
                      isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'
                    }`}
                    disabled={isStreaming || isUploading}
                    onClick={handleImportToolClick}
                    type="button"
                  >
                    <Upload size={15} />
                    Import
                  </button>
                  <button
                    className={`flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm disabled:opacity-50 ${
                      isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'
                    }`}
                    onClick={openCalendarTool}
                    type="button"
                  >
                    <Calendar size={15} />
                    Calendar
                  </button>
                  <button
                    className={`flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm disabled:opacity-50 ${
                      isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'
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

      {calendarOpen && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
          onClick={() => setCalendarOpen(false)}
        >
          <div
            className={`w-full max-w-lg rounded-xl border p-6 shadow-2xl ${
              isDark
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
                className={`rounded-md p-1 ${
                  isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'
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
                className={`w-full rounded-lg border px-3 py-2 text-sm outline-none focus:border-emerald-600 ${
                  isDark
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
              <p className="text-xs opacity-60">
                AEGIS uses local Outlook only.
              </p>
              {calendarMessage && !calendarResult && (
                <div
                  className={`rounded-lg border px-3 py-2 text-xs ${
                    isDark
                      ? 'border-emerald-800 bg-emerald-950/40 text-emerald-200'
                      : 'border-emerald-300 bg-emerald-50 text-emerald-800'
                  }`}
                >
                  {calendarMessage}
                </div>
              )}
            </div>

            <textarea
              className={`mb-4 w-full rounded-lg border px-4 py-3 text-sm outline-none focus:border-emerald-600 ${
                isDark
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
                className={`mt-4 rounded-lg border p-4 text-sm ${
                  isDark
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
    </div>
  );
}
