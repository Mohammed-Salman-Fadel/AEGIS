// Full settings panel with tabs: General, Inference, Models, Voice, RAG, Personalize
import { Settings, X, Download, Pause, Play } from 'lucide-react';
import type { SettingsTab, ThemeMode, ModelResponse, ProviderResponse, CatalogModel, ModelDownloadState } from '../types';
import {
  OLLAMA_MODEL_CATALOG, MODEL_PROVIDER_TAGS, RESPONSE_STYLE_OPTIONS, APPEARANCE_THEME_OPTIONS,
} from '../constants';
import { modelSearchPlaceholder, installedModelsLabel } from '../lib/modelDownload';

interface SettingsPanelProps {
  isDark: boolean;
  settingsOpen: boolean;
  settingsClosing: boolean;
  settingsTab: SettingsTab;
  settingsMessage: string | null;
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
  isVoiceLowRamMode: boolean;
  isTtsEnabled: boolean;
  isRagEnabled: boolean;
  ragTopK: number;
  ragSimilarityThreshold: number;
  profileText: string;
  profilePath: string;
  profileImportInputRef: React.RefObject<HTMLInputElement | null>;
  onClose: () => void;
  onSetSettingsTab: (tab: SettingsTab) => void;
  onSetTheme: (theme: ThemeMode) => void;
  onSetAppearanceTheme: (theme: string) => void;
  onSetResponseStyle: (style: string) => void;
  onSelectModel: (name: string) => void;
  onSelectProvider: (name: string) => void;
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
  onProfileTextChange: (value: string) => void;
  onSaveProfile: () => void;
  onImportProfile: (e: React.ChangeEvent<HTMLInputElement>) => void;
}

