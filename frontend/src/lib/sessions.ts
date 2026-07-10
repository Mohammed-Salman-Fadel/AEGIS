// Session utility functions
import type { Message, RetrievalChunk, EngineTurn } from '../types/index.js';

export function turnsToMessages(turns: EngineTurn[], sessionId: string): Message[] {
  return turns.flatMap((turn, turnIdx) => {
    const assistantIdx = turnIdx * 2 + 1;
    let sources: RetrievalChunk[] | undefined = undefined;
    const saved = localStorage.getItem(`aegis-sources-${sessionId}-${assistantIdx}`);
    if (saved) {
      try { sources = JSON.parse(saved); }
      catch (e) { console.error('Failed to parse saved sources:', e); }
    }
    return [
      { role: 'user' as const, content: turn.query, edited: turn.edited, timestamp: turn.created_at },
      { role: 'assistant' as const, content: turn.response, timestamp: turn.created_at, sources },
    ];
  });
}

export function mergeIndexedDocuments(
  currentDocuments: any[],
  nextDocuments: any[],
) {
  const merged = new Map<string, any>();
  currentDocuments.forEach((doc) => merged.set(doc.stored_path, doc));
  nextDocuments.forEach((doc) => merged.set(doc.stored_path, doc));
  return Array.from(merged.values());
}
