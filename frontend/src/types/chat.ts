// Chat message types, markdown block types, and retrieval chunk types

export type Role = 'user' | 'assistant';

export interface RetrievalChunk {
  text: string;
  source: string;
  page?: number;
  score: number;
}

export interface ReasoningEvent {
  phase: string;
  title: string;
  detail?: string;
  round?: number;
  tool?: string;
}

export interface ContextCompactionEvent {
  removed_turns: number;
  kept_turns: number;
  estimated_tokens_before: number;
  estimated_tokens_after: number;
  history_budget_tokens: number;
  context_window_tokens: number;
}

export interface Message {
  role: Role;
  content: string;
  edited?: boolean;
  timestamp?: string;
  sources?: RetrievalChunk[];
  reasoningEvents?: ReasoningEvent[];
}

export type ChatMode = 'general' | 'coder' | 'academic';

export type MarkdownHeadingLevel = 1 | 2 | 3 | 4 | 5 | 6;

export type MarkdownBlock =
  | { type: 'heading'; level: MarkdownHeadingLevel; text: string }
  | { type: 'paragraph'; text: string }
  | { type: 'ordered'; items: string[] }
  | { type: 'unordered'; items: string[] }
  | { type: 'code'; text: string; language: string }
  | { type: 'hr' };

export type ImportPhase = 'idle' | 'uploading' | 'indexing' | 'complete' | 'error';
