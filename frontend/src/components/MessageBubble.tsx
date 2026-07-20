// Individual chat message bubble with editing, sources, TTS, copy, and patch apply
import { useEffect, useState } from 'react';
import { Bot, User, Edit3, Copy, Check, BookOpen, Volume2, VolumeX, FileCode, ChevronDown, BrainCircuit, Route, Search, FileText, TerminalSquare, CheckCircle2, AlertTriangle, Sparkles } from 'lucide-react';
import type { Message, CodeProject, ReasoningEvent, RetrievalChunk } from '../types';
import { extractUnifiedDiff, fitTextareaToContent } from '../lib';
import { AssistantMarkdown } from './AssistantMarkdown';
import { useT } from '../lib/i18n';

function friendlyToolName(tool?: string) {
  if (!tool) return 'context';
  const names: Record<string, string> = {
    rag: 'documents',
    search: 'code search',
    read_file: 'file reader',
    search_files: 'file finder',
    run_terminal: 'terminal check',
    git_status: 'git status',
    list_directory: 'folder scan',
    calculate: 'calculator',
    search_knowledge: 'memory search',
    ocr_image: 'OCR',
    describe_image: 'vision',
    zotero: 'research library',
    write_file: 'patch proposal',
  };
  return names[tool] ?? tool.replaceAll('_', ' ');
}

function reasoningCopy(event: ReasoningEvent) {
  const tool = friendlyToolName(event.tool);
  switch (event.phase) {
    case 'routing':
    case 'route':
      return {
        title: 'Choosing the response path',
        detail: 'Checking whether a specific tool would improve the answer.',
        tone: 'sky',
      };
    case 'route_direct':
      return {
        title: 'Answering directly',
        detail: event.detail ?? 'No external tools are needed for this request.',
        tone: 'emerald',
      };
    case 'route_tools':
      return {
        title: 'Using focused tools',
        detail: event.detail ?? 'Only the tools relevant to this request are available.',
        tone: 'cyan',
      };
    case 'start':
      return {
        title: 'Framing the problem',
        detail: 'Separating what is known from what needs verification.',
        tone: 'emerald',
      };
    case 'thinking':
      return {
        title: event.round && event.round > 1 ? `Rechecking the plan` : 'Looking for the next useful move',
        detail: event.detail ?? (event.round && event.round > 1 ? 'Using the latest result to decide whether another check is worth it.' : 'Choosing between answering now or gathering stronger evidence.'),
        tone: 'zinc',
      };
    case 'tool_call':
      return {
        title: `Consulting ${tool}`,
        detail: event.detail ?? (event.tool === 'rag'
          ? 'Pulling relevant passages from the active document context.'
          : event.tool === 'run_terminal'
            ? 'Running a read-only check and watching for useful output.'
            : `Gathering signal from ${tool} before answering.`),
        tone: 'cyan',
      };
    case 'tool_result':
      return {
        title: `${tool} came back`,
        detail: event.detail ?? 'Adding the result to the answer context and checking if it is enough.',
        tone: 'emerald',
      };
    case 'tool_error':
      return {
        title: `${tool} hit a snag`,
        detail: 'The loop will try another route or answer with the reliable context it has.',
        tone: 'amber',
      };
    case 'repair':
      return {
        title: 'Cleaning up a tool decision',
        detail: 'The model response was not valid tool JSON, so AEGIS asked for a safer structured action.',
        tone: 'amber',
      };
    case 'fallback':
      return {
        title: 'Switching to a safe answer',
        detail: 'Tool routing was unreliable, so raw reasoning output is hidden and a final answer is requested.',
        tone: 'amber',
      };
    case 'guard':
      return {
        title: event.title || 'Applying a safety guard',
        detail: event.detail ?? 'AEGIS adjusted the route to keep the result accurate and efficient.',
        tone: 'amber',
      };
    case 'limit':
      return {
        title: 'Wrapping up the loop',
        detail: 'The reasoning budget is reached, so AEGIS is composing from gathered context.',
        tone: 'amber',
      };
    case 'final':
      return {
        title: 'Ready to answer',
        detail: 'Enough context is available, so the response is being composed.',
        tone: 'emerald',
      };
    default:
      return {
        title: event.title,
        detail: event.detail ?? 'Working through the request.',
        tone: 'zinc',
      };
  }
}