export function SettingsPanel({
  isDark, settingsOpen, settingsClosing, settingsTab, settingsMessage, settingsLoading,
  theme, appearanceTheme, responseStyle, availableModels, availableProviders, activeProvider,
  modelSearch, selectedModelProviderTag, filteredCatalogModels,
  downloadingModel, pausedModelDownload, modelDownloadState, modelDownloadProgress, modelDownloadStatus,
  isVoiceLowRamMode, isTtsEnabled, isRagEnabled, ragTopK, ragSimilarityThreshold,
  profileText, profilePath, profileImportInputRef,
  onClose, onSetSettingsTab, onSetTheme, onSetAppearanceTheme, onSetResponseStyle,
  onSelectModel, onSelectProvider, onModelSearchChange, onSetModelProviderTag,
  onDownloadModel, onPauseDownload, onCancelDownload, onResumeDownload,
  onToggleVoiceLowRam, onToggleTts, onToggleRag, onChangeRagTopK, onChangeRagThreshold,
  onProfileTextChange, onSaveProfile, onImportProfile,
}: SettingsPanelProps) {
  if (!settingsOpen) return null;

  return (
    <div className={`fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4 ${settingsClosing ? 'aegis-modal-backdrop-out' : 'aegis-modal-backdrop'}`} onClick={onClose}>
      <div className={`flex h-[64vh] min-h-[420px] w-full max-w-4xl overflow-hidden rounded-2xl border shadow-2xl ${settingsClosing ? 'aegis-modal-panel-out' : 'aegis-modal-panel'} ${isDark ? 'border-zinc-800 bg-zinc-950 text-zinc-100' : 'border-stone-300 bg-white text-slate-900'}`} onClick={(e) => e.stopPropagation()}>
        <aside className={`w-48 shrink-0 border-r p-4 ${isDark ? 'border-zinc-800 bg-zinc-950' : 'border-stone-200 bg-stone-50'}`}>
          <div className="mb-4 flex items-center gap-2 text-sm font-semibold">
            <Settings size={16} />
            Settings
          </div>
          {(['general', 'inference', 'models', 'voice', 'rag', 'personalize'] as SettingsTab[]).map((value) => (
            <button
              key={value}
              className={`mb-1 flex w-full items-center rounded-lg px-3 py-2 text-left text-sm transition ${settingsTab === value ? 'aegis-accent-solid text-white' : isDark ? 'text-zinc-400 hover:bg-zinc-900 hover:text-zinc-100' : 'text-slate-600 hover:bg-stone-200 hover:text-slate-950'}`}
              onClick={() => onSetSettingsTab(value)}
              type="button"
            >
              {value.charAt(0).toUpperCase() + value.slice(1)}
            </button>
          ))}
        </aside>

        <section className="flex min-w-0 flex-1 flex-col">
          <div className={`flex h-14 shrink-0 items-center justify-between px-5 ${isDark ? 'border-zinc-800' : 'border-stone-200'}`}>
            <div>
              <div className="text-sm font-semibold capitalize">{settingsTab}</div>
              <div className={`text-xs ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>{settingsLoading ? 'Loading settings...' : 'Local AEGIS preferences'}</div>
            </div>
            <button aria-label="Close settings" className={`rounded-md p-1 transition ${isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'}`} onClick={onClose} type="button">
              <X size={18} />
            </button>
          </div>

          {settingsMessage && (
            <div className={`mx-5 mb-2 rounded-lg border px-3 py-2 text-xs ${settingsMessage.toLowerCase().includes('could not') || settingsMessage.toLowerCase().includes('failed') || settingsMessage.toLowerCase().includes('only')
              ? isDark ? 'border-red-900/60 bg-red-950/30 text-red-200' : 'border-red-200 bg-red-50 text-red-700'
              : isDark ? 'border-emerald-900/60 bg-emerald-950/20 text-emerald-200' : 'border-emerald-200 bg-emerald-50 text-emerald-800'}`}>
              {settingsMessage}
            </div>
          )}

          <div className="settings-scroll min-h-0 flex-1 overflow-y-auto px-5 pb-5">
            {/* General Tab */}
            {settingsTab === 'general' && (
              <div className="space-y-5">
                <div>
                  <label className="mb-2 block text-sm font-semibold" htmlFor="general-model">Active Model</label>
                  <select
                    className={`w-full rounded-lg border px-3 py-2 text-sm outline-none focus:border-emerald-600 ${isDark ? 'border-zinc-800 bg-zinc-900 text-zinc-100' : 'border-stone-300 bg-white text-slate-900'}`}
                    disabled={availableModels.length === 0 || Boolean(downloadingModel)}
                    id="general-model"
                    onChange={(e) => onSelectModel(e.target.value)}
                    value={availableModels.find((m) => m.active)?.name ?? ''}
                  >
                    <option value="" disabled>{availableModels.length === 0 ? 'No installed models found' : 'Choose active model'}</option>
                    {availableModels.map((m) => (<option key={m.name} value={m.name}>{m.name}</option>))}
                  </select>
                  <div className={`mt-1 text-xs ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>Switching warms the selected model before the engine commits to it.</div>
                </div>
                <div>
                  <div className="mb-2 text-sm font-semibold">Appearance</div>
                  <div className="mb-3 flex flex-wrap gap-2">
                    {(['dark', 'light'] as ThemeMode[]).map((mode) => (
                      <button key={mode} className={`rounded-lg border px-3 py-2 text-sm transition ${theme === mode ? 'aegis-accent-selected' : isDark ? 'border-zinc-800 text-zinc-300 hover:bg-zinc-900' : 'border-stone-300 text-slate-700 hover:bg-stone-100'}`} onClick={() => onSetTheme(mode)} type="button">
                        {mode === 'dark' ? 'Dark mode' : 'Light mode'}
                      </button>
                    ))}
                  </div>
                  <div className={`mb-3 text-xs ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>Pick a base mode and a color profile for the overall interface.</div>
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
                  <div className="mb-2 text-sm font-semibold">Response Style</div>
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

            {/* Voice Tab */}
            {settingsTab === 'voice' && (
              <div className="space-y-5">
                <div>
                  <div className="mb-2 text-sm font-semibold">Voice Caching & Performance</div>
                  <div className="flex flex-col gap-3">
                    <label className={`flex items-start justify-between rounded-xl border p-4 cursor-pointer transition ${isVoiceLowRamMode ? isDark ? 'border-emerald-500 bg-emerald-950/25 text-emerald-100' : 'border-emerald-500 bg-emerald-50 text-emerald-900' : isDark ? 'border-zinc-800 hover:bg-zinc-900/60' : 'border-stone-300 hover:bg-stone-50'}`}>
                      <div className="flex flex-col gap-1 pr-4">
                        <span className="text-sm font-semibold">Low RAM Mode</span>
                        <span className={`text-xs leading-5 ${isDark ? 'text-zinc-400' : 'text-slate-500'}`}>Automatically unloads Whisper (STT) and Kokoro (TTS) models from system memory immediately after processing each voice prompt. Reduces RAM usage by up to ~470 MB, but slightly increases latency on the next voice input as models must reload.</span>
                      </div>
                      <input type="checkbox" checked={isVoiceLowRamMode} onChange={(e) => onToggleVoiceLowRam(e.target.checked)} className="mt-1 h-4 w-4 shrink-0 rounded border-stone-300 text-emerald-600 focus:ring-emerald-500 cursor-pointer" />
                    </label>
                    <label className={`flex items-start justify-between rounded-xl border p-4 cursor-pointer transition ${isTtsEnabled ? isDark ? 'border-emerald-500 bg-emerald-950/25 text-emerald-100' : 'border-emerald-500 bg-emerald-50 text-emerald-900' : isDark ? 'border-zinc-800 hover:bg-zinc-900/60' : 'border-stone-300 hover:bg-stone-50'}`}>
                      <div className="flex flex-col gap-1 pr-4">
                        <span className="text-sm font-semibold">Read Aloud by Default</span>
                        <span className={`text-xs leading-5 ${isDark ? 'text-zinc-400' : 'text-slate-500'}`}>Automatically speak assistant responses out loud using the local high-quality voice agent.</span>
                      </div>
                      <input type="checkbox" checked={isTtsEnabled} onChange={(e) => onToggleTts(e.target.checked)} className="mt-1 h-4 w-4 shrink-0 rounded border-stone-300 text-emerald-600 focus:ring-emerald-500 cursor-pointer" />
                    </label>
                  </div>
                </div>
              </div>
            )}

            {/* RAG Tab */}
            {settingsTab === 'rag' && (
              <div className="space-y-5">
                <div>
                  <div className="mb-2 text-sm font-semibold">Document Context (RAG)</div>
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

            {/* Inference Tab */}
            {settingsTab === 'inference' && (
              <div className="space-y-4">
                <div>
                  <label className="mb-2 block text-sm font-semibold" htmlFor="provider-select">Inference Provider</label>
                  <select
                    className={`w-full rounded-lg border px-3 py-2 text-sm outline-none focus:border-emerald-600 ${isDark ? 'border-zinc-800 bg-zinc-900 text-zinc-100' : 'border-stone-300 bg-white text-slate-900'}`}
                    disabled={availableProviders.length === 0}
                    id="provider-select"
                    onChange={(e) => onSelectProvider(e.target.value)}
                    value={activeProvider?.name ?? ''}
                  >
                    <option value="" disabled>{availableProviders.length === 0 ? 'No providers available' : 'Choose provider'}</option>
                    {availableProviders.map((p) => (<option key={p.name} value={p.name}>{p.name}</option>))}
                  </select>
                </div>
                {activeProvider && (
                  <div className={`rounded-xl border p-4 text-sm ${isDark ? 'border-zinc-800 bg-zinc-900/40 text-zinc-300' : 'border-stone-300 bg-stone-50 text-slate-600'}`}>
                    {activeProvider.description}
                  </div>
                )}
              </div>
            )}

            {/* Models Tab */}
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
                      <div className={`p-3 text-sm ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>No catalog models match this filter.</div>
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
                    disabled={availableModels.length === 0 || modelDownloadState === 'downloading'}
                    id="installed-model-select"
                    onChange={(e) => onSelectModel(e.target.value)}
                    value={availableModels.find((m) => m.active)?.name ?? ''}
                  >
                    <option value="" disabled>{availableModels.length === 0 ? 'No installed models found' : 'Choose installed model'}</option>
                    {availableModels.map((m) => (<option key={m.name} value={m.name}>{m.active ? `${m.name} (active)` : m.name}</option>))}
                  </select>
                  <div className={`mt-1 text-xs ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>Selecting an installed model warms it before making it active.</div>
                </div>
              </div>
            )}

            {/* Personalize Tab */}
            {settingsTab === 'personalize' && (
              <div className="space-y-3">
                <div>
                  <div className="text-sm font-semibold">Local Personalization Profile</div>
                  <div className={`mt-1 text-xs ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>{profilePath || 'Markdown save path will appear after the engine responds.'}</div>
                  <div className={`mt-2 text-xs leading-5 ${isDark ? 'text-zinc-400' : 'text-slate-600'}`}>Add identity details, preferences, writing style notes, goals, or context about how you want AEGIS to respond. This is stored locally as a markdown file and injected into model context during inference so replies stay more aligned to you.</div>
                </div>
                <input accept=".txt,.md" className="hidden" onChange={onImportProfile} ref={profileImportInputRef} type="file" />
                <textarea
                  className={`min-h-52 w-full resize-none rounded-xl border p-3 text-sm leading-6 outline-none focus:border-emerald-600 ${isDark ? 'border-zinc-800 bg-zinc-900 text-zinc-100 placeholder:text-zinc-500' : 'border-stone-300 bg-white text-slate-900 placeholder:text-slate-400'}`}
                  onChange={(e) => onProfileTextChange(e.target.value)}
                  placeholder={'Examples:\n- My name is Mohammed.\n- I prefer concise but technically precise answers.\n- I am working on AEGIS and usually want practical implementation help.\n- When explaining code, prioritize architecture before syntax details.'}
                  value={profileText}
                />
                <div className="flex justify-end gap-2">
                  <button className={`rounded-lg border px-4 py-2 text-sm transition ${isDark ? 'border-zinc-800 text-zinc-300 hover:bg-zinc-900' : 'border-stone-300 text-slate-700 hover:bg-stone-100'}`} onClick={() => profileImportInputRef.current?.click()} type="button">
                    Import .txt/.md
                  </button>
                  <button className="rounded-lg bg-emerald-600 px-4 py-2 text-sm font-medium text-white transition hover:bg-emerald-500" onClick={onSaveProfile} type="button">
                    Save Profile
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
