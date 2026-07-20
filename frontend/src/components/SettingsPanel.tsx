// Settings panel with tabs: General, Inference, Models, Voice, RAG, Memories
import { useState, useEffect } from 'react';
import { Settings, X, Download, Pause, Play, Plus, Eye, Sun, Moon, Monitor, BookOpen, Check, AlertCircle, Loader, TerminalSquare, ShieldCheck } from 'lucide-react';
import type { SettingsTab, ThemeMode, ModelResponse, ProviderResponse, CatalogModel, ModelDownloadState, CommandLineSettings } from '../types';
import { useT, type Language } from '../lib/i18n';
import { API_BASE } from '../constants';
import {
  OLLAMA_MODEL_CATALOG, MODEL_PROVIDER_TAGS, RESPONSE_STYLE_OPTIONS, APPEARANCE_THEME_OPTIONS,
} from '../constants';
import { modelSearchPlaceholder, installedModelsLabel } from '../lib/modelDownload';
import { useDialogA11y } from '../hooks/useDialogA11y';

interface SettingsPanelProps {
  isDark: boolean;
  settingsOpen: boolean;
  settingsClosing: boolean;
  settingsTab: SettingsTab;
  settingsLoading: boolean;
  theme: ThemeMode;
  appearanceTheme: string;
  responseStyle: string;
  availableModels: ModelResponse[];
  availableProviders: ProviderResponse[];
  activeProvider: ProviderResponse | undefined;
  modelSearch: string;
  selectedModelProviderTag: string;
  filteredCatalogModels: CatalogModel[];
  downloadingModel: string | null;
  pausedModelDownload: string | null;
  modelDownloadState: ModelDownloadState;
  modelDownloadProgress: number;
  modelDownloadStatus: string;
  modelSwitching: boolean;
  providerSwitching: boolean;
  commandLineSettings: CommandLineSettings;
  commandLineSaving: boolean;
  isVoiceLowRamMode: boolean;
  isTtsEnabled: boolean;
  isRagEnabled: boolean;
  ragTopK: number;
  ragSimilarityThreshold: number;
  profileText: string;
  profilePath: string;
  memoryInput: string;
  onClose: () => void;
  onSetSettingsTab: (tab: SettingsTab) => void;
  onSetTheme: (theme: ThemeMode) => void;
  onSetAppearanceTheme: (theme: string) => void;
  onSetResponseStyle: (style: string) => void;
  onSelectModel: (name: string) => void;
  onSelectProvider: (name: string) => void;
  onChangeCommandLineSettings: (settings: CommandLineSettings) => void;
  onModelSearchChange: (value: string) => void;
  onSetModelProviderTag: (tag: string) => void;
  onDownloadModel: (name?: string) => void;
  onPauseDownload: () => void;
  onCancelDownload: () => void;
  onResumeDownload: () => void;
  onToggleVoiceLowRam: (enabled: boolean) => void;
  onToggleTts: (enabled: boolean) => void;
  onToggleRag: (enabled: boolean) => void;
  onChangeRagTopK: (val: number) => void;
  onChangeRagThreshold: (val: number) => void;
  onMemoryInputChange: (value: string) => void;
  onAddMemory: () => void;
  onDisplayMemories: () => void;
  onSaveProfile: () => void;
  onProfileTextChange: (value: string) => void;
  lang: Language;
  onSetLanguage: (lang: Language) => void;
  obsidianVaultPath: string;
  onObsidianVaultPathChange: (value: string) => void;
  obsidianEnabled: boolean;
  onObsidianEnabledChange: (value: boolean) => void;
}

type CommandLineCapability = Exclude<keyof CommandLineSettings, 'git_safety'>;

const COMMAND_LINE_CAPABILITIES: Array<{
  key: CommandLineCapability;
  label: string;
  description: string;
}> = [
  { key: 'agentic_loop', label: 'Agentic coding loop', description: 'Run Understand, Explore, Plan, Permission, Edit, Format, Test, and Review as a controlled task cycle.' },
  { key: 'repository_detection', label: 'Repository detection', description: 'Detect the Git root, languages, frameworks, package managers, branch, and existing changes.' },
  { key: 'repository_instructions', label: 'Repository instructions', description: 'Read AGENTS.md, CONTRIBUTING.md, CLAUDE.md, and .aegis.md before planning changes.' },
  { key: 'semantic_index', label: 'Repository semantic index', description: 'Cache files, symbols, docs, configuration, tests, and recent Git history for natural-language navigation.' },
  { key: 'context_budgeting', label: 'Context budgeting', description: 'Rank indexed files by relevance and compact context instead of repeatedly loading the entire repository.' },
  { key: 'persistent_task_plan', label: 'Persistent task plan', description: 'Save live task stages and resume the same task plan after a CLI or engine restart.' },
  { key: 'task_checkpoints', label: 'Reversible checkpoints', description: 'Snapshot touched files before edits and permit guarded restoration with aegis code restore.' },
  { key: 'patch_application', label: 'File editing', description: 'Allow validated workspace-local patches. When disabled, coding tasks are forced into read-only mode.' },
  { key: 'command_execution', label: 'Command execution', description: 'Allow explicitly approved formatting, build, and test commands inside the repository.' },
  { key: 'automatic_verification', label: 'Automatic verification', description: 'Infer and run affected checks after edits when the selected permission mode allows it.' },
  { key: 'deep_reasoning', label: 'Deep reasoning by default', description: 'Enable the higher-overhead reasoning path for CLI coding tasks unless overridden by the command.' },
];

