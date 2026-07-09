// General UI utility functions
import type { ImportPhase } from '../types';

export function fitTextareaToContent(textarea: HTMLTextAreaElement) {
  textarea.style.height = '0px';
  const maxHeight = Math.max(window.innerHeight * 0.55, 160);
  textarea.style.height = `${Math.min(textarea.scrollHeight, maxHeight)}px`;
}

export function isFatalUiError(message: string) {
  const normalized = message.toLowerCase();
  return normalized.includes('fatal') || normalized.includes('unrecoverable');
}

export function importPhaseLabel(phase: ImportPhase, fileLabel: string) {
  const target = fileLabel ? ` ${fileLabel}` : ' document';
  switch (phase) {
    case 'uploading': return `Uploading${target}`;
    case 'indexing': return `Reading and indexing${target}`;
    case 'complete': return `Finished indexing${target}`;
    case 'error': return `Import failed${target}`;
    case 'idle':
    default: return 'Ready to import';
  }
}

export function sessionUpdatedAtMs(session: { updated_at: string }) {
  const timestamp = Date.parse(session.updated_at);
  return Number.isNaN(timestamp) ? 0 : timestamp;
}

export function formatSessionLastAccessed(updatedAt: string) {
  const timestamp = Date.parse(updatedAt);
  if (Number.isNaN(timestamp)) return 'Unavailable';
  return new Intl.DateTimeFormat(undefined, {
    month: 'short', day: 'numeric', year: 'numeric',
    hour: 'numeric', minute: '2-digit',
  }).format(new Date(timestamp));
}