function ReasoningTrace({ events, isDark, isStreaming }: { events: ReasoningEvent[]; isDark: boolean; isStreaming: boolean }) {
  const [expandedOverride, setExpandedOverride] = useState<boolean | null>(null);
  const [visibleEventCount, setVisibleEventCount] = useState(() => Math.min(events.length, 1));
  const expanded = expandedOverride ?? isStreaming;

  useEffect(() => {
    if (visibleEventCount >= events.length) return;
    const timer = window.setTimeout(() => {
      setVisibleEventCount((current) => Math.min(current + 1, events.length));
    }, 180);
    return () => window.clearTimeout(timer);
  }, [events.length, visibleEventCount]);

  if (events.length === 0) return null;

  const visibleEvents = events.slice(0, Math.max(visibleEventCount, 1));
  const latest = visibleEvents[visibleEvents.length - 1];
  const latestCopy = reasoningCopy(latest);
  const toolCount = visibleEvents.filter((event) => event.phase === 'tool_call').length;
  const checkCount = visibleEvents.filter((event) => ['thinking', 'repair', 'guard'].includes(event.phase)).length;
  const directRoute = visibleEvents.some((event) => event.phase === 'route_direct');
  const toolFailed = visibleEvents.some((event) => event.phase === 'tool_error');
  const usedTools = [...new Set(visibleEvents
    .filter((event) => event.phase === 'tool_call')
    .map((event) => friendlyToolName(event.tool)))];
  const summary = isStreaming
    ? latestCopy.title
    : toolCount
      ? toolFailed ? `${usedTools.join(', ')} check incomplete` : `Used ${usedTools.join(', ')}`
      : directRoute
        ? 'Answered directly'
        : `Reasoned through ${visibleEvents.length} step${visibleEvents.length === 1 ? '' : 's'}`;
  const subline = isStreaming
    ? latestCopy.detail
    : toolCount
      ? `${toolCount} focused tool call${toolCount === 1 ? '' : 's'}${toolFailed ? ', fallback used' : ''}${checkCount ? `, ${checkCount} decision pass${checkCount === 1 ? '' : 'es'}` : ''}`
      : directRoute
        ? 'No external tools needed'
        : `${checkCount} decision pass${checkCount === 1 ? '' : 'es'}`;

  const renderIcon = (event: ReasoningEvent, active: boolean) => {
    const className = `shrink-0 ${active && isStreaming ? 'animate-pulse' : ''}`;
    const size = 14;
    if (['route', 'routing', 'route_direct', 'route_tools'].includes(event.phase)) return <Route className={className} size={size} />;
    if (event.phase === 'tool_call') return event.tool === 'run_terminal' ? <TerminalSquare className={className} size={size} /> : <Search className={className} size={size} />;
    if (event.phase === 'tool_result') return <FileText className={className} size={size} />;
    if (event.phase === 'tool_error' || event.phase === 'repair' || event.phase === 'fallback' || event.phase === 'guard') return <AlertTriangle className={className} size={size} />;
    if (event.phase === 'final') return <CheckCircle2 className={className} size={size} />;
    if (event.phase === 'start') return <Sparkles className={className} size={size} />;
    return <BrainCircuit className={className} size={size} />;
  };

  const toneClass = (tone: string, active = false) => {
    if (tone === 'emerald') return active ? isDark ? 'text-emerald-300 bg-emerald-400/15 ring-emerald-400/25' : 'text-emerald-700 bg-emerald-100/80 ring-emerald-300/45' : 'text-emerald-400 bg-emerald-400/10';
    if (tone === 'cyan') return active ? isDark ? 'text-cyan-300 bg-cyan-400/15 ring-cyan-400/25' : 'text-cyan-700 bg-cyan-100/80 ring-cyan-300/45' : 'text-cyan-400 bg-cyan-400/10';
    if (tone === 'sky') return active ? isDark ? 'text-sky-300 bg-sky-400/15 ring-sky-400/25' : 'text-sky-700 bg-sky-100/80 ring-sky-300/45' : 'text-sky-400 bg-sky-400/10';
    if (tone === 'amber') return active ? isDark ? 'text-amber-300 bg-amber-400/15 ring-amber-400/25' : 'text-amber-700 bg-amber-100/85 ring-amber-300/45' : 'text-amber-400 bg-amber-400/10';
    return active ? isDark ? 'text-zinc-200 bg-white/[0.08] ring-white/10' : 'text-stone-700 bg-stone-900/10 ring-stone-700/20' : isDark ? 'text-zinc-400 bg-white/5' : 'text-stone-600 bg-stone-900/5';
  };

  return (
    <div className={`aegis-reasoning-trace mb-5 text-[14px] ${isDark ? 'text-zinc-300' : 'text-slate-600'}`}>
      <button
        className={`aegis-reasoning-summary inline-flex max-w-full items-center gap-2 rounded-xl border px-3.5 py-2 text-left transition-all duration-200 active:scale-[0.99] ${expanded ? 'shadow-[0_10px_30px_rgba(0,0,0,0.12)]' : 'shadow-none'} ${isDark ? 'border-white/10 bg-white/[0.03] hover:bg-white/[0.06]' : 'border-stone-300/70 bg-transparent hover:bg-stone-900/[0.03]'}`}
        onClick={() => setExpandedOverride(!expanded)}
        type="button"
      >
        <span className={`flex h-6 w-6 shrink-0 items-center justify-center rounded-md ring-1 ${toneClass(latestCopy.tone, true)}`}>
          {renderIcon(latest, true)}
        </span>
        <span className="min-w-0 flex items-baseline gap-2">
          <span className={`shrink-0 text-[12px] font-medium ${isStreaming ? isDark ? 'text-yellow-400' : 'text-amber-600' : isDark ? 'text-emerald-300' : 'text-emerald-700'}`}>
            {isStreaming ? 'Explore' : 'Explored'}
          </span>
          <span className={`truncate text-[14px] ${isDark ? 'text-zinc-200' : 'text-slate-800'}`}>{summary}</span>
        </span>
        <span className={`hidden shrink-0 text-[12px] sm:inline ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>
          {subline}
        </span>
        <ChevronDown className={`shrink-0 transition-transform ${expanded ? 'rotate-180' : ''} ${isDark ? 'text-zinc-500' : 'text-slate-400'}`} size={14} />
      </button>
      <div
        aria-hidden={!expanded}
        className={`grid transition-[grid-template-rows,opacity,margin] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)] ${
          expanded ? 'mt-4 grid-rows-[1fr] opacity-100' : 'mt-0 grid-rows-[0fr] opacity-0'
        }`}
      >
        <div className="min-h-0 overflow-hidden">
          <div className={`aegis-reasoning-panel space-y-3 transition-transform duration-300 ease-[cubic-bezier(0.22,1,0.36,1)] ${expanded ? 'translate-y-0' : '-translate-y-2'} border-white/0 bg-transparent`}>
            {visibleEvents.map((event, idx) => {
              const copy = reasoningCopy(event);
              const active = idx === visibleEvents.length - 1;
              const label = event.phase === 'tool_call'
                ? friendlyToolName(event.tool)
                : event.phase === 'tool_result'
                  ? 'Result'
                  : event.phase === 'final'
                    ? 'Answer'
                    : event.phase === 'thinking'
                      ? 'Think'
                      : ['route', 'routing', 'route_direct', 'route_tools'].includes(event.phase)
                        ? 'Route'
                        : 'Note';
              return (
                <div
                  key={`${event.phase}-${idx}`}
                  className={`aegis-reasoning-row flex min-w-0 items-start gap-3 rounded-xl px-2 py-1.5 transition-all duration-300 ease-out animate-[reasoningStageIn_260ms_cubic-bezier(0.22,1,0.36,1)] ${
                    expanded ? 'translate-y-0 opacity-100' : '-translate-y-1 opacity-0'
                  } ${active ? isDark ? 'bg-white/[0.055] ring-1 ring-white/[0.08]' : 'bg-emerald-50/70 ring-1 ring-emerald-100' : ''}`}
                >
                  <span className={`w-16 shrink-0 pt-0.5 text-[12px] font-medium ${active ? isDark ? 'text-emerald-300' : 'text-emerald-800' : isDark ? 'text-zinc-500' : 'text-stone-500'}`}>
                    {label}
                  </span>
                  <div className="min-w-0 flex-1">
                    <div className={`text-[14px] leading-6 tracking-[-0.002em] ${active ? isDark ? 'text-zinc-100' : 'text-stone-950' : isDark ? 'text-zinc-500' : 'text-[rgba(75,89,116,0.72)]'}`}>
                      <span className={active ? 'font-medium' : ''}>{copy.title}</span>
                      <span className="mx-2 opacity-45">/</span>
                      <span>{copy.detail}</span>
                      {event.round && <span className="ml-2 opacity-45">pass {event.round}</span>}
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
}

interface MessageBubbleProps {
  message: Message;
  index: number;
  isDark: boolean;
  isStreaming: boolean;
  editingMessageIndex: number | null;
  editingMessageText: string;
  copiedMessageIndex: number | null;
  selectedMessageSourcesIndex: number | null;
  speakingMessageIndex: number | null;
  activeProject: CodeProject | null;
  onBeginEditing: (index: number, content: string) => void;
  onCancelEditing: () => void;
  onEditingTextChange: (value: string) => void;
  onResendEdited: (index: number) => void;
  onCopyMessage: (index: number, content: string) => void;
  onToggleSources: (index: number, sources: RetrievalChunk[]) => void;
  onSpeak: (text: string, force: boolean, index: number) => void;
  onApplyPatch: () => void;
}

export function MessageBubble({
  message, index, isDark, isStreaming, editingMessageIndex, editingMessageText,
  copiedMessageIndex, selectedMessageSourcesIndex, speakingMessageIndex, activeProject,
  onBeginEditing, onCancelEditing, onEditingTextChange, onResendEdited,
  onCopyMessage, onToggleSources, onSpeak, onApplyPatch,
}: MessageBubbleProps) {
  const t = useT();
  if (message.role === 'assistant') {
    return (
      <div className="group grid grid-cols-[2rem_minmax(0,1fr)] gap-4 py-4">
        <div className={`mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-xl ${isDark ? 'bg-white/[0.06] text-zinc-300 ring-1 ring-white/10' : 'bg-[rgba(255,250,240,0.9)] text-stone-600 shadow-sm ring-1 ring-[rgba(94,76,55,0.22)]'}`}>
          <Bot size={16} />
        </div>
        <div className="min-w-0">
          <ReasoningTrace events={message.reasoningEvents ?? []} isDark={isDark} isStreaming={isStreaming && !message.content} />
          <div className={`aegis-prose max-w-none text-[16px] leading-8 tracking-normal ${isDark ? 'text-zinc-100' : 'text-stone-900'}`}>
            <AssistantMarkdown content={message.content} isDark={isDark} />
          </div>

          {message.content && (
            <div className="mt-3 flex items-center gap-1 opacity-45 transition-all duration-150 hover:opacity-100 focus-within:opacity-100">
              {message.sources && message.sources.length > 0 && (
                <button
                  aria-label="Inspect retrieved sources"
                  className={`inline-flex h-7 items-center gap-1.5 rounded-md px-2 text-[12px] transition ${selectedMessageSourcesIndex === index
                    ? isDark ? 'bg-white/[0.06] text-emerald-300' : 'bg-emerald-50 text-emerald-700'
                    : isDark ? 'text-zinc-500 hover:bg-white/[0.06] hover:text-emerald-300' : 'text-slate-500 hover:bg-slate-900/5 hover:text-emerald-700'}`}
                  onClick={() => onToggleSources(index, message.sources || [])}
                  title={t('messages.sources')}
                  type="button"
                >
                  <BookOpen size={13} />
                  Sources
                </button>
              )}
              <button
                aria-label={speakingMessageIndex === index ? 'Stop reading' : 'Read aloud'}
                className={`inline-flex h-7 items-center gap-1.5 rounded-md px-2 text-[12px] transition ${isDark
                  ? speakingMessageIndex === index ? 'bg-white/[0.06] text-emerald-300' : 'text-zinc-500 hover:bg-white/[0.06] hover:text-emerald-300'
                  : speakingMessageIndex === index ? 'bg-emerald-50 text-emerald-700' : 'text-slate-500 hover:bg-slate-900/5 hover:text-emerald-700'}`}
                onClick={() => onSpeak(message.content, true, index)}
                title={speakingMessageIndex === index ? 'Stop reading' : 'Read aloud'}
                type="button"
              >
                {speakingMessageIndex === index ? <VolumeX size={13} className="animate-pulse" /> : <Volume2 size={13} />}
                Read
              </button>
              <button
                aria-label="Copy response"
                className={`inline-flex h-7 items-center gap-1.5 rounded-md px-2 text-[12px] transition ${isDark ? 'text-zinc-500 hover:bg-white/[0.06] hover:text-emerald-300' : 'text-slate-500 hover:bg-slate-900/5 hover:text-emerald-700'}`}
                onClick={() => onCopyMessage(index, message.content)}
                title={copiedMessageIndex === index ? 'Copied' : 'Copy response'}
                type="button"
              >
                {copiedMessageIndex === index ? <Check size={13} /> : <Copy size={13} />}
                {copiedMessageIndex === index ? 'Copied' : 'Copy'}
              </button>
            </div>
          )}

          {activeProject && Boolean(extractUnifiedDiff(message.content)) && (
            <button
              className={`mt-3 inline-flex items-center gap-2 rounded-lg border px-3 py-1.5 text-xs font-medium transition ${activeProject.writable
                ? isDark ? 'border-emerald-700 text-emerald-200 hover:bg-emerald-950/40' : 'border-emerald-300 text-emerald-700 hover:bg-emerald-50'
                : isDark ? 'border-zinc-800 text-zinc-500' : 'border-stone-300 text-slate-500'}`}
              disabled={!activeProject.writable}
              onClick={onApplyPatch}
              title={activeProject.writable ? 'Apply the unified diff to the active project' : 'Grant project edit permission before applying patches'}
              type="button"
            >
              <FileCode size={14} />
              Apply changes
            </button>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="flex justify-end gap-3 py-1">
      <div className="group flex max-w-[74%] flex-col items-end">
        {editingMessageIndex === index ? (
          <div className={`w-[min(32rem,78vw)] rounded-2xl border p-2.5 shadow-sm ${isDark ? 'border-emerald-700/60 bg-zinc-950/80' : 'border-emerald-500/60 bg-white'}`}>
            <textarea
              autoFocus
              className={`mb-2 max-h-56 min-h-11 w-full resize-none overflow-hidden rounded-md border px-3 py-2.5 text-sm leading-5 outline-none focus:border-emerald-600 ${isDark ? 'border-zinc-800 bg-zinc-950 text-zinc-100' : 'border-stone-300 bg-white text-slate-900'}`}
              onChange={(e) => { onEditingTextChange(e.target.value); fitTextareaToContent(e.currentTarget); }}
              ref={(ta) => { if (ta) fitTextareaToContent(ta); }}
              rows={1}
              value={editingMessageText}
            />
            <div className="flex justify-end gap-2">
              <button className={`rounded-md border px-3 py-1.5 text-xs ${isDark ? 'border-zinc-800 text-zinc-300 hover:bg-zinc-800' : 'border-stone-300 text-slate-700 hover:bg-stone-100'}`} onClick={onCancelEditing} type="button">{t('messages.editing_cancel')}</button>
              <button className="rounded-md bg-emerald-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-emerald-500 disabled:opacity-60" disabled={!editingMessageText.trim() || isStreaming} onClick={() => onResendEdited(index)} type="button">{t('messages.editing_resend')}</button>
            </div>
          </div>
        ) : (
          <div className={`rounded-2xl px-4 py-2.5 text-[14px] font-medium leading-6 tracking-[-0.006em] shadow-sm ${isDark ? 'bg-emerald-600 text-white shadow-[0_8px_22px_rgba(255,255,255,0.07)]' : 'bg-emerald-700 text-white shadow-[0_12px_28px_rgba(5,150,105,0.24)] ring-1 ring-emerald-900/10'}`}>
            <span className="whitespace-pre-wrap">{message.content || '...'}</span>
          </div>
        )}

        {editingMessageIndex !== index && (
          <div className="mt-1 flex items-center gap-1 opacity-60 hover:opacity-100 focus-within:opacity-100 transition-all duration-150">
            <button
              aria-label="Edit message"
              className={`inline-flex h-7 w-7 items-center justify-center rounded-md transition ${isDark ? 'text-zinc-500 hover:bg-zinc-900 hover:text-emerald-300' : 'text-slate-500 hover:bg-stone-200 hover:text-emerald-700'}`}
              disabled={isStreaming}
              onClick={() => onBeginEditing(index, message.content)}
              title="Edit message"
              type="button"
            >
              <Edit3 size={13} />
            </button>
            <button
              aria-label="Copy message"
              className={`inline-flex h-7 w-7 items-center justify-center rounded-md transition ${isDark ? 'text-zinc-500 hover:bg-zinc-900 hover:text-emerald-300' : 'text-slate-500 hover:bg-stone-200 hover:text-emerald-700'}`}
              onClick={() => onCopyMessage(index, message.content)}
              title={copiedMessageIndex === index ? 'Copied' : 'Copy message'}
              type="button"
            >
              {copiedMessageIndex === index ? <Check size={13} /> : <Copy size={13} />}
            </button>
          </div>
        )}
      </div>
      <div className={`mt-1 flex h-8 w-8 shrink-0 items-center justify-center rounded-xl shadow-sm ${isDark ? 'bg-emerald-700 text-white shadow-white/5' : 'bg-emerald-100 text-emerald-800 shadow-emerald-900/10 ring-1 ring-emerald-700/20'}`}>
        <User size={16} />
      </div>
    </div>
  );
}
