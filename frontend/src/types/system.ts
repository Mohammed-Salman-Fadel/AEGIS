// System stats and context usage types

export interface SystemStats {
  cpu: number;
  ram: number;
}

export interface ContextUsage {
  provider: string;
  model: string;
  used_tokens: number;
  context_window: number;
  usage_source: string;
}

export interface IndexedDocument {
  file_name: string;
  stored_path: string;
  chunks_added: number;
}

export interface IngestResponse {
  status: string;
  total_chunks: number;
  documents: IndexedDocument[];
  session?: EngineSession | null;
}

export interface DeleteIndexedDocumentResponse {
  status: string;
  deleted_chunks: number;
}

export interface ProfileResponse {
  contents: string;
  path: string;
}

import type { EngineSession } from './sessions.js';

export interface InferenceStats {
  latency: number;
  tps: number;
  ttft: number;
  ragTime: number;
  similarity: number;
  chunks: number;
  backend: string;
}
