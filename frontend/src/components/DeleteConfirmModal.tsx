// Confirmation modal before permanently deleting a session
import { X } from 'lucide-react';
import type { EngineSessionSummary } from '../types';
import { useDialogA11y } from '../hooks/useDialogA11y';

interface DeleteConfirmModalProps {
  isDark: boolean;
  session: EngineSessionSummary;
  onClose: () => void;
  onConfirm: () => void;
}

export function DeleteConfirmModal({ isDark, session, onClose, onConfirm }: DeleteConfirmModalProps) {
  const dialogRef = useDialogA11y(true, onClose);
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4" onClick={onClose}>
      <div
        aria-describedby="delete-dialog-description"
        aria-labelledby="delete-dialog-title"
        aria-modal="true"
        className={`w-full max-w-md rounded-2xl border p-6 shadow-2xl ${isDark ? 'border-zinc-800 bg-zinc-950 text-zinc-100' : 'border-stone-300 bg-white text-slate-900'}`}
        onClick={(e) => e.stopPropagation()}
        ref={dialogRef}
        role="dialog"
        tabIndex={-1}
      >
        <div className="mb-3 flex items-center justify-between gap-4">
          <div className="text-lg font-semibold" id="delete-dialog-title">Delete Conversation</div>
          <button
            aria-label="Cancel deletion"
            className={`rounded-md p-1 transition ${isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'}`}
            onClick={onClose}
            type="button"
          >
            <X size={18} />
          </button>
        </div>
        <p className={`text-sm leading-6 ${isDark ? 'text-zinc-400' : 'text-slate-600'}`} id="delete-dialog-description">
          This will permanently delete &ldquo;{session.title}&rdquo; and its saved conversation history. This action cannot be undone.
        </p>
        <div className="mt-6 flex justify-end gap-2">
          <button
            className={`rounded-lg border px-4 py-2 text-sm transition ${isDark ? 'border-zinc-800 text-zinc-300 hover:bg-zinc-900' : 'border-stone-300 text-slate-700 hover:bg-stone-100'}`}
            onClick={onClose}
            type="button"
          >
            Cancel
          </button>
          <button
            className="rounded-lg bg-red-600 px-4 py-2 text-sm font-medium text-white transition hover:bg-red-500"
            data-dialog-initial-focus
            onClick={onConfirm}
            type="button"
          >
            Delete permanently
          </button>
        </div>
      </div>
    </div>
  );
}
