// Popup modal displaying all stored memory entries
import { X, Brain } from 'lucide-react';

interface MemoriesPopupProps {
  isDark: boolean;
  isOpen: boolean;
  memories: string[];
  onClose: () => void;
}

export function MemoriesPopup({ isDark, isOpen, memories, onClose }: MemoriesPopupProps) {
  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-[60] flex items-center justify-center bg-black/60 p-4" onClick={onClose}>
      <div
        className={`w-full max-w-2xl max-h-[70vh] flex flex-col rounded-2xl border shadow-2xl ${isDark ? 'border-zinc-800 bg-zinc-950 text-zinc-100' : 'border-stone-300 bg-white text-slate-900'}`}
        onClick={(e) => e.stopPropagation()}
      >
        <div className={`flex items-center justify-between px-6 py-4 border-b ${isDark ? 'border-zinc-800' : 'border-stone-200'}`}>
          <div className="flex items-center gap-2 text-base font-semibold">
            <Brain size={18} />
            Stored Memories ({memories.length})
          </div>
          <button
            aria-label="Close memories"
            className={`rounded-md p-1 transition ${isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'}`}
            onClick={onClose}
            type="button"
          >
            <X size={18} />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto px-6 py-4 space-y-2">
          {memories.length === 0 ? (
            <p className={`text-sm italic ${isDark ? 'text-zinc-500' : 'text-slate-500'}`}>
              No memories stored yet. Add one in Settings &rarr; Memories.
            </p>
          ) : (
            memories.map((memory, index) => (
              <div
                key={index}
                className={`rounded-lg border px-4 py-3 text-sm leading-6 ${isDark ? 'border-zinc-800 bg-zinc-900/50 text-zinc-200' : 'border-stone-200 bg-stone-50 text-slate-700'}`}
              >
                <span className="font-mono text-[10px] uppercase tracking-wider opacity-50 mr-2">
                  #{index + 1}
                </span>
                {memory}
              </div>
            ))
          )}
        </div>

        <div className={`flex justify-end px-6 py-3 border-t ${isDark ? 'border-zinc-800' : 'border-stone-200'}`}>
          <button
            className={`rounded-lg border px-4 py-2 text-sm transition ${isDark ? 'border-zinc-800 text-zinc-300 hover:bg-zinc-900' : 'border-stone-300 text-slate-700 hover:bg-stone-100'}`}
            onClick={onClose}
            type="button"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
