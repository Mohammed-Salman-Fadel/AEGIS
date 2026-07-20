// Modal requesting write permission for a scanned project folder
import { FolderOpen } from 'lucide-react';
import { useDialogA11y } from '../hooks/useDialogA11y';

interface ProjectPermissionModalProps {
  isDark: boolean;
  onClose: () => void;
  onKeepReadonly: () => void;
  onRequestEditAccess: () => void;
}

export function ProjectPermissionModal({ isDark, onClose, onKeepReadonly, onRequestEditAccess }: ProjectPermissionModalProps) {
  const dialogRef = useDialogA11y(true, onClose);
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4" onClick={onClose}>
      <div
        aria-describedby="project-permission-description"
        aria-labelledby="project-permission-title"
        aria-modal="true"
        className={`w-full max-w-md rounded-2xl border p-6 shadow-2xl ${isDark ? 'border-zinc-800 bg-zinc-950 text-zinc-100' : 'border-stone-300 bg-white text-slate-900'}`}
        onClick={(e) => e.stopPropagation()}
        ref={dialogRef}
        role="dialog"
        tabIndex={-1}
      >
        <div className="mb-3 flex items-center gap-2 text-lg font-semibold" id="project-permission-title">
          <FolderOpen size={18} />
          Allow Project Edits?
        </div>
        <p className={`text-sm leading-6 ${isDark ? 'text-zinc-400' : 'text-slate-600'}`} id="project-permission-description">
          AEGIS has scanned this project for context. To edit files, it must request browser
          write permission, and patches will still require your explicit approval before they are applied.
        </p>
        <div className="mt-6 flex justify-end gap-2">
          <button
            className={`rounded-lg border px-4 py-2 text-sm transition ${isDark ? 'border-zinc-800 text-zinc-300 hover:bg-zinc-900' : 'border-stone-300 text-slate-700 hover:bg-stone-100'}`}
            data-dialog-initial-focus
            onClick={onKeepReadonly}
            type="button"
          >
            Keep read-only
          </button>
          <button
            className="rounded-lg bg-emerald-600 px-4 py-2 text-sm font-medium text-white transition hover:bg-emerald-500"
            onClick={onRequestEditAccess}
            type="button"
          >
            Request edit access
          </button>
        </div>
      </div>
    </div>
  );
}
