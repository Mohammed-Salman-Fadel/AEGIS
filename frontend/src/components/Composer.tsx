// Chat composer footer with textarea, tools menu (import/calendar/export), voice, and send button
import { useEffect, useRef, useState } from 'react';
import { Upload, Calendar, Download, Wrench, ChevronDown, Mic, Send, Check, FolderOpen, X, BookOpen, Image, Edit3, Clock, Square } from 'lucide-react';
import type { IndexedDocument, CodeProject, ContextUsage } from '../types';
import { importPhaseLabel, fitTextareaToContent, personalizeWelcomeMessage } from '../lib';
import { useTranslate } from '../lib/i18n';
import { DEFAULT_WELCOME_MESSAGES } from '../constants';

interface ComposerProps {
  isDark: boolean;
  isStreaming: boolean;
  isUploading: boolean;
  isClearingIndexedDocuments: boolean;
  isVoiceMode: boolean;
  showCenteredComposer: boolean;
  showImportProgress: boolean;
  toolsOpen: boolean;
  input: string;
  importPhase: string;
  importProgress: number;
  importFileLabel: string;
  indexedDocuments: IndexedDocument[];
  indexedDocumentLabel: string;
  indexedChunkCount: number;
  documentContextNotice: string | null;
  activeProject: CodeProject | null;
  projectEditMessage: string | null;
  tokenMeterLabel: string;
  contextUsage: ContextUsage;
  reasoningEnabled: boolean;
  messageQueue: string[];
  activeWelcomeMessage: string;
  profileText: string;
  fileInputRef: React.RefObject<HTMLInputElement | null>;
  imageFileInputRef: React.RefObject<HTMLInputElement | null>;
  composerTextareaRef: React.RefObject<HTMLTextAreaElement | null>;
  onInputChange: (value: string) => void;
  onSubmit: (e: React.FormEvent<HTMLFormElement>) => void;
  onToggleTools: () => void;
  onImportClick: () => void;
  onCalendarOpen: () => void;
  onExportPdf: () => void;
  obsidianEnabled: boolean;
  onObsidianOpen: () => void;
  onFileUpload: (e: React.ChangeEvent<HTMLInputElement>) => void;
  onImageUpload: (e: React.ChangeEvent<HTMLInputElement>) => void;
  onClearDocuments: () => void;
  onVoiceModeOpen: () => void;
  onToggleReasoning: () => void;
  onDetachProject: () => void;
  onStopGeneration: () => void;
  onRemoveFromQueue: (index: number) => void;
  onEditFromQueue: (index: number) => void;
  onSaveQueueEdit: () => void;
  onCancelQueueEdit: () => void;
  queueEditIndex: number | null;
  availableModels: { name: string; description: string; active: boolean }[];
  onSelectModel: (name: string) => void;
  modelSwitching: boolean;
}

