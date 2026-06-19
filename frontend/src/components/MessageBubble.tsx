// Individual chat message bubble with editing, sources, TTS, copy, and patch apply
import { Bot, User, Edit3, Copy, Check, BookOpen, Volume2, VolumeX, FileCode } from 'lucide-react';
import type { Message, CodeProject } from '../types';
import { copyTextToClipboard, extractUnifiedDiff, fitTextareaToContent } from '../lib';
import { AssistantMarkdown } from './AssistantMarkdown';
import { useT } from '../lib/i18n';

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
  onToggleSources: (index: number, sources: any[]) => void;
  onSpeak: (text: string, force: boolean, index: number) => void;
  onApplyPatch: () => void;
}

export function MessageBubble({
  message, index, isDark, isStreaming, editingMessageIndex, editingMessageText,
  copiedMessageIndex, selectedMessageSourcesIndex, speakingMessageIndex, activeProject,
  onBeginEditing, onCancelEditing, onEditingTextChange, onResendEdited,
  onCopyMessage, onToggleSources, onSpeak,   onApplyPatch,
}: MessageBubbleProps) {
  const t = useT();
  return (
    <div className={`flex gap-3 ${message.role === 'user' ? 'justify-end' : 'justify-start'}`}>
      {message.role === 'assistant' && (
        <div className={`mt-1 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg ${isDark ? 'bg-zinc-800 text-zinc-200 shadow-sm shadow-white/5' : 'bg-white text-slate-700 shadow-sm shadow-stone-300/70 ring-1 ring-stone-200'}`}>
          <Bot size={16} />
        </div>
      )}
      <div className={`group flex max-w-[78%] flex-col ${message.role === 'user' ? 'items-end' : 'items-start'}`}>
        {editingMessageIndex === index && message.role === 'user' ? (
          <div className={`w-[min(32rem,78vw)] rounded-lg border p-2.5 shadow-sm ${isDark ? 'border-emerald-700 bg-zinc-900' : 'border-emerald-500 bg-white'}`}>
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
          <div className={`rounded-lg px-4 py-3 text-sm leading-6 shadow-sm ${message.role === 'user'
            ? isDark ? 'bg-emerald-600 text-white shadow-[0_8px_22px_rgba(255,255,255,0.07)]' : 'bg-emerald-600 text-white shadow-[0_10px_24px_rgba(16,185,129,0.24)]'
            : isDark ? 'border border-zinc-800 bg-zinc-900 text-zinc-200 shadow-[0_8px_22px_rgba(255,255,255,0.065)]' : 'border border-stone-200 bg-white/95 text-slate-800 shadow-[0_10px_26px_rgba(120,113,108,0.20)]'}`}
          >
            {message.role === 'assistant' ? (
              <AssistantMarkdown content={message.content} isDark={isDark} />
            ) : (
              <span className="whitespace-pre-wrap">{message.content || '...'}</span>
            )}
          </div>
        )}

        {/* Assistant action buttons */}
        {message.role === 'assistant' && message.content && (
          <div className="mt-1 flex items-center gap-1 opacity-60 hover:opacity-100 focus-within:opacity-100 transition-all duration-150">
            {message.sources && message.sources.length > 0 && (
              <button
                aria-label="Inspect retrieved sources"
                className={`inline-flex h-7 w-7 items-center justify-center rounded-md transition ${selectedMessageSourcesIndex === index
                  ? isDark ? 'text-emerald-400 bg-zinc-900 border border-emerald-500/20' : 'text-emerald-600 bg-stone-200 border border-emerald-300/30'
                  : isDark ? 'text-zinc-500 hover:bg-zinc-900 hover:text-emerald-300' : 'text-slate-500 hover:bg-stone-200 hover:text-emerald-700'}`}
                onClick={() => onToggleSources(index, message.sources || [])}
                title={t('messages.sources')}
                type="button"
              >
                <BookOpen size={13} className={selectedMessageSourcesIndex === index ? 'animate-pulse' : ''} />
              </button>
            )}
            <button
              aria-label={speakingMessageIndex === index ? 'Stop reading' : 'Read aloud'}
              className={`inline-flex h-7 w-7 items-center justify-center rounded-md transition ${isDark
                ? speakingMessageIndex === index ? 'text-emerald-400 bg-zinc-900' : 'text-zinc-500 hover:bg-zinc-900 hover:text-emerald-300'
                : speakingMessageIndex === index ? 'text-emerald-600 bg-stone-200' : 'text-slate-500 hover:bg-stone-200 hover:text-emerald-700'}`}
              onClick={() => onSpeak(message.content, true, index)}
              title={speakingMessageIndex === index ? 'Stop reading' : 'Read aloud'}
              type="button"
            >
              {speakingMessageIndex === index ? <VolumeX size={13} className="animate-pulse" /> : <Volume2 size={13} />}
            </button>
            <button
              aria-label="Copy response"
              className={`inline-flex h-7 w-7 items-center justify-center rounded-md transition ${isDark ? 'text-zinc-500 hover:bg-zinc-900 hover:text-emerald-300' : 'text-slate-500 hover:bg-stone-200 hover:text-emerald-700'}`}
              onClick={() => onCopyMessage(index, message.content)}
              title={copiedMessageIndex === index ? 'Copied' : 'Copy response'}
              type="button"
            >
              {copiedMessageIndex === index ? <Check size={13} /> : <Copy size={13} />}
            </button>
          </div>
        )}

        {/* User action buttons */}
        {message.role === 'user' && editingMessageIndex !== index && (
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

        {/* Patch apply button */}
        {message.role === 'assistant' && activeProject && Boolean(extractUnifiedDiff(message.content)) && (
          <button
            className={`mt-2 inline-flex items-center gap-2 rounded-lg border px-3 py-1.5 text-xs font-medium transition ${activeProject.writable
              ? isDark ? 'border-emerald-700 text-emerald-200 hover:bg-emerald-950/40' : 'border-emerald-300 text-emerald-700 hover:bg-emerald-50'
              : isDark ? 'border-zinc-800 text-zinc-500' : 'border-stone-300 text-slate-500'}`}
            disabled={!activeProject.writable}
            onClick={onApplyPatch}
            title={activeProject.writable ? 'Apply the unified diff to the active project' : 'Grant project edit permission before applying patches'}
            type="button"
          >
            <FileCode size={14} />
            Apply suggested patch
          </button>
        )}
      </div>
      {message.role === 'user' && (
        <div className={`mt-1 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg shadow-sm ${isDark ? 'bg-emerald-700 text-white shadow-white/5' : 'bg-emerald-50 text-emerald-700 shadow-emerald-100 ring-1 ring-emerald-200'}`}>
          <User size={16} />
        </div>
      )}
    </div>
  );
}