export function SettingsPanel({
  isDark, settingsOpen, settingsClosing, settingsTab, settingsLoading,
  theme, appearanceTheme, responseStyle, availableModels, availableProviders, activeProvider,
  modelSearch, selectedModelProviderTag, filteredCatalogModels,
  downloadingModel, pausedModelDownload, modelDownloadState, modelDownloadProgress, modelDownloadStatus, modelSwitching, providerSwitching,
  commandLineSettings, commandLineSaving,
  isVoiceLowRamMode, isTtsEnabled, isRagEnabled, ragTopK, ragSimilarityThreshold,
  profileText, profilePath, memoryInput,
  onClose, onSetSettingsTab, onSetTheme, onSetAppearanceTheme, onSetResponseStyle,
  onSelectModel, onSelectProvider, onModelSearchChange, onSetModelProviderTag,
  onChangeCommandLineSettings,
  onDownloadModel, onPauseDownload, onCancelDownload, onResumeDownload,
  onToggleVoiceLowRam, onToggleTts, onToggleRag, onChangeRagTopK, onChangeRagThreshold,
  onMemoryInputChange, onAddMemory, onDisplayMemories, onSaveProfile, onProfileTextChange,
  lang, onSetLanguage,
  obsidianVaultPath, onObsidianVaultPathChange, obsidianEnabled, onObsidianEnabledChange,
}: SettingsPanelProps) {
  const dialogRef = useDialogA11y(settingsOpen, onClose);
  const t = useT();
  const isInferenceSwitching = modelSwitching || providerSwitching;
  const changeCommandLineCapability = (key: CommandLineCapability, enabled: boolean) => {
    const next = { ...commandLineSettings, [key]: enabled };
    if (key === 'command_execution' && !enabled) next.automatic_verification = false;
    if (key === 'semantic_index' && !enabled) next.context_budgeting = false;
    onChangeCommandLineSettings(next);
  };
  if (!settingsOpen) return null;

  const tabs: SettingsTab[] = ['general', 'models', 'command-line', 'tools', 'personalize', 'voice', 'rag', 'memories'];

  return (
    <div className={`fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4 ${settingsClosing ? 'aegis-modal-backdrop-out' : 'aegis-modal-backdrop'}`} onClick={onClose}>
      <div
        aria-labelledby="settings-dialog-title"
        aria-modal="true"
        className={`aegis-settings-panel flex min-h-[340px] max-h-[calc(100dvh-2rem)] overflow-hidden rounded-2xl border shadow-2xl ${settingsClosing ? 'aegis-modal-panel-out' : 'aegis-modal-panel'} aegis-bg-surface aegis-border-subtle aegis-text-base`}
        style={{ aspectRatio: '1.22 / 1', width: 'min(88vw, calc((100dvh - 2rem) * 1.22), 74rem)' }}
        onClick={(e) => e.stopPropagation()}
        ref={dialogRef}
        role="dialog"
        tabIndex={-1}
      >
        <aside className={`w-52 shrink-0 border-r p-5 aegis-bg-base aegis-border-subtle ${isDark ? '' : 'shadow-[8px_0_26px_rgba(80,62,39,0.08)]'}`}>
          <div className="aegis-display mb-8 flex items-center gap-2 text-[13px] font-semibold tracking-[-0.01em]" id="settings-dialog-title">
            <Settings size={16} />
            {t('settings.title')}
          </div>
          {tabs.map((value) => (
            <button
              key={value}
              className={`mb-1.5 flex w-full items-center rounded-xl px-3.5 py-2.5 text-left text-[13px] font-medium transition ${settingsTab === value ? 'aegis-accent-solid text-white' : isDark ? 'text-zinc-400 hover:bg-zinc-900 hover:text-zinc-100' : 'text-stone-600 hover:bg-[rgba(94,76,55,0.1)] hover:text-stone-950'}`}
              onClick={() => onSetSettingsTab(value)}
              type="button"
            >
              {t(`settings.tab.${value}`)}
            </button>
          ))}
        </aside>

        <section className="flex min-w-0 flex-1 flex-col">
          <div className={`flex h-16 shrink-0 items-center justify-between border-b px-6 aegis-border-subtle`}>
            <div>
              <div className="aegis-display text-[15px] font-semibold capitalize tracking-[-0.015em]">{t(`settings.tab.${settingsTab}`)}</div>
              <div className={`text-[12px] font-normal ${isDark ? 'text-zinc-500' : 'text-stone-500'}`}>{settingsLoading ? t('settings.loading') : t('settings.preferences')}</div>
            </div>
            <button aria-label="Close settings" className={`rounded-md p-1 transition ${isDark ? 'hover:bg-zinc-900' : 'hover:bg-[rgba(94,76,55,0.1)]'}`} data-dialog-initial-focus onClick={onClose} type="button">
              <X size={18} />
            </button>
          </div>

          <div className="settings-scroll min-h-0 flex-1 overflow-y-auto px-6 pb-6 pt-1 aegis-bg-base">
            {settingsTab === 'general' && (
              <div className="space-y-6">
                <div>
                  <label className="mb-2 block text-sm font-semibold" htmlFor="general-model">{t('settings.general.active_model')}</label>
                  <select
                    className={`w-full rounded-lg border px-3 py-2 text-sm outline-none focus:border-emerald-600 ${isDark ? 'border-zinc-800 bg-zinc-900 text-zinc-100' : 'border-stone-300 bg-white text-slate-900'}`}
                    disabled={availableModels.length === 0 || Boolean(downloadingModel) || isInferenceSwitching}
                    id="general-model"
                    onChange={(e) => onSelectModel(e.target.value)}
                    value={availableModels.find((m) => m.active)?.name ?? ''}
                  >
                    <option value="" disabled>{availableModels.length === 0 ? 'No installed models found' : 'Choose active model'}</option>
                    {availableModels.map((m) => (<option key={m.name} value={m.name}>{m.name}</option>))}
                  </select>
                  {modelSwitching ? (
                    <div className="mt-2 flex items-center gap-2 text-xs text-emerald-600" role="status">
                      <Loader className="animate-spin" size={14} />
                      <span>Loading the selected model into memory…</span>
                    </div>
                  ) : (
                    <div className={`mt-1 text-xs ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>{t('settings.general.model_switch_hint')}</div>
                  )}
                </div>
                <div>
                  <div className="mb-2 text-sm font-semibold">{t('settings.general.language')}</div>
                  <select
                    className={`w-full rounded-lg border px-3 py-2 text-sm outline-none focus:border-emerald-600 ${isDark ? 'border-zinc-800 bg-zinc-900 text-zinc-100' : 'border-stone-300 bg-white text-slate-900'}`}
                    value={lang}
                    onChange={(e) => onSetLanguage(e.target.value as Language)}
                  >
                    <option value="en">English</option>
                    <option value="tr">Türkçe</option>
                  </select>
                  <div className={`mt-1 text-xs ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>{t('settings.general.language_hint')}</div>
                </div>
                <div>
                  <label className="mb-2 block text-sm font-semibold" htmlFor="provider-select">{t('settings.inference.provider')}</label>
                  <select
                    className={`w-full rounded-lg border px-3 py-2 text-sm outline-none focus:border-emerald-600 ${isDark ? 'border-zinc-800 bg-zinc-900 text-zinc-100' : 'border-stone-300 bg-white text-slate-900'}`}
                    disabled={availableProviders.length === 0 || isInferenceSwitching}
                    id="provider-select"
                    onChange={(e) => onSelectProvider(e.target.value)}
                    value={activeProvider?.name ?? ''}
                  >
                    <option value="" disabled>{availableProviders.length === 0 ? 'No providers available' : 'Choose provider'}</option>
                    {availableProviders.map((p) => (<option key={p.name} value={p.name}>{p.name}</option>))}
                  </select>
                  {providerSwitching && (
                    <div className="mt-2 flex items-center gap-2 text-xs text-emerald-600" role="status">
                      <Loader className="animate-spin" size={14} />
                      <span>Switching inference provider…</span>
                    </div>
                  )}
                  {activeProvider && (
                    <div className={`mt-2 rounded-xl border p-3 text-xs leading-5 ${isDark ? 'border-zinc-800 bg-zinc-900/40 text-zinc-400' : 'border-stone-300 bg-stone-50 text-slate-500'}`}>
                      {activeProvider.description}
                    </div>
                  )}
                </div>
              </div>
            )}

            {settingsTab === 'personalize' && (
              <div className="space-y-5">
                <div>
                  <div className="mb-2 text-sm font-semibold">{t('settings.personalize.appearance')}</div>
                  <div className={`inline-flex items-center gap-1 rounded-lg border p-1 ${isDark ? 'border-zinc-800 bg-zinc-900' : 'border-stone-300 bg-[rgba(239,233,222,0.86)]'}`}>
                    {(['light', 'dark', 'system'] as ThemeMode[]).map((mode) => {
                      const Icon = mode === 'light' ? Sun : mode === 'dark' ? Moon : Monitor;
                      return (
                        <button
                          key={mode}
                          className={`flex items-center justify-center rounded-md p-1.5 transition-all ${theme === mode ? (isDark ? 'bg-zinc-700 text-zinc-100 shadow-sm' : 'bg-white text-slate-900 shadow-sm') : isDark ? 'text-zinc-500 hover:text-zinc-300' : 'text-slate-400 hover:text-slate-700'}`}
                          onClick={() => onSetTheme(mode)}
                          type="button"
                          title={mode === 'light' ? 'Light mode' : mode === 'dark' ? 'Dark mode' : 'System default'}
                        >
                          <Icon size={16} />
                        </button>
                      );
                    })}
                  </div>
                  <div className={`mb-3 text-xs ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>{t('settings.personalize.appearance_hint')}</div>
                  <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
                    {APPEARANCE_THEME_OPTIONS.map((option) => (
                      <button key={option.value} className={`rounded-xl border p-3 text-left transition ${appearanceTheme === option.value ? 'aegis-accent-selected shadow-lg' : isDark ? 'border-zinc-800 hover:bg-zinc-900' : 'border-stone-300 hover:bg-stone-50'}`} onClick={() => onSetAppearanceTheme(option.value)} type="button">
                        <span className={`mb-3 block h-14 rounded-lg border ${isDark ? 'border-white/10' : 'border-black/5'}`} style={{ background: option.preview }} />
                        <div className="flex items-center justify-between gap-2">
                          <div className="text-sm font-semibold">{option.label}</div>
                          {appearanceTheme === option.value && <span className={`rounded-full px-2 py-0.5 text-[10px] uppercase tracking-[0.12em] ${isDark ? 'bg-white/10 text-zinc-100' : 'bg-black/5 text-slate-700'}`}>Active</span>}
                        </div>
                        <div className={`mt-1 text-xs leading-5 ${isDark ? 'text-zinc-400' : 'text-slate-500'}`}>{option.description}</div>
                      </button>
                    ))}
                  </div>
                </div>
                <div>
                  <div className="mb-2 text-sm font-semibold">{t('settings.personalize.response_style')}</div>
                  <div className="grid gap-2 sm:grid-cols-2">
                    {RESPONSE_STYLE_OPTIONS.map((option) => (
                      <button key={option.value} className={`rounded-xl border p-3 text-left transition ${responseStyle === option.value ? 'aegis-accent-selected' : isDark ? 'border-zinc-800 hover:bg-zinc-900' : 'border-stone-300 hover:bg-stone-50'}`} onClick={() => onSetResponseStyle(option.value)} type="button">
                        <div className="text-sm font-semibold">{option.label}</div>
                        <div className={`mt-1 text-xs leading-5 ${isDark ? 'text-zinc-400' : 'text-slate-500'}`}>{option.description}</div>
                      </button>
                    ))}
                  </div>
                </div>
              </div>
            )}

            {settingsTab === 'voice' && (
              <div className="space-y-5">
                <div>
                  <div className="mb-2 text-sm font-semibold">{t('settings.voice.caching')}</div>
                  <div className="flex flex-col gap-3">
                    <label className={`flex items-start justify-between rounded-xl border p-4 cursor-pointer transition ${isVoiceLowRamMode ? isDark ? 'border-emerald-500 bg-emerald-950/25 text-emerald-100' : 'border-emerald-500 bg-emerald-50 text-emerald-900' : isDark ? 'border-zinc-800 hover:bg-zinc-900/60' : 'border-stone-300 hover:bg-stone-50'}`}>
                      <div className="flex flex-col gap-1 pr-4">
                        <span className="text-sm font-semibold">{t('settings.voice.low_ram')}</span>
                        <span className={`text-xs leading-5 ${isDark ? 'text-zinc-400' : 'text-slate-500'}`}>{t('settings.voice.low_ram_desc')}</span>
                      </div>
                      <input type="checkbox" checked={isVoiceLowRamMode} onChange={(e) => onToggleVoiceLowRam(e.target.checked)} className="mt-1 h-4 w-4 shrink-0 rounded border-stone-300 text-emerald-600 focus:ring-emerald-500 cursor-pointer" />
                    </label>
                    <label className={`flex items-start justify-between rounded-xl border p-4 cursor-pointer transition ${isTtsEnabled ? isDark ? 'border-emerald-500 bg-emerald-950/25 text-emerald-100' : 'border-emerald-500 bg-emerald-50 text-emerald-900' : isDark ? 'border-zinc-800 hover:bg-zinc-900/60' : 'border-stone-300 hover:bg-stone-50'}`}>
                      <div className="flex flex-col gap-1 pr-4">
                        <span className="text-sm font-semibold">{t('settings.voice.read_aloud')}</span>
                        <span className={`text-xs leading-5 ${isDark ? 'text-zinc-400' : 'text-slate-500'}`}>{t('settings.voice.read_aloud_desc')}</span>
                      </div>
                      <input type="checkbox" checked={isTtsEnabled} onChange={(e) => onToggleTts(e.target.checked)} className="mt-1 h-4 w-4 shrink-0 rounded border-stone-300 text-emerald-600 focus:ring-emerald-500 cursor-pointer" />
                    </label>
                  </div>
                </div>
              </div>
            )}

            {settingsTab === 'rag' && (
              <div className="space-y-5">
                <div>
                  <div className="mb-2 text-sm font-semibold">{t('settings.rag.title')}</div>
                  <div className="flex flex-col gap-3">
                    <label className={`flex items-start justify-between rounded-xl border p-4 cursor-pointer transition ${isRagEnabled ? isDark ? 'border-emerald-500 bg-emerald-950/25 text-emerald-100' : 'border-emerald-500 bg-emerald-50 text-emerald-900' : isDark ? 'border-zinc-800 hover:bg-zinc-900/60' : 'border-stone-300 hover:bg-stone-50'}`}>
                      <div className="flex flex-col gap-1 pr-4">
                        <span className="text-sm font-semibold">Enable Retrieval-Augmented Generation</span>
                        <span className={`text-xs leading-5 ${isDark ? 'text-zinc-400' : 'text-slate-500'}`}>Inject relevant document excerpts from imported files into the LLM context to answer your questions. If disabled, the model will not read from your document library during chat conversations.</span>
                      </div>
                      <input type="checkbox" checked={isRagEnabled} onChange={(e) => onToggleRag(e.target.checked)} className="mt-1 h-4 w-4 shrink-0 rounded border-stone-300 text-emerald-600 focus:ring-emerald-500 cursor-pointer" />
                    </label>
                    {isRagEnabled && (
                      <>
                        <div className={`rounded-xl border p-4 ${isDark ? 'border-zinc-800' : 'border-stone-200'}`}>
                          <div className="mb-1 flex items-center justify-between">
                            <span className="text-sm font-semibold">Retrieve Limit (Top-K)</span>
                            <span className="text-sm font-bold text-emerald-600">{ragTopK} chunks</span>
                          </div>
                          <span className={`block mb-3 text-xs leading-5 ${isDark ? 'text-zinc-400' : 'text-slate-500'}`}>The maximum number of document passages to retrieve and supply to the AI model per message. Higher values provide more context but consume more memory and tokens.</span>
                          <input type="range" min="1" max="10" step="1" value={ragTopK} onChange={(e) => onChangeRagTopK(Number(e.target.value))} className="h-2 w-full cursor-pointer appearance-none rounded-lg bg-stone-200 dark:bg-zinc-800 accent-emerald-600" />
                        </div>
                        <div className={`rounded-xl border p-4 ${isDark ? 'border-zinc-800' : 'border-stone-200'}`}>
                          <div className="mb-1 flex items-center justify-between">
                            <span className="text-sm font-semibold">Similarity Cutoff Score</span>
                            <span className="text-sm font-bold text-emerald-600">{ragSimilarityThreshold === 0.0 ? 'None (Retrieve all)' : `≥ ${ragSimilarityThreshold.toFixed(2)}`}</span>
                          </div>
                          <span className={`block mb-3 text-xs leading-5 ${isDark ? 'text-zinc-400' : 'text-slate-500'}`}>Only inject retrieved passages whose similarity scores exceed this cutoff. Helps filter out irrelevant text noise. A setting of 0.0 disables cutoff filtering.</span>
                          <input type="range" min="0.0" max="0.9" step="0.05" value={ragSimilarityThreshold} onChange={(e) => onChangeRagThreshold(Number(e.target.value))} className="h-2 w-full cursor-pointer appearance-none rounded-lg bg-stone-200 dark:bg-zinc-800 accent-emerald-600" />
                        </div>
                      </>
                    )}
                  </div>
                </div>
              </div>
            )}

            {settingsTab === 'models' && (
              <div className="space-y-4">
                <div>
                  <label className="mb-2 block text-sm font-semibold" htmlFor="model-search">Search or Download Model</label>
                  <div className="flex gap-2">
                    <input
                      className={`min-w-0 flex-1 rounded-lg border px-3 py-2 text-sm outline-none focus:border-emerald-600 ${isDark ? 'border-zinc-800 bg-zinc-900 text-zinc-100 placeholder:text-zinc-500' : 'border-stone-300 bg-white text-slate-900 placeholder:text-slate-400'}`}
                      id="model-search"
                      onChange={(e) => onModelSearchChange(e.target.value)}
                      placeholder={modelSearchPlaceholder(activeProvider?.name)}
                      value={modelSearch}
                    />
                    <button
                      className="rounded-lg bg-emerald-600 px-4 py-2 text-sm font-medium text-white transition hover:bg-emerald-500 disabled:opacity-60"
                      disabled={!modelSearch.trim() || modelDownloadState === 'downloading'}
                      onClick={() => onDownloadModel()}
                      type="button"
                    >
                      Download
                    </button>
                  </div>
                </div>
                <div className="space-y-2">
                  <div className="flex flex-wrap gap-2">
                    {MODEL_PROVIDER_TAGS.map((tag) => (
                      <button key={tag} className={`rounded-full border px-3 py-1.5 text-xs transition ${selectedModelProviderTag === tag ? 'border-emerald-500 bg-emerald-600 text-white' : isDark ? 'border-zinc-800 text-zinc-400 hover:bg-zinc-900 hover:text-zinc-100' : 'border-stone-300 text-slate-600 hover:bg-stone-100 hover:text-slate-950'}`} onClick={() => onSetModelProviderTag(tag)} type="button">
                        {tag}
                      </button>
                    ))}
                  </div>
                  <div className={`settings-scroll max-h-56 space-y-2 overflow-y-auto rounded-xl border p-2 ${isDark ? 'border-zinc-800 bg-zinc-950/40' : 'border-stone-300 bg-stone-50'}`}>
                    {filteredCatalogModels.length === 0 ? (
                      activeProvider?.name?.toLowerCase() === 'lmstudio' ? (
                        <div className={`p-4 text-sm ${isDark ? 'text-zinc-400' : 'text-slate-600'}`}>
                          <p className="mb-2">Paste a HuggingFace model URL to download via LM Studio:</p>
                          <div className="flex gap-2">
                            <input className={`flex-1 rounded-lg border px-3 py-2 text-sm outline-none focus:border-emerald-600 ${isDark ? 'border-zinc-800 bg-zinc-900 text-zinc-100' : 'border-stone-300 bg-white text-slate-900'}`}
                              placeholder="https://huggingface.co/lmstudio-community/mistral-7b-instruct-v0.3-gguf"
                              value={modelSearch}
                              onChange={(e) => onModelSearchChange(e.target.value)}
                              onKeyDown={(e) => { if (e.key === 'Enter' && modelSearch.trim()) onDownloadModel(modelSearch.trim()); }}
                            />
                            <button className={`aegis-accent-ghost inline-flex shrink-0 items-center justify-center rounded-md border p-2 transition ${modelDownloadState === 'downloading' ? 'cursor-not-allowed opacity-45' : isDark ? 'text-zinc-400' : 'text-slate-500'}`} disabled={modelDownloadState === 'downloading'} onClick={() => modelSearch.trim() && onDownloadModel(modelSearch.trim())} type="button"><Download size={15} /></button>
                          </div>
                        </div>
                      ) : (
                        <div className={`p-3 text-sm ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>No catalog models match this filter.</div>
                      )
                    ) : (
                      filteredCatalogModels.map((model) => (
                        <div key={model.name} className={`flex w-full items-start justify-between gap-3 rounded-lg p-3 text-left transition ${modelSearch.trim() === model.name ? isDark ? 'bg-emerald-950/30 text-emerald-100' : 'bg-emerald-50 text-emerald-900' : isDark ? 'hover:bg-zinc-900' : 'hover:bg-white'}`}>
                          <span className="min-w-0">
                            <span className="block truncate font-mono text-sm">{model.name}</span>
                            <span className={`mt-1 block text-xs leading-5 ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>{model.description}</span>
                            <span className="mt-2 flex flex-wrap gap-1.5">
                              {[model.provider, ...model.tags].map((tag) => (
                                <span key={`${model.name}-${tag}`} className={`rounded-full px-2 py-0.5 text-[10px] ${isDark ? 'bg-zinc-800 text-zinc-400' : 'bg-stone-200 text-slate-600'}`}>{tag}</span>
                              ))}
                              <span className={`rounded-full px-2 py-0.5 text-[10px] ${isDark ? 'bg-zinc-700 text-emerald-400' : 'bg-stone-200 text-emerald-700'}`}>{model.source === 'ollama' ? 'Ollama' : 'HuggingFace'}</span>
                            </span>
                          </span>
                          <button aria-label={`Download ${model.name}`} className={`aegis-accent-ghost mt-0.5 inline-flex shrink-0 items-center justify-center rounded-md border border-transparent p-2 transition ${modelDownloadState === 'downloading' ? 'cursor-not-allowed opacity-45' : isDark ? 'text-zinc-400' : 'text-slate-500'}`} disabled={modelDownloadState === 'downloading'} onClick={() => onDownloadModel(model.name)} type="button">
                            <Download size={15} />
                          </button>
                        </div>
                      ))
                    )}
                  </div>
                </div>

                {(downloadingModel || pausedModelDownload) && (
                  <div className={`rounded-xl border p-3 ${isDark ? 'border-zinc-800 bg-zinc-900/50' : 'border-stone-300 bg-stone-50'}`}>
                    <div className="mb-2 flex items-center justify-between text-xs">
                      <span className="truncate">{downloadingModel ?? pausedModelDownload}: {modelDownloadStatus}</span>
                      <span className="font-mono">{modelDownloadProgress}%</span>
                    </div>
                    <div className={`h-1.5 rounded-full ${isDark ? 'bg-zinc-800' : 'bg-stone-200'}`}>
                      <div className="h-full rounded-full bg-emerald-500 transition-all duration-300" style={{ width: `${modelDownloadProgress}%` }} />
                    </div>
                    <div className="mt-3 flex justify-end gap-2">
                      {modelDownloadState === 'downloading' ? (
                        <button className={`inline-flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-xs transition ${isDark ? 'border-zinc-800 text-zinc-300 hover:bg-zinc-900' : 'border-stone-300 text-slate-700 hover:bg-stone-100'}`} onClick={onPauseDownload} type="button">
                          <Pause size={13} /> Pause
                        </button>
                      ) : (
                        <button className="inline-flex items-center gap-1.5 rounded-lg bg-emerald-600 px-3 py-1.5 text-xs text-white transition hover:bg-emerald-500" onClick={onResumeDownload} type="button">
                          <Play size={13} /> Resume
                        </button>
                      )}
                      <button className={`inline-flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-xs transition ${isDark ? 'border-red-900/70 text-red-300 hover:bg-red-950/30' : 'border-red-200 text-red-700 hover:bg-red-50'}`} onClick={onCancelDownload} type="button">
                        <X size={13} /> Cancel
                      </button>
                    </div>
                  </div>
                )}

                <div>
                  <label className="mb-2 block text-sm font-semibold" htmlFor="installed-model-select">{installedModelsLabel(activeProvider?.name)}</label>
                  <select
                    className={`w-full rounded-lg border px-3 py-2 text-sm outline-none focus:border-emerald-600 ${isDark ? 'border-zinc-800 bg-zinc-900 text-zinc-100' : 'border-stone-300 bg-white text-slate-900'}`}
                    disabled={availableModels.length === 0 || modelDownloadState === 'downloading' || isInferenceSwitching}
                    id="installed-model-select"
                    onChange={(e) => onSelectModel(e.target.value)}
                    value={availableModels.find((m) => m.active)?.name ?? ''}
                  >
                    <option value="" disabled>{availableModels.length === 0 ? 'No installed models found' : 'Choose installed model'}</option>
                    {availableModels.map((m) => (<option key={m.name} value={m.name}>{m.active ? `${m.name} (active)` : m.name}</option>))}
                  </select>
                  {modelSwitching ? (
                    <div className="mt-2 flex items-center gap-2 text-xs text-emerald-600" role="status">
                      <Loader className="animate-spin" size={14} />
                      <span>Warming selected model…</span>
                    </div>
                  ) : (
                    <div className={`mt-1 text-xs ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>Selecting an installed model warms it before making it active.</div>
                  )}
                  {availableModels.length === 0 ? (
                    <div className={`mt-3 rounded-xl border p-3 text-xs leading-5 ${isDark ? 'border-zinc-800 bg-zinc-900/40 text-zinc-400' : 'border-stone-200 bg-stone-50 text-slate-600'}`}>
                      <span className="block font-semibold">No models available for this provider</span>
                      <span>Download a model above, or switch providers if the model is managed outside AEGIS.</span>
                    </div>
                  ) : (
                    <div className="mt-3 flex flex-wrap gap-2 text-[11px]">
                      <span className={`rounded-full px-2 py-1 ${isDark ? 'bg-zinc-800 text-zinc-300' : 'bg-stone-200 text-stone-700'}`}>{availableModels.some((model) => model.status === 'degraded') ? 'Catalog unavailable' : `${availableModels.length} installed`}</span>
                      <span className={`rounded-full px-2 py-1 ${availableModels.some((model) => model.status === 'degraded') ? 'bg-amber-500/15 text-amber-700' : 'bg-emerald-500/15 text-emerald-600'}`}>{availableModels.some((model) => model.status === 'degraded') ? 'Provider unavailable' : availableModels.some((model) => model.status === 'ready' || model.active) ? 'Active model ready' : 'Select to warm'}</span>
                      <span className={`rounded-full px-2 py-1 ${isDark ? 'bg-zinc-800 text-zinc-400' : 'bg-stone-200 text-stone-600'}`}>{availableModels[0]?.supports_managed_download === false ? 'Downloads managed by provider' : 'Managed downloads supported'}</span>
                    </div>
                  )}
                </div>
              </div>
            )}

            {settingsTab === 'command-line' && (
              <div className="space-y-4 pt-4">
                <div className={`rounded-2xl border p-4 ${isDark ? 'border-emerald-900/60 bg-emerald-950/20' : 'border-emerald-200 bg-emerald-50/70'}`}>
                  <div className="flex items-start gap-3">
                    <TerminalSquare className="mt-0.5 text-emerald-500" size={19} />
                    <div>
                      <div className="text-sm font-semibold">Command Line capabilities</div>
                      <p className={`mt-1 text-xs leading-5 ${isDark ? 'text-zinc-400' : 'text-slate-600'}`}>
                        These controls are enforced by the CLI before every coding task. Changes apply to new commands immediately.
                      </p>
                    </div>
                    {commandLineSaving && <Loader className="ml-auto animate-spin text-emerald-500" size={17} />}
                  </div>
                </div>

                <div className="grid gap-2.5">
                  {COMMAND_LINE_CAPABILITIES.map((capability) => (
                    <CommandLineToggle
                      key={capability.key}
                      checked={commandLineSettings[capability.key]}
                      description={capability.description}
                      disabled={commandLineSaving
                        || (capability.key === 'automatic_verification' && !commandLineSettings.command_execution)
                        || (capability.key === 'context_budgeting' && !commandLineSettings.semantic_index)}
                      isDark={isDark}
                      label={capability.label}
                      onChange={(enabled) => changeCommandLineCapability(capability.key, enabled)}
                    />
                  ))}
                </div>

                <div className={`flex items-start gap-3 rounded-xl border p-3.5 ${isDark ? 'border-zinc-800 bg-zinc-900/50' : 'border-stone-200 bg-stone-50'}`}>
                  <ShieldCheck className="mt-0.5 shrink-0 text-emerald-500" size={17} />
                  <div>
                    <div className="text-xs font-semibold">Git safety is always enforced</div>
                    <p className={`mt-1 text-xs leading-5 ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>
                      AEGIS never writes outside the repository, never edits .git, and refuses unsafe overlap with unrelated user changes.
                    </p>
                  </div>
                </div>
              </div>
            )}

            {/* Tools Tab */}
            {settingsTab === 'tools' && (
              <div className="space-y-5">
                <div>
                  <div className="mb-2 text-sm font-semibold">Obsidian</div>

                  {/* Toggle */}
                  <div className="flex items-center justify-between mb-3">
                    <span className={`text-sm ${isDark ? 'text-zinc-300' : 'text-slate-700'}`}>Add Obsidian to tools</span>
                    <button
                      className={`relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 focus:outline-none ${obsidianEnabled ? 'bg-emerald-500' : isDark ? 'bg-zinc-700' : 'bg-stone-300'}`}
                      onClick={() => onObsidianEnabledChange(!obsidianEnabled)}
                      type="button"
                      role="switch"
                      aria-checked={obsidianEnabled}
                    >
                      <span className={`pointer-events-none inline-block h-4 w-4 translate-y-0 transform rounded-full bg-white shadow transition-transform duration-200 ${obsidianEnabled ? 'translate-x-4' : 'translate-x-0'}`} />
                    </button>
                  </div>

                  <label className="mb-1 block text-xs font-semibold uppercase tracking-wide opacity-70" htmlFor="obsidian-vault-path">Vault Path</label>
                  <div className="flex items-center gap-2">
                    <input
                      id="obsidian-vault-path"
                      className={`flex-1 rounded-lg border px-3 py-2 text-sm outline-none focus:border-emerald-600 ${isDark ? 'border-zinc-800 bg-zinc-900 text-zinc-100 placeholder:text-zinc-500' : 'border-stone-300 bg-white text-slate-900 placeholder:text-slate-400'}`}
                      value={obsidianVaultPath}
                      onChange={(e) => onObsidianVaultPathChange(e.target.value)}
                      placeholder="C:\Users\YourName\Documents\MyVault"
                      disabled={!obsidianEnabled}
                    />
                    <ObsidianPathStatus path={obsidianEnabled ? obsidianVaultPath : ''} isDark={isDark} />
                  </div>
                </div>
              </div>
            )}

            {/* Memories Tab */}
            {settingsTab === 'memories' && (
              <div className="space-y-4">
                <div>
                  <div className="text-sm font-semibold">{t('settings.memories.title')}</div>
                  <div className={`mt-2 text-xs leading-5 ${isDark ? 'text-zinc-400' : 'text-slate-600'}`}>
                    {t('settings.memories.desc')}
                  </div>
                </div>

                <div className="flex gap-2">
                  <input
                    className={`min-w-0 flex-1 rounded-lg border px-3 py-2 text-sm outline-none focus:border-emerald-600 ${isDark ? 'border-zinc-800 bg-zinc-900 text-zinc-100 placeholder:text-zinc-500' : 'border-stone-300 bg-white text-slate-900 placeholder:text-slate-400'}`}
                    onChange={(e) => onMemoryInputChange(e.target.value)}
                    placeholder={t('settings.memories.placeholder')}
                    value={memoryInput}
                    onKeyDown={(e) => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); onAddMemory(); } }}
                  />
                  <button
                    className="rounded-lg bg-emerald-600 px-4 py-2 text-sm font-medium text-white transition hover:bg-emerald-500 disabled:opacity-60"
                    disabled={!memoryInput.trim()}
                    onClick={onAddMemory}
                    type="button"
                  >
                    <Plus size={16} className="inline-block mr-1" />
                    {t('settings.memories.add')}
                  </button>
                </div>

                <div className={`rounded-xl border p-4 ${isDark ? 'border-zinc-800 bg-zinc-900/40' : 'border-stone-200 bg-stone-50'}`}>
                  <div className="mb-2 flex items-center justify-between">
                    <span className="text-sm font-semibold">{t('settings.memories.saved_profile')}</span>
                    <span className={`text-[11px] ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>
                      {profilePath || 'Path will appear after first save'}
                    </span>
                  </div>
                  <textarea
                    className={`min-h-36 w-full resize-none rounded-lg border p-3 text-sm leading-6 outline-none focus:border-emerald-600 ${isDark ? 'border-zinc-800 bg-zinc-950 text-zinc-100 placeholder:text-zinc-500' : 'border-stone-300 bg-white text-slate-900 placeholder:text-slate-400'}`}
                    value={profileText || ''}
                    onChange={(e) => onProfileTextChange(e.target.value)}
                    placeholder={t('settings.memories.empty')}
                  />
                </div>

                <div className="flex justify-end gap-2">
                  <button
                    className={`inline-flex items-center gap-2 rounded-lg border px-4 py-2 text-sm transition ${isDark ? 'border-zinc-800 text-zinc-300 hover:bg-zinc-900' : 'border-stone-300 text-slate-700 hover:bg-stone-100'}`}
                    onClick={onDisplayMemories}
                    type="button"
                  >
                    <Eye size={15} />
                    {t('settings.memories.display')}
                  </button>
                  <button
                    className="rounded-lg bg-emerald-600 px-4 py-2 text-sm font-medium text-white transition hover:bg-emerald-500 disabled:opacity-60"
                    disabled={!profileText.trim()}
                    onClick={onSaveProfile}
                    type="button"
                  >
                    {t('settings.memories.save')}
                  </button>
                </div>
              </div>
            )}

          </div>
        </section>
      </div>
    </div>
  );
}

function CommandLineToggle({
  checked,
  description,
  disabled,
  isDark,
  label,
  onChange,
}: {
  checked: boolean;
  description: string;
  disabled: boolean;
  isDark: boolean;
  label: string;
  onChange: (enabled: boolean) => void;
}) {
  return (
    <div className={`flex items-center gap-4 rounded-xl border px-4 py-3.5 transition ${isDark ? 'border-zinc-800 bg-zinc-900/35' : 'border-stone-200 bg-white'} ${disabled ? 'opacity-55' : ''}`}>
      <div className="min-w-0 flex-1">
        <div className="text-[13px] font-semibold">{label}</div>
        <div className={`mt-1 text-xs leading-5 ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>{description}</div>
      </div>
      <button
        aria-checked={checked}
        aria-label={`${checked ? 'Disable' : 'Enable'} ${label}`}
        className={`relative h-6 w-11 shrink-0 rounded-full transition-colors ${checked ? 'bg-emerald-500' : isDark ? 'bg-zinc-700' : 'bg-stone-300'} disabled:cursor-not-allowed`}
        disabled={disabled}
        onClick={() => onChange(!checked)}
        role="switch"
        type="button"
      >
        <span className={`absolute left-1 top-1 h-4 w-4 rounded-full bg-white shadow-sm transition-transform ${checked ? 'translate-x-5' : 'translate-x-0'}`} />
      </button>
    </div>
  );
}

function ObsidianPathStatus({ path, isDark }: { path: string; isDark: boolean }) {
  const [state, setState] = useState<'idle' | 'checking' | 'valid' | 'invalid'>('idle');
  const [message, setMessage] = useState('');

  useEffect(() => {
    if (!path.trim()) {
      setState('idle');
      setMessage('');
      return;
    }
    setState('checking');
    setMessage('');
    const controller = new AbortController();
    fetch(`${API_BASE}/mcp/obsidian/validate?path=${encodeURIComponent(path.trim())}`, { signal: controller.signal })
      .then((r) => r.json())
      .then((data) => {
        setState(data.valid ? 'valid' : 'invalid');
        setMessage(data.message || '');
      })
      .catch(() => {
        setState('idle');
        setMessage('');
      });
    return () => controller.abort();
  }, [path]);

  if (state === 'idle' || !path.trim()) return null;

  if (state === 'checking') {
    return <Loader size={18} className="shrink-0 animate-spin text-zinc-400" />;
  }

  if (state === 'valid') {
    return (
      <span className="shrink-0" title={message}>
        <Check size={18} className="text-emerald-500" />
      </span>
    );
  }

  return (
    <span className="group relative shrink-0" title={message}>
      <AlertCircle size={18} className="text-red-500" />
      <span className={`absolute left-1/2 -translate-x-1/2 top-full mt-1 w-64 rounded-lg border p-2 text-xs opacity-0 transition-opacity group-hover:opacity-100 pointer-events-none z-10 ${isDark ? 'border-red-900 bg-red-950 text-red-200' : 'border-red-200 bg-red-50 text-red-700'}`}>
        {message}
      </span>
    </span>
  );
}