export function Composer({
  isDark, isStreaming, isUploading, isClearingIndexedDocuments, isVoiceMode,
  showCenteredComposer, showImportProgress, toolsOpen, input,
  importPhase, importProgress, importFileLabel,
  indexedDocuments, indexedDocumentLabel, indexedChunkCount,
  documentContextNotice, activeProject, projectEditMessage,
  tokenMeterLabel, contextUsage, reasoningEnabled,
  messageQueue,
  activeWelcomeMessage, profileText,
  fileInputRef, imageFileInputRef, composerTextareaRef,
  obsidianEnabled, onInputChange, onSubmit, onToggleTools, onImportClick, onCalendarOpen, onExportPdf, onObsidianOpen,
  onFileUpload, onImageUpload, onClearDocuments, onVoiceModeOpen, onDetachProject, onStopGeneration,
  onToggleReasoning,
  onRemoveFromQueue, onEditFromQueue, onSaveQueueEdit, onCancelQueueEdit, queueEditIndex,
  availableModels, onSelectModel, modelSwitching,
}: ComposerProps) {
  const { t, lang } = useTranslate();
  const [queueExpanded, setQueueExpanded] = useState(false);
  const [modelDropdownOpen, setModelDropdownOpen] = useState(false);
  const toolsMenuRef = useRef<HTMLDivElement>(null);
  const modelMenuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setModelDropdownOpen(false);
  }, [showCenteredComposer, isStreaming]);

  useEffect(() => {
    if (!toolsOpen && !modelDropdownOpen) return;
    const closeOnOutsideOrEscape = (event: MouseEvent | KeyboardEvent) => {
      if (event instanceof KeyboardEvent) {
        if (event.key === 'Escape') {
          if (toolsOpen) onToggleTools();
          if (modelDropdownOpen) setModelDropdownOpen(false);
        }
        return;
      }
      if (toolsMenuRef.current && !toolsMenuRef.current.contains(event.target as Node) && toolsOpen) onToggleTools();
      if (modelMenuRef.current && !modelMenuRef.current.contains(event.target as Node)) setModelDropdownOpen(false);
    };
    document.addEventListener('mousedown', closeOnOutsideOrEscape);
    document.addEventListener('keydown', closeOnOutsideOrEscape);
    return () => { document.removeEventListener('mousedown', closeOnOutsideOrEscape); document.removeEventListener('keydown', closeOnOutsideOrEscape); };
  }, [modelDropdownOpen, onToggleTools, toolsOpen]);

  const handleSubmit = (e: React.FormEvent<HTMLFormElement>) => {
    setModelDropdownOpen(false);
    onSubmit(e);
  };

  const handleMenuKeys = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return;
    const items = Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>('button:not([disabled])'));
    if (items.length === 0) return;
    event.preventDefault();
    const current = items.indexOf(document.activeElement as HTMLButtonElement);
    const next = event.key === 'Home' ? 0 : event.key === 'End' ? items.length - 1 : event.key === 'ArrowDown' ? (current + 1 + items.length) % items.length : (current - 1 + items.length) % items.length;
    items[next].focus();
  };

  return (
    <footer className={`px-5 transition-all duration-500 ease-out sm:px-8 ${
      showCenteredComposer
        ? 'pointer-events-none absolute inset-x-0 top-1/2 z-20 -translate-y-1/2 pb-0 pt-0'
        : 'aegis-composer-footer relative shrink-0 pb-4 pt-5'
    }`}>
      {!showCenteredComposer && (
        <div className="aegis-composer-fade pointer-events-none absolute inset-x-0 -top-14 h-24" />
      )}

      {showCenteredComposer && (
        <div className="pointer-events-auto mx-auto mb-[clamp(0.65rem,1.6vh,1.4rem)] flex w-full max-w-[48rem] flex-col items-center text-center select-none">
          <div className={`aegis-display welcome-message mt-0 max-w-[42rem] text-[clamp(1.02rem,1.55vw,1.2rem)] font-medium leading-snug tracking-[-0.018em] ${isDark ? 'text-zinc-200' : 'text-stone-800'}`}>
            {(() => {
              const idx = DEFAULT_WELCOME_MESSAGES.indexOf(activeWelcomeMessage);
              const msg = idx >= 0 ? t('welcome.' + idx) : activeWelcomeMessage;
              return personalizeWelcomeMessage(msg, profileText, lang);
            })()}
          </div>
        </div>
      )}

      {showImportProgress && (
        <div className={`mx-auto mb-3 max-w-[50rem] rounded-xl border px-3.5 py-2.5 ${importPhase === 'error'
          ? isDark ? 'border-red-900/70 bg-red-950/20 text-red-200' : 'border-red-200 bg-red-50 text-red-800'
          : isDark ? 'border-zinc-800 bg-zinc-900/80 text-zinc-200' : 'border-stone-300 bg-white text-slate-700'}`}>
          <div className="mb-2 flex items-center justify-between gap-3 text-xs">
            <span className="truncate">{importPhaseLabel(importPhase as any, importFileLabel)}</span>
            <span className="font-mono">{importProgress}%</span>
          </div>
          <div className={`h-1.5 overflow-hidden rounded-full ${isDark ? 'bg-zinc-800' : 'bg-stone-200'}`} role="progressbar" aria-label="Document import progress" aria-valuemin={0} aria-valuemax={100} aria-valuenow={importProgress}>
            <div className={`h-full rounded-full transition-all duration-300 ${importPhase === 'error' ? 'bg-red-500' : importPhase === 'complete' ? 'bg-emerald-500' : 'bg-emerald-400'} ${importPhase === 'indexing' ? 'animate-pulse' : ''}`} style={{ width: `${importProgress}%` }} />
          </div>
        </div>
      )}

      {indexedDocuments.length > 0 && (
        <div aria-busy={isClearingIndexedDocuments} className={`group relative mx-auto mb-3 flex max-w-[50rem] items-center gap-2 rounded-xl border py-2 pl-3.5 pr-9 text-xs transition-all duration-[1800ms] ease-out ${
          isClearingIndexedDocuments
            ? isDark ? 'border-red-900/70 bg-red-950/30 text-red-200 opacity-35' : 'border-red-300 bg-red-50 text-red-700 opacity-35'
            : isDark ? 'border-emerald-900/60 bg-emerald-950/20 text-emerald-200 opacity-100' : 'border-emerald-200 bg-emerald-50 text-emerald-800 opacity-100'
        }`}>
          <Upload className="shrink-0" size={14} />
          <span className="min-w-0 truncate">Document context active: {indexedDocumentLabel} indexed into {indexedChunkCount} chunks.</span>
          {!isClearingIndexedDocuments && (
            <button
              aria-label="Remove imported document context"
              className={`absolute right-2 top-1/2 inline-flex h-6 w-6 -translate-y-1/2 items-center justify-center rounded-md opacity-0 transition group-hover:opacity-100 group-focus-within:opacity-100 disabled:cursor-not-allowed ${isDark ? 'text-emerald-100/80 hover:bg-emerald-900/40 hover:text-emerald-50 disabled:text-emerald-200/45' : 'text-emerald-800/70 hover:bg-emerald-100 hover:text-emerald-950 disabled:text-emerald-700/45'}`}
              disabled={isUploading || isStreaming}
              onClick={onClearDocuments}
              title="Remove imported document context"
              type="button"
            >
              <X size={14} />
            </button>
          )}
        </div>
      )}

      {documentContextNotice && (
        <div className={`mx-auto mb-3 flex max-w-[50rem] items-start gap-2 rounded-xl border px-3.5 py-2.5 text-xs ${isDark ? 'border-zinc-800 bg-zinc-900/70 text-zinc-300' : 'border-stone-300 bg-white text-slate-600'}`}>
          <Check className="mt-0.5 shrink-0" size={14} />
          <span className="min-w-0">{documentContextNotice}</span>
        </div>
      )}

      {activeProject && (
        <div className={`mx-auto mb-3 flex max-w-[50rem] items-center justify-between gap-3 rounded-xl border px-3.5 py-2.5 text-xs ${isDark ? 'border-sky-900/60 bg-sky-950/20 text-sky-200' : 'border-sky-200 bg-sky-50 text-sky-800'}`}>
          <span className="flex min-w-0 items-center gap-2">
            <FolderOpen size={14} />
            <span className="truncate">Project context active: {activeProject.name} &middot; {activeProject.fileCount} files &middot; {activeProject.writable ? 'edits require patch approval' : 'read-only'}</span>
          </span>
          <button aria-label="Detach project context" className={`shrink-0 rounded-md p-1 transition ${isDark ? 'hover:bg-sky-900/40' : 'hover:bg-sky-100'}`} onClick={onDetachProject} type="button">
            <X size={14} />
          </button>
        </div>
      )}

      {projectEditMessage && (
        <div className={`mx-auto mb-3 max-w-[50rem] rounded-xl border px-3.5 py-2.5 text-xs ${isDark ? 'border-zinc-800 bg-zinc-900/70 text-zinc-300' : 'border-stone-300 bg-white text-slate-600'}`}>
          {projectEditMessage}
        </div>
      )}

      {/* ── Message queue display ── */}
      {messageQueue.length > 0 && (
        <div className="mx-auto mb-2 max-w-[50rem]">
          <div className={`flex w-full items-center gap-1.5 rounded-md border px-2 py-1 text-left text-[12px] transition-all duration-200 ${isDark ? 'border-amber-800/40 bg-amber-950/20 text-amber-400 hover:bg-amber-950/40' : 'border-amber-300/50 bg-amber-50/70 text-amber-700 hover:bg-amber-100/80'}`}>
          <button className="flex min-w-0 flex-1 items-center gap-1.5 text-left" onClick={() => setQueueExpanded(!queueExpanded)} type="button">
            <span className="font-medium">{queueEditIndex !== null ? 'Editing...' : 'Queue'}</span>
            <span className={`text-[10px] opacity-50 ${isDark ? 'text-amber-400' : 'text-amber-700'}`}>
              {queueEditIndex !== null
                ? `item ${queueEditIndex + 1} of ${messageQueue.length}`
                : messageQueue.length > 1
                  ? `- ${messageQueue.length} messages`
                  : '- 1 message'}
            </span>
            {queueEditIndex === null && (
              <ChevronDown size={11} className={`ml-auto shrink-0 transition-transform duration-200 ${queueExpanded ? 'rotate-180' : ''}`} />
            )}
          </button>
          {queueEditIndex !== null && (
            <div className="ml-auto flex shrink-0 gap-1">
              <button className={`rounded px-1.5 py-0.5 text-[10px] font-semibold ${isDark ? 'bg-amber-600/30 text-amber-300 hover:bg-amber-600/50' : 'bg-amber-500/20 text-amber-800 hover:bg-amber-500/30'}`} onClick={onSaveQueueEdit} type="button">Save</button>
              <button className="rounded px-1.5 py-0.5 text-[10px] font-semibold" onClick={onCancelQueueEdit} type="button">Cancel</button>
            </div>
          )}
          </div>

          <div
            className={`overflow-hidden transition-all duration-250 ease-in-out ${
              queueExpanded ? 'max-h-48 opacity-100 mt-0.5' : 'max-h-0 opacity-0'
            }`}
          >
            <div className={`rounded-md border p-1.5 ${isDark ? 'border-amber-800/20 bg-amber-950/15' : 'border-amber-300/20 bg-amber-50/60'}`}>
              <div className="max-h-36 space-y-0.5 overflow-y-auto">
                {messageQueue.map((msg, i) => (
                  <div key={i} className={`group flex items-center gap-1.5 rounded px-1.5 py-1 text-[11px] ${isDark ? 'text-amber-300/70 hover:bg-amber-950/20' : 'text-amber-800/70 hover:bg-amber-50'}`}>
                    <span className={`w-3 shrink-0 text-right text-[10px] font-mono ${isDark ? 'text-amber-400/50' : 'text-amber-700/50'}`}>{i + 1}.</span>
                    <span className="min-w-0 flex-1 truncate">{msg}</span>
                    <div className="flex shrink-0 gap-0.5 opacity-100 transition-opacity sm:opacity-0 sm:group-hover:opacity-100 sm:group-focus-within:opacity-100">
                      <button
                        className={`rounded p-0.5 ${isDark ? 'text-amber-400/60 hover:text-amber-300' : 'text-amber-700/60 hover:text-amber-600'}`}
                        onClick={() => onEditFromQueue(i)}
                        title="Edit message"
                        type="button"
                      >
                        <Edit3 size={11} />
                      </button>
                      <button
                        className={`rounded p-0.5 ${isDark ? 'text-amber-400/60 hover:text-red-400' : 'text-amber-700/60 hover:text-red-600'}`}
                        onClick={() => onRemoveFromQueue(i)}
                        title="Remove from queue"
                        type="button"
                      >
                        <X size={11} />
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </div>
      )}

      <form className={`pointer-events-auto mx-auto transition-all duration-500 ease-out ${showCenteredComposer ? 'max-w-[44rem]' : 'max-w-[50rem]'} ${showCenteredComposer ? 'mt-0' : ''}`} onSubmit={handleSubmit}>
        <input accept=".pdf,.txt" className="hidden" disabled={isStreaming || isUploading} multiple onChange={onFileUpload} ref={fileInputRef} title="Supported files: PDF, TXT" type="file" />
        <input accept=".png,.jpg,.jpeg,.gif,.webp,.bmp" className="hidden" disabled={isStreaming} onChange={onImageUpload} ref={imageFileInputRef} title="Supported image files: PNG, JPG, GIF, WEBP, BMP" type="file" />
        <div className={`aegis-composer-input-shell border shadow-sm backdrop-blur-xl transition-all duration-500 ease-out focus-within:-translate-y-0.5 ${showCenteredComposer ? 'rounded-[1.75rem] px-4 py-3' : 'rounded-[1.4rem] px-3 py-2.5'} ${isDark ? 'border-white/10 bg-zinc-950/82 text-zinc-100 shadow-black/25' : 'aegis-light-panel text-stone-950'}`}>
          <textarea
            className={`w-full resize-none bg-transparent px-2 pt-1 text-[16px] leading-7 tracking-[-0.004em] outline-none ${showCenteredComposer ? 'max-h-[40vh] min-h-[5rem]' : 'max-h-[55vh] min-h-[3.1rem]'} ${isDark ? 'placeholder:text-zinc-600' : 'placeholder:text-stone-500'}`}
            disabled={isStreaming && isUploading}
            onChange={(e) => onInputChange(e.target.value)}
            onInput={(e) => fitTextareaToContent(e.currentTarget)}
            onKeyDown={(e) => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); e.currentTarget.form?.requestSubmit(); } }}
            placeholder={t('composer.placeholder')}
            ref={composerTextareaRef}
            rows={1}
            value={input}
          />
          <div className="mt-2 flex flex-wrap items-center justify-between gap-2">
            <div className="relative" ref={toolsMenuRef}> {/* Tools button */}
              {isStreaming && (
                <button
                  aria-label="Stop generation"
                  className="inline-flex h-8 items-center gap-2 rounded-full bg-red-600 px-3 text-xs font-medium text-white transition hover:bg-red-500"
                  onClick={onStopGeneration}
                  type="button"
                >
                  <Square size={14} fill="currentColor" />
                  <span className="hidden sm:inline">Stop</span>
                </button>
              )}
              <button
                aria-expanded={toolsOpen}
                aria-controls="composer-tools-menu"
                aria-haspopup="menu"
                className={`inline-flex h-8 items-center gap-1.5 rounded-full border px-2.5 text-[12px] font-medium transition-all duration-200 ${isStreaming ? 'cursor-not-allowed opacity-60' : ''} ${toolsOpen ? '-translate-y-0.5 scale-[0.98]' : 'translate-y-0 scale-100'} ${toolsOpen ? 'aegis-accent-subtle' : isDark ? 'border-transparent text-zinc-500 hover:bg-white/[0.06] hover:text-zinc-200' : 'border-transparent text-stone-600 hover:bg-stone-900/6 hover:text-stone-950'}`}
                disabled={isStreaming}
                onClick={onToggleTools}
                type="button"
                title={t('composer.tools')}
              >
                <Wrench className={toolsOpen ? 'rotate-12 transition-transform' : 'transition-transform'} size={15} />
                <span className="hidden sm:inline">{t('composer.tools')}</span>
                <ChevronDown className={`transition-transform duration-200 ${toolsOpen ? 'rotate-180' : 'rotate-0'}`} size={13} />
              </button>
              {toolsOpen && (
                <div className={`absolute bottom-full left-0 z-20 mb-2 w-48 animate-[toolsMenuIn_160ms_ease-out] rounded-lg border p-1 shadow-xl ${isDark ? 'border-zinc-800 bg-zinc-950 text-zinc-100' : 'border-[rgba(94,76,55,0.26)] bg-[rgba(255,250,240,0.96)] text-stone-950 shadow-[0_18px_44px_rgba(80,62,39,0.18)]'}`} id="composer-tools-menu" onKeyDown={handleMenuKeys} role="menu">
                  <button className={`flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm ${isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'}`} disabled={isStreaming || isUploading} onClick={onImportClick} role="menuitem" type="button">
                    <Upload size={15} /> {t('composer.import')}
                  </button>
                  <button className={`flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm disabled:opacity-50 ${isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'}`} onClick={onCalendarOpen} role="menuitem" type="button">
                    <Calendar size={15} /> {t('composer.calendar')}
                  </button>
                  <button className={`flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm disabled:opacity-50 ${isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'}`} disabled={false} onClick={onExportPdf} role="menuitem" type="button">
                    <Download size={15} /> {t('composer.export')}
                  </button>
                  <button className={`flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm disabled:opacity-50 ${isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'}`} disabled={isStreaming} onClick={() => imageFileInputRef.current?.click()} role="menuitem" type="button">
                    <Image size={15} /> Image
                  </button>
                  {obsidianEnabled && (
                    <button className={`flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm disabled:opacity-50 ${isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'}`} onClick={onObsidianOpen} role="menuitem" type="button">
                      <BookOpen size={15} /> Obsidian
                    </button>
                  )}
                </div>
              )}
            </div>
            <div className="flex flex-1 items-center justify-end gap-1.5">
              <span className={`hidden rounded-full px-2 py-1 font-mono text-[10px] md:inline-flex ${isDark ? 'bg-white/5 text-zinc-600' : 'bg-slate-900/5 text-slate-400'}`} title={`${contextUsage.model || 'Active model'} context usage from the last completed inference`}>
                {tokenMeterLabel}
              </span>
              <button
                aria-checked={reasoningEnabled}
                aria-label="Toggle deeper reasoning"
                className={`aegis-accent-ghost inline-flex h-8 items-center gap-2 rounded-full border px-2.5 text-[12px] font-medium transition-all duration-200 ${
                  reasoningEnabled
                    ? isDark ? 'border-transparent text-zinc-100' : 'border-transparent text-black'
                    : isDark ? 'border-transparent text-zinc-500 hover:bg-white/[0.06]' : 'border-transparent text-stone-600 hover:bg-stone-900/6'
                } ${isStreaming ? 'cursor-not-allowed opacity-60' : ''}`}
                disabled={isStreaming}
                onClick={onToggleReasoning}
                role="switch"
                title="Toggle deeper reasoning for general chat. Code tasks may still use a read-only reasoning loop automatically."
                type="button"
              >
                <span>Reasoning</span>
                <span className={`relative h-5 w-9 rounded-full transition-colors duration-200 ${reasoningEnabled ? 'aegis-reasoning-switch-active' : isDark ? 'bg-zinc-700' : 'bg-stone-300'}`}>
                  <span className={`absolute left-0.5 top-0.5 h-4 w-4 rounded-full bg-white shadow-sm transition-transform duration-200 ${reasoningEnabled ? 'translate-x-4' : 'translate-x-0'}`} />
                </span>
              </button>
              <button
                className={`inline-flex h-8 w-8 items-center justify-center rounded-full transition-all duration-200 ${isVoiceMode ? 'aegis-accent-chip-active text-white' : isDark ? 'text-zinc-500 hover:bg-white/[0.06] hover:text-emerald-300' : 'text-stone-600 hover:bg-stone-900/6 hover:text-emerald-800'}`}
                onClick={onVoiceModeOpen}
                type="button"
                title="Voice Mode"
              >
                <Mic size={17} />
              </button>
              {/* ── Model dropdown ── */}
              <div className="relative" ref={modelMenuRef}>
                <button
                aria-controls="composer-model-menu"
                aria-expanded={modelDropdownOpen}
                aria-haspopup="menu"
                className={`aegis-accent-ghost inline-flex h-8 items-center gap-1 rounded-full border px-2 text-[12px] font-medium transition-all duration-200 ${
                    isStreaming ? 'cursor-not-allowed opacity-60' : ''
                  } ${modelDropdownOpen ? '-translate-y-0.5 scale-[0.98]' : 'translate-y-0 scale-100'} ${
                    modelDropdownOpen
                      ? 'aegis-accent-subtle'
                      : isDark ? 'border-transparent text-zinc-500 hover:bg-white/[0.06]' : 'border-transparent text-stone-600 hover:bg-stone-900/6'
                  }`}
                  disabled={isStreaming}
                  onClick={() => setModelDropdownOpen(!modelDropdownOpen)}
                  type="button"
                  title="Switch model"
                >
                  <span className="max-w-[72px] truncate text-center tracking-wide">{contextUsage.model ? contextUsage.model.split(':')[0] : 'Model'}</span>
                  <ChevronDown size={11} className={`shrink-0 transition-transform duration-200 ${modelDropdownOpen ? 'rotate-180' : ''}`} />
                </button>

                {/* ── Model switching progress bar ── */}
                <div
                  className={`pointer-events-none absolute -bottom-[5px] left-1/2 h-[3px] -translate-x-1/2 rounded-full transition-all duration-300 ${
                    modelSwitching ? 'w-4/5 opacity-100' : 'w-0 opacity-0'
                  } ${isDark ? 'bg-emerald-400' : 'bg-emerald-500'}`}
                  style={{ animation: modelSwitching ? 'pulse 1.2s ease-in-out infinite' : 'none' }}
                />

                {modelDropdownOpen && (
                  <div
                    className={`absolute bottom-full right-0 z-30 mb-2 w-44 animate-[toolsMenuIn_160ms_ease-out] rounded-lg border py-1 shadow-xl ${
                      isDark ? 'border-zinc-800 bg-zinc-950 text-zinc-100' : 'border-[rgba(94,76,55,0.26)] bg-[rgba(255,250,240,0.96)] text-stone-950 shadow-[0_18px_44px_rgba(80,62,39,0.18)]'
                    }`} id="composer-model-menu" onKeyDown={handleMenuKeys} role="menu"
                  >
                    <div className={`px-2.5 py-1 text-[11px] font-medium uppercase tracking-wider ${isDark ? 'text-zinc-500' : 'text-slate-400'}`}>
                      Installed Models
                    </div>
                    {availableModels.length === 0 ? (
                      <div className={`px-3 py-3 text-center text-[11px] ${isDark ? 'text-zinc-600' : 'text-slate-400'}`}>
                        No models found. Install one in Settings.
                      </div>
                    ) : (
                      <div className="max-h-48 overflow-y-auto">
                        {availableModels.map((m) => (
                          <button
                            key={m.name}
                            className={`flex w-full items-center gap-1.5 px-2.5 py-1.5 text-left text-[12px] transition-colors ${
                              m.active
                                ? isDark ? 'bg-emerald-950/30 text-emerald-300' : 'bg-emerald-100/70 text-emerald-800'
                                : isDark ? 'text-zinc-300 hover:bg-zinc-900' : 'text-stone-700 hover:bg-[rgba(94,76,55,0.08)]'
                            }`}
                            onClick={() => { onSelectModel(m.name); setModelDropdownOpen(false); }} role="menuitem"
                            type="button"
                          >
                            <span className={`shrink-0 text-[9px] ${m.active ? 'opacity-100' : 'opacity-0'}`}>
                              {m.active ? 'on' : ''}
                            </span>
                            <span className="min-w-0 flex-1 truncate">{m.name}</span>
                            {m.active && (
                              <span className={`shrink-0 text-[9px] ${isDark ? 'text-emerald-400' : 'text-emerald-600'}`}>active</span>
                            )}
                          </button>
                        ))}
                      </div>
                    )}
                  </div>
                )}
              </div>
              <button
                className={`inline-flex h-8 items-center gap-2 rounded-full px-3 text-xs font-medium text-white transition-all duration-200 ${isStreaming && input.trim() ? 'aegis-accent-chip-active' : 'aegis-accent-solid'} disabled:opacity-60`}
                disabled={isStreaming && !input.trim()}
                type="submit"
              >
                {isStreaming && input.trim() ? (
                  <>
                    <Clock size={15} />
                    <span className="hidden sm:inline">Queue</span>
                  </>
                ) : (
                  <>
                    <Send size={15} />
                    <span className="hidden sm:inline">{t('composer.send')}</span>
                  </>
                )}
              </button>
            </div>
          </div>
        </div>
      </form>
    </footer>
  );
}
