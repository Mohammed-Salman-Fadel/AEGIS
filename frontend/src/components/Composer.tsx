// Chat composer footer with textarea, tools menu (import/calendar/export), voice, and send button
import { Upload, Calendar, Download, Wrench, ChevronDown, Mic, Send, Check, FolderOpen, X } from 'lucide-react';
import type { IndexedDocument, CodeProject, ContextUsage } from '../types';
import { importPhaseLabel, formatTokenMeter, fitTextareaToContent, personalizeWelcomeMessage } from '../lib';
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
  activeWelcomeMessage: string;
  profileText: string;
  fileInputRef: React.RefObject<HTMLInputElement | null>;
  composerTextareaRef: React.RefObject<HTMLTextAreaElement | null>;
  onInputChange: (value: string) => void;
  onSubmit: (e: React.FormEvent<HTMLFormElement>) => void;
  onToggleTools: () => void;
  onImportClick: () => void;
  onCalendarOpen: () => void;
  onExportPdf: () => void;
  onFileUpload: (e: React.ChangeEvent<HTMLInputElement>) => void;
  onClearDocuments: () => void;
  onVoiceModeOpen: () => void;
  onDetachProject: () => void;
}

export function Composer({
  isDark, isStreaming, isUploading, isClearingIndexedDocuments, isVoiceMode,
  showCenteredComposer, showImportProgress, toolsOpen, input,
  importPhase, importProgress, importFileLabel,
  indexedDocuments, indexedDocumentLabel, indexedChunkCount,
  documentContextNotice, activeProject, projectEditMessage,
  tokenMeterLabel, contextUsage,
  activeWelcomeMessage, profileText,
  fileInputRef, composerTextareaRef,
  onInputChange, onSubmit, onToggleTools, onImportClick, onCalendarOpen, onExportPdf,
  onFileUpload, onClearDocuments, onVoiceModeOpen, onDetachProject,
}: ComposerProps) {
  const { t, lang } = useTranslate();
  return (
    <footer className={`px-4 transition-all duration-500 ease-out ${
      showCenteredComposer
        ? 'pointer-events-none absolute inset-x-0 top-1/2 z-20 -translate-y-1/2 pb-0 pt-0'
        : `relative shrink-0 pb-4 pt-5 ${isDark ? 'bg-zinc-950/95 shadow-[0_-24px_42px_rgba(0,0,0,0.35)]' : 'bg-stone-100/95 shadow-[0_-24px_42px_rgba(120,113,108,0.18)]'}`
    }`}>
      {!showCenteredComposer && (
        <div className={`pointer-events-none absolute inset-x-0 -top-8 h-8 ${isDark ? 'bg-gradient-to-t from-zinc-950/95 to-transparent' : 'bg-gradient-to-t from-stone-100/95 to-transparent'}`} />
      )}

      {showCenteredComposer && (
        <div className={`welcome-message pointer-events-auto mx-auto mb-5 max-w-2xl text-center text-xl font-semibold ${isDark ? 'text-zinc-100' : 'text-slate-900'}`}>
          {(() => {
            const idx = DEFAULT_WELCOME_MESSAGES.indexOf(activeWelcomeMessage);
            const msg = idx >= 0 ? t('welcome.' + idx) : activeWelcomeMessage;
            return personalizeWelcomeMessage(msg, profileText, lang);
          })()}
        </div>
      )}

      {showImportProgress && (
        <div className={`mx-auto mb-3 max-w-3xl rounded-lg border px-3 py-2 ${importPhase === 'error'
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
        <div aria-busy={isClearingIndexedDocuments} className={`group relative mx-auto mb-3 flex max-w-3xl items-center gap-2 rounded-lg border py-2 pl-3 pr-9 text-xs transition-all duration-[1800ms] ease-out ${
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
        <div className={`mx-auto mb-3 flex max-w-3xl items-start gap-2 rounded-lg border px-3 py-2 text-xs ${isDark ? 'border-zinc-800 bg-zinc-900/70 text-zinc-300' : 'border-stone-300 bg-white text-slate-600'}`}>
          <Check className="mt-0.5 shrink-0" size={14} />
          <span className="min-w-0">{documentContextNotice}</span>
        </div>
      )}

      {activeProject && (
        <div className={`mx-auto mb-3 flex max-w-3xl items-center justify-between gap-3 rounded-lg border px-3 py-2 text-xs ${isDark ? 'border-sky-900/60 bg-sky-950/20 text-sky-200' : 'border-sky-200 bg-sky-50 text-sky-800'}`}>
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
        <div className={`mx-auto mb-3 max-w-3xl rounded-lg border px-3 py-2 text-xs ${isDark ? 'border-zinc-800 bg-zinc-900/70 text-zinc-300' : 'border-stone-300 bg-white text-slate-600'}`}>
          {projectEditMessage}
        </div>
      )}

      <form className={`pointer-events-auto mx-auto transition-all duration-500 ease-out ${showCenteredComposer ? 'max-w-2xl' : 'max-w-3xl'}`} onSubmit={onSubmit}>
        <input accept=".pdf,.txt" className="hidden" disabled={isStreaming || isUploading} multiple onChange={onFileUpload} ref={fileInputRef} title="Supported files: PDF, TXT" type="file" />
        <div className={`border shadow-sm transition-all duration-500 ease-out ${showCenteredComposer ? 'rounded-[1.75rem] px-4 pb-3 pt-3' : 'rounded-xl px-3 pb-2.5 pt-3'} ${isDark ? 'border-zinc-800 bg-zinc-950/92 text-zinc-100 shadow-black/30' : 'border-stone-300 bg-white text-slate-900 shadow-stone-300/30'}`}>
          <textarea
            className={`w-full resize-none bg-transparent text-sm leading-6 outline-none ${showCenteredComposer ? 'max-h-28 min-h-[30px]' : 'max-h-44 min-h-[38px]'} ${isDark ? 'placeholder:text-zinc-500' : 'placeholder:text-slate-400'}`}
            disabled={isStreaming}
            onChange={(e) => onInputChange(e.target.value)}
            onInput={(e) => fitTextareaToContent(e.currentTarget)}
            onKeyDown={(e) => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); e.currentTarget.form?.requestSubmit(); } }}
                    placeholder={t('composer.placeholder')}
            ref={composerTextareaRef}
            rows={1}
            value={input}
          />
          <div className="mt-2 flex items-center justify-between gap-3">
            <div className="relative">
              <button
                aria-expanded={toolsOpen}
                className={`aegis-accent-ghost inline-flex items-center gap-2 rounded-lg border px-2.5 py-2 text-[11px] font-semibold uppercase tracking-[0.16em] transition-all duration-200 ${isStreaming ? 'cursor-not-allowed opacity-60' : ''} ${toolsOpen ? '-translate-y-0.5 scale-[0.98]' : 'translate-y-0 scale-100'} ${toolsOpen ? 'aegis-accent-subtle' : isDark ? 'border-transparent text-zinc-500 hover:bg-zinc-800' : 'border-transparent text-slate-500 hover:bg-stone-100'}`}
                disabled={isStreaming}
                onClick={onToggleTools}
                type="button"
              >
                <Wrench className={toolsOpen ? 'rotate-12 transition-transform' : 'transition-transform'} size={15} />
                        <span>{t('composer.tools')}</span>
                <ChevronDown className={`transition-transform duration-200 ${toolsOpen ? 'rotate-180' : 'rotate-0'}`} size={13} />
              </button>
              {toolsOpen && (
                <div className={`absolute bottom-full left-0 z-20 mb-2 w-48 animate-[toolsMenuIn_160ms_ease-out] rounded-lg border p-1 shadow-xl ${isDark ? 'border-zinc-800 bg-zinc-950 text-zinc-100' : 'border-stone-300 bg-white text-slate-900'}`}>
                  <button className={`flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm ${isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'}`} disabled={isStreaming || isUploading} onClick={onImportClick} type="button">
                    <Upload size={15} /> {t('composer.import')}
                  </button>
                  <button className={`flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm disabled:opacity-50 ${isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'}`} onClick={onCalendarOpen} type="button">
                    <Calendar size={15} /> {t('composer.calendar')}
                  </button>
                  <button className={`flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm disabled:opacity-50 ${isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'}`} disabled={false} onClick={onExportPdf} type="button">
                    <Download size={15} /> {t('composer.export')}
                  </button>
                </div>
              )}
            </div>
            <div className="flex items-center gap-3">
              <span className={`font-mono text-[11px] ${isDark ? 'text-zinc-600' : 'text-slate-400'}`} title={`${contextUsage.model || 'Active model'} context usage from the last completed inference`}>
                {tokenMeterLabel}
              </span>
              <button
                className={`inline-flex h-9 w-9 items-center justify-center rounded-lg transition-all duration-200 ${isVoiceMode ? 'aegis-accent-chip-active text-white' : isDark ? 'text-zinc-500 hover:bg-zinc-800 hover:text-emerald-400' : 'text-slate-500 hover:bg-stone-100 hover:text-emerald-600'}`}
                onClick={onVoiceModeOpen}
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
                    <span>{t('composer.send')}</span>
                    <Send size={15} />
              </button>
            </div>
          </div>
        </div>
      </form>
    </footer>
  );
}
