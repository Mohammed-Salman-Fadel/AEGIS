import { useEffect, useMemo, useRef, useState, type MouseEvent, type RefObject } from 'react';
import { createPortal } from 'react-dom';
import { Clock3, CornerDownLeft, PencilLine, Sparkles } from 'lucide-react';
import type { Message } from '../types';

interface PromptCheckpoint {
  messageIndex: number;
  number: number;
  prompt: Message;
  response?: Message;
}

interface PromptCheckpointsProps {
  messages: Message[];
  scrollContainerRef: RefObject<HTMLDivElement | null>;
  isDark: boolean;
  hidden?: boolean;
}

function compactText(value: string, limit: number) {
  const normalized = value.replace(/\s+/g, ' ').trim();
  return normalized.length > limit ? `${normalized.slice(0, limit).trimEnd()}...` : normalized;
}

function checkpointTime(timestamp?: string) {
  if (!timestamp) return 'Time unavailable';
  const date = new Date(timestamp);
  if (Number.isNaN(date.getTime())) return 'Time unavailable';
  return date.toLocaleString([], { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
}

export function PromptCheckpoints({ messages, scrollContainerRef, isDark, hidden = false }: PromptCheckpointsProps) {
  const [activeMessageIndex, setActiveMessageIndex] = useState<number | null>(null);
  const [hoveredMessageIndex, setHoveredMessageIndex] = useState<number | null>(null);
  const [tooltipPosition, setTooltipPosition] = useState<{ top: number; right: number } | null>(null);
  const hideTooltipTimer = useRef<number | null>(null);
  const checkpoints = useMemo<PromptCheckpoint[]>(() => {
    const userMessageIndexes = messages.flatMap((message, messageIndex) =>
      message.role === 'user' ? [messageIndex] : [],
    );
    return userMessageIndexes.map((messageIndex, index) => {
      const message = messages[messageIndex];
      const response = messages.slice(messageIndex + 1).find((candidate) => candidate.role === 'assistant');
      return { messageIndex, number: index + 1, prompt: message, response };
    });
  }, [messages]);

  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container || checkpoints.length === 0) return;

    const updateActiveCheckpoint = () => {
      const containerTop = container.getBoundingClientRect().top;
      const targetLine = containerTop + Math.min(180, container.clientHeight * 0.3);
      let closest = checkpoints[0].messageIndex;
      let closestDistance = Number.POSITIVE_INFINITY;
      for (const checkpoint of checkpoints) {
        const element = container.querySelector<HTMLElement>(`[data-message-index="${checkpoint.messageIndex}"]`);
        if (!element) continue;
        const distance = Math.abs(element.getBoundingClientRect().top - targetLine);
        if (distance < closestDistance) {
          closest = checkpoint.messageIndex;
          closestDistance = distance;
        }
      }
      setActiveMessageIndex(closest);
    };

    const initialFrame = window.requestAnimationFrame(updateActiveCheckpoint);
    container.addEventListener('scroll', updateActiveCheckpoint, { passive: true });
    const resizeObserver = new ResizeObserver(updateActiveCheckpoint);
    resizeObserver.observe(container);
    return () => {
      window.cancelAnimationFrame(initialFrame);
      container.removeEventListener('scroll', updateActiveCheckpoint);
      resizeObserver.disconnect();
    };
  }, [checkpoints, scrollContainerRef]);

  useEffect(() => () => {
    if (hideTooltipTimer.current !== null) window.clearTimeout(hideTooltipTimer.current);
  }, []);

  if (hidden || checkpoints.length < 2) return null;

  const jumpToCheckpoint = (messageIndex: number) => {
    if (hideTooltipTimer.current !== null) {
      window.clearTimeout(hideTooltipTimer.current);
      hideTooltipTimer.current = null;
    }
    const container = scrollContainerRef.current;
    const target = container?.querySelector<HTMLElement>(`[data-message-index="${messageIndex}"]`);
    target?.scrollIntoView({ behavior: 'smooth', block: 'start' });
    setActiveMessageIndex(messageIndex);
    setHoveredMessageIndex(null);
    setTooltipPosition(null);
  };

  const showTooltip = (messageIndex: number, event: MouseEvent<HTMLDivElement>) => {
    if (hideTooltipTimer.current !== null) {
      window.clearTimeout(hideTooltipTimer.current);
      hideTooltipTimer.current = null;
    }
    const rect = event.currentTarget.getBoundingClientRect();
    setHoveredMessageIndex(messageIndex);
    setTooltipPosition({
      top: Math.max(12, Math.min(rect.top - 120, window.innerHeight - 340)),
      right: Math.max(16, window.innerWidth - rect.right + 16),
    });
  };

  const hideTooltip = () => {
    if (hideTooltipTimer.current !== null) window.clearTimeout(hideTooltipTimer.current);
    hideTooltipTimer.current = window.setTimeout(() => {
      setHoveredMessageIndex(null);
      setTooltipPosition(null);
      hideTooltipTimer.current = null;
    }, 80);
  };

  const hoveredCheckpoint = checkpoints.find((checkpoint) => checkpoint.messageIndex === hoveredMessageIndex);
  const hoveredResponseText = hoveredCheckpoint?.response?.content || 'Response is still being generated or was not saved.';
  const hoveredReasoningCount = hoveredCheckpoint?.response?.reasoningEvents?.length ?? 0;
  const hoveredSourceCount = hoveredCheckpoint?.response?.sources?.length ?? 0;

  return (
    <>
    <nav
      aria-label="Prompt checkpoints"
      className="pointer-events-none absolute bottom-32 right-3 top-28 z-20 hidden w-[min(28rem,calc(100%-5rem))] items-center justify-end lg:flex 2xl:right-5"
    >
      <div className="prompt-checkpoint-scroll pointer-events-none max-h-full w-full overflow-y-auto py-4">
        <div className="relative flex min-h-full flex-col items-end justify-center gap-1 pr-1.5">
          <div className={`pointer-events-none absolute bottom-2 right-[9px] top-2 w-px ${isDark ? 'bg-white/[0.035]' : 'bg-stone-900/[0.06]'}`} />
          {checkpoints.map((checkpoint) => {
            const active = activeMessageIndex === checkpoint.messageIndex;
            return (
              <div
                className="group/checkpoint pointer-events-none relative flex min-h-5 w-full items-center justify-end"
                key={checkpoint.messageIndex}
                onMouseEnter={(event) => showTooltip(checkpoint.messageIndex, event)}
                onMouseLeave={hideTooltip}
              >
                <button
                  aria-label={`Jump to prompt ${checkpoint.number}: ${compactText(checkpoint.prompt.content, 80)}`}
                  className="pointer-events-auto relative z-10 flex h-5 w-9 items-center justify-end outline-none"
                  onClick={() => jumpToCheckpoint(checkpoint.messageIndex)}
                  type="button"
                >
                  <span
                    className={`block h-[2px] rounded-full transition-[width,background-color,box-shadow,transform] duration-300 ease-out group-hover/checkpoint:w-7 group-hover/checkpoint:scale-y-125 group-focus-within/checkpoint:w-7 ${
                      active
                        ? isDark
                          ? 'w-6 bg-zinc-100 shadow-[0_0_10px_rgba(255,255,255,0.28)]'
                          : 'w-6 bg-stone-900 shadow-[0_0_10px_rgba(28,25,23,0.18)]'
                        : isDark
                          ? 'w-2 bg-zinc-600 group-hover/checkpoint:bg-zinc-300'
                          : 'w-2 bg-stone-400 group-hover/checkpoint:bg-stone-700'
                    }`}
                  />
                </button>
              </div>
            );
          })}
        </div>
      </div>
    </nav>
    {hoveredCheckpoint && tooltipPosition && createPortal(
      <div
        className={`pointer-events-auto fixed z-50 max-h-[calc(100vh-1.5rem)] w-[min(25rem,calc(100vw-3rem))] overflow-y-auto rounded-2xl border p-4 shadow-2xl backdrop-blur-xl transition-opacity duration-100 ${
          isDark
            ? 'border-white/10 bg-zinc-900/95 text-zinc-100 shadow-black/45'
            : 'border-stone-300/80 bg-[rgba(255,252,245,0.96)] text-stone-950 shadow-stone-900/15'
        }`}
        role="tooltip"
        style={{ top: tooltipPosition.top, right: tooltipPosition.right }}
        onMouseEnter={() => {
          if (hideTooltipTimer.current !== null) {
            window.clearTimeout(hideTooltipTimer.current);
            hideTooltipTimer.current = null;
          }
        }}
        onMouseLeave={hideTooltip}
      >
        <div className="flex items-start gap-3">
          <span className={`mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-[11px] font-semibold ${isDark ? 'bg-white/[0.07] text-zinc-300' : 'bg-stone-900/[0.06] text-stone-700'}`}>
            {hoveredCheckpoint.number}
          </span>
          <div className="min-w-0 flex-1">
            <div className="aegis-display max-h-32 overflow-y-auto break-words text-[14px] font-semibold leading-5">
              {hoveredCheckpoint.prompt.content}
            </div>
            <p className={`mt-2 max-h-40 overflow-y-auto whitespace-pre-wrap break-words text-[12px] leading-5 ${isDark ? 'text-zinc-400' : 'text-stone-600'}`}>
              {hoveredResponseText}
            </p>
          </div>
        </div>
        <div className={`mt-3 flex flex-wrap items-center gap-x-3 gap-y-1.5 border-t pt-3 text-[10px] ${isDark ? 'border-white/[0.07] text-zinc-500' : 'border-stone-900/[0.08] text-stone-500'}`}>
          <span className="inline-flex items-center gap-1"><Clock3 size={11} />{checkpointTime(hoveredCheckpoint.prompt.timestamp)}</span>
          {hoveredCheckpoint.prompt.edited && <span className="inline-flex items-center gap-1"><PencilLine size={11} />Edited</span>}
          {hoveredReasoningCount > 0 && <span className="inline-flex items-center gap-1"><Sparkles size={11} />{hoveredReasoningCount} reasoning steps</span>}
          {hoveredSourceCount > 0 && <span className="inline-flex items-center gap-1"><CornerDownLeft size={11} />{hoveredSourceCount} sources</span>}
        </div>
      </div>,
      document.body,
    )}
    </>
  );
}
