// Performance metrics sidebar showing system stats, inference metrics, and RAG analysis
import { PanelLeftClose, BookOpen, FileText } from 'lucide-react';
import type { SystemStats, RetrievalChunk, InferenceStats } from '../types';
import { useT } from '../lib/i18n';

interface MetricsSidebarProps {
  isDark: boolean;
  isMetricsOpen: boolean;
  metricsTab: 'metrics' | 'sources';
  systemStats: SystemStats;
  inferenceStats: InferenceStats;
  selectedMessageSources: RetrievalChunk[] | null;
  selectedMessageSourcesIndex: number | null;
  onClose: () => void;
  onSetMetricsTab: (tab: 'metrics' | 'sources') => void;
  onClearSelection: () => void;
}

export function MetricsSidebar({
  isDark,
  isMetricsOpen,
  metricsTab,
  systemStats,
  inferenceStats,
  selectedMessageSources,
  selectedMessageSourcesIndex,
  onClose,
  onSetMetricsTab,
  onClearSelection,
}: MetricsSidebarProps) {
  const t = useT();
  return (
    <aside
      className={`flex shrink-0 flex-col border-l transition-all duration-300 ease-in-out ${isMetricsOpen ? 'w-72' : 'w-0 border-transparent p-0'} ${isDark ? 'border-zinc-800 bg-zinc-950' : 'border-stone-300 bg-stone-50'}`}
    >
      <div className={`flex h-full flex-col overflow-hidden ${isMetricsOpen ? 'opacity-100' : 'pointer-events-none opacity-0'}`}>
        <div className="flex h-16 shrink-0 items-center justify-between px-6 border-b dark:border-zinc-900 border-stone-200">
            <div className="text-xs font-bold uppercase tracking-wider text-zinc-500">
              {metricsTab === 'sources' ? t('metrics.sources') : t('metrics.performance')}
            </div>
          <button
            className={`rounded-md p-1 transition ${isDark ? 'text-zinc-500 hover:bg-zinc-900 hover:text-zinc-300' : 'text-slate-400 hover:bg-stone-200 hover:text-slate-600'}`}
            onClick={onClose}
            type="button"
          >
            <PanelLeftClose className="rotate-180" size={16} />
          </button>
        </div>

        <div className={`flex border-b shrink-0 ${isDark ? 'border-zinc-800' : 'border-stone-200'}`}>
          <button
            className={`flex-1 py-3 text-center text-[10px] font-bold uppercase tracking-wider transition ${metricsTab === 'metrics' ? 'border-b-2 border-emerald-500 text-emerald-500' : isDark ? 'text-zinc-500 hover:text-zinc-300' : 'text-slate-500 hover:text-slate-700'}`}
            onClick={() => onSetMetricsTab('metrics')}
            type="button"
          >
              {t('metrics.live_stats')}
            </button>
            <button
              className={`flex-1 py-3 text-center text-[10px] font-bold uppercase tracking-wider transition relative ${metricsTab === 'sources' ? 'border-b-2 border-emerald-500 text-emerald-500' : isDark ? 'text-zinc-500 hover:text-zinc-300' : 'text-slate-500 hover:text-slate-700'}`}
              onClick={() => onSetMetricsTab('sources')}
              type="button"
            >
              {t('metrics.sources')}
            {selectedMessageSources && selectedMessageSources.length > 0 && (
              <span className="absolute right-3.5 top-2.5 flex h-4 w-4 items-center justify-center rounded-full bg-emerald-500 text-[9px] font-extrabold text-white">
                {selectedMessageSources.length}
              </span>
            )}
          </button>
        </div>

        {metricsTab === 'sources' ? (
          <div className="flex-1 overflow-y-auto p-5 space-y-4">
            {selectedMessageSources && selectedMessageSources.length > 0 ? (
              <>
                <div className="flex items-center justify-between">
                  <span className="text-[10px] font-bold uppercase tracking-wider text-zinc-500">Retrieved Excerpts</span>
                  <button className={`text-[9px] font-bold uppercase tracking-wider transition hover:text-emerald-500 ${isDark ? 'text-zinc-400' : 'text-slate-500'}`} onClick={onClearSelection}>
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
                      <div key={sIdx} className={`rounded-lg border p-3.5 space-y-2.5 text-xs transition duration-200 ${isDark ? 'border-zinc-800 bg-zinc-900/30 hover:bg-zinc-900/50 text-zinc-300' : 'border-stone-200 bg-white hover:bg-stone-50 text-slate-800 shadow-sm'}`}>
                        <div className="flex flex-wrap items-center justify-between gap-1.5 border-b pb-2 border-dashed border-stone-200 dark:border-zinc-800/60">
                          <div className="flex items-center gap-1.5 font-bold text-emerald-600 dark:text-emerald-400 truncate max-w-[70%]">
                            <FileText size={12} className="shrink-0" />
                            <span className="truncate" title={filename}>{filename}</span>
                          </div>
                          <div className="flex items-center gap-1 shrink-0">
                            {page !== undefined && page !== null && (
                              <span className={`px-1.5 py-0.5 rounded text-[9px] font-extrabold uppercase tracking-wider ${isDark ? 'bg-zinc-800 text-zinc-400' : 'bg-stone-100 text-slate-500'}`}>Pg {page}</span>
                            )}
                            {!isLegacyString && (
                              <span className={`font-mono text-[9px] px-1.5 py-0.5 rounded font-extrabold uppercase tracking-wider ${isDark ? 'bg-emerald-950/40 text-emerald-400' : 'bg-emerald-50 text-emerald-700'}`}>
                                {(score * 100).toFixed(0)}%
                              </span>
                            )}
                          </div>
                        </div>
                        {text ? (
                          <div className={`font-serif leading-relaxed p-2.5 rounded border border-dashed text-[11px] overflow-y-auto max-h-48 whitespace-pre-wrap ${isDark ? 'border-zinc-800/80 bg-zinc-950/50 text-zinc-400' : 'border-stone-200 bg-stone-50/50 text-slate-600'}`}>
                            {text}
                          </div>
                        ) : (
                          <div className="text-[11px] italic text-zinc-500">No excerpt available for legacy reference format.</div>
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
                  <h3 className={`text-xs font-bold uppercase tracking-wider ${isDark ? 'text-zinc-300' : 'text-slate-800'}`}>No Turn Selected</h3>
                  <p className="text-[11px] leading-relaxed text-zinc-500">
                    Click the <span className="inline-flex items-center align-middle font-bold text-emerald-500">sources</span> button on any AI response to inspect retrieved context excerpts in detail.
                  </p>
                </div>
              </div>
            )}
          </div>
        ) : (
          <div className="flex-1 space-y-8 overflow-y-auto p-6">
            <div className="space-y-4">
              <div className="text-xs font-semibold uppercase tracking-wider text-zinc-500">System Resources</div>
              <div>
                <div className="mb-2 flex justify-between text-xs">
                    <span className={isDark ? 'text-zinc-400' : 'text-slate-500'}>{t('metrics.cpu')}</span>
                  <span className="font-mono font-medium">{systemStats.cpu}%</span>
                </div>
                <div className={`h-1.5 w-full overflow-hidden rounded-full ${isDark ? 'bg-zinc-800' : 'bg-stone-200'}`}>
                  <div className={`h-full transition-all duration-500 ${systemStats.cpu > 85 ? 'bg-red-500' : systemStats.cpu > 60 ? 'bg-amber-500' : 'bg-emerald-500'}`} style={{ width: `${systemStats.cpu}%` }} />
                </div>
              </div>
              <div>
                <div className="mb-2 flex justify-between text-xs">
                    <span className={isDark ? 'text-zinc-400' : 'text-slate-500'}>{t('metrics.ram')}</span>
                  <span className="font-mono font-medium">{systemStats.ram}%</span>
                </div>
                <div className={`h-1.5 w-full overflow-hidden rounded-full ${isDark ? 'bg-zinc-800' : 'bg-stone-200'}`}>
                  <div className={`h-full transition-all duration-500 ${systemStats.ram > 85 ? 'bg-red-500' : systemStats.ram > 60 ? 'bg-amber-500' : 'bg-emerald-500'}`} style={{ width: `${systemStats.ram}%` }} />
                </div>
              </div>
            </div>

            <div className={`h-px w-full ${isDark ? 'bg-zinc-800' : 'bg-stone-200'}`} />

            <div>
              <div className="mb-4 text-xs font-semibold uppercase tracking-wider text-zinc-500">Inference Engine</div>
              <div className="grid grid-cols-2 gap-3">
                <div className={`rounded-lg border p-3 ${isDark ? 'border-zinc-800 bg-zinc-900/40' : 'border-stone-300 bg-white'}`}>
                    <div className="mb-1 text-[10px] uppercase text-zinc-500">{t('metrics.latency')}</div>
                    <div className="font-mono text-sm font-semibold">{inferenceStats.latency > 0 ? `${(inferenceStats.latency / 1000).toFixed(2)}s` : '---'}</div>
                  </div>
                  <div className={`rounded-lg border p-3 ${isDark ? 'border-zinc-800 bg-zinc-900/40' : 'border-stone-300 bg-white'}`}>
                    <div className="mb-1 text-[10px] uppercase text-zinc-500">{t('metrics.tps')}</div>
                    <div className="font-mono text-sm font-semibold">{inferenceStats.tps > 0 ? `${inferenceStats.tps}` : '---'}</div>
                  </div>
                  <div className={`rounded-lg border p-3 ${isDark ? 'border-zinc-800 bg-zinc-900/40' : 'border-stone-300 bg-white'}`}>
                    <div className="mb-1 text-[10px] uppercase text-zinc-500">{t('metrics.ttft')}</div>
                    <div className="font-mono text-sm font-semibold">{inferenceStats.ttft > 0 ? `${inferenceStats.ttft}ms` : '---'}</div>
                  </div>
                  <div className={`rounded-lg border p-3 ${isDark ? 'border-zinc-800 bg-zinc-900/40' : 'border-stone-300 bg-white'}`}>
                    <div className="mb-1 text-[10px] uppercase text-zinc-500">{t('metrics.rag_delay')}</div>
                  <div className="font-mono text-sm font-semibold">{inferenceStats.ragTime > 0 ? `${inferenceStats.ragTime}ms` : '---'}</div>
                </div>
              </div>
            </div>

            <div className={`h-px w-full ${isDark ? 'bg-zinc-800' : 'bg-stone-200'}`} />

            <div>
              <div className="mb-4 text-xs font-semibold uppercase tracking-wider text-zinc-500">RAG Engine Analysis</div>
              <div className="space-y-4">
                <div>
                  <div className="mb-1.5 flex justify-between text-[11px]">
                    <span className={isDark ? 'text-zinc-400' : 'text-slate-500'}>{t('metrics.similarity')}</span>
                    <span className="font-mono font-medium">{inferenceStats.similarity > 0 ? `${(inferenceStats.similarity * 100).toFixed(0)}%` : '---'}</span>
                  </div>
                  <div className={`h-1 w-full overflow-hidden rounded-full ${isDark ? 'bg-zinc-800' : 'bg-stone-200'}`}>
                    <div className="h-full bg-emerald-500 opacity-60 transition-all duration-500" style={{ width: `${inferenceStats.similarity * 100}%` }} />
                  </div>
                </div>
                <div className="grid grid-cols-2 gap-3">
                  <div className={`rounded-lg border p-2 ${isDark ? 'border-zinc-800 bg-zinc-900/40' : 'border-stone-300 bg-white'}`}>
                    <div className="mb-0.5 text-[9px] uppercase text-zinc-500">{t('metrics.chunks')}</div>
                    <div className="font-mono text-xs font-semibold">{inferenceStats.chunks || '0'}</div>
                  </div>
                  <div className={`rounded-lg border p-2 ${isDark ? 'border-zinc-800 bg-zinc-900/40' : 'border-stone-300 bg-white'}`}>
                    <div className="mb-0.5 text-[9px] uppercase text-zinc-500">{t('metrics.backend')}</div>
                    <div className="truncate font-mono text-[10px] font-semibold">{inferenceStats.backend}</div>
                  </div>
                </div>
              </div>
            </div>

            <div className={`rounded-lg p-3 text-[11px] leading-relaxed ${isDark ? 'bg-zinc-900/60 text-zinc-500' : 'bg-stone-100 text-slate-500'}`}>
              {t('metrics.disclaimer')}
            </div>
          </div>
        )}
      </div>
    </aside>
  );
}
