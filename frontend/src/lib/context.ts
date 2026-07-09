// Context usage API helpers
import type { ContextUsage } from '../types';
import { API_BASE, EMPTY_CONTEXT_USAGE } from '../constants';

export function normalizeContextUsage(data: Partial<ContextUsage>): ContextUsage {
  return {
    provider: String(data.provider ?? ''),
    model: String(data.model ?? ''),
    used_tokens: Math.max(0, Math.round(Number(data.used_tokens ?? 0))),
    context_window: Math.max(0, Math.round(Number(data.context_window ?? 0))),
    usage_source: String(data.usage_source ?? ''),
  };
}

export async function fetchContextUsage(sessionId: string | null): Promise<ContextUsage> {
  const params = new URLSearchParams();
  if (sessionId) params.set('session_id', sessionId);
  const query = params.toString();
  const suffix = query ? `?${query}` : '';
  const url = `${API_BASE}/context/usage${suffix}`;
  const response = await fetch(url);
  if (!response.ok) throw new Error(`Engine returned HTTP ${response.status} while loading context usage.`);
  return normalizeContextUsage((await response.json()) as Partial<ContextUsage>);
}

export function formatTokenMeter(usage: ContextUsage) {
  if (usage.usage_source === 'unavailable') return 'Tokens unavailable';
  if (usage.context_window <= 0) return 'Loading tokens...';
  const used = usage.used_tokens.toLocaleString();
  const limit = usage.context_window.toLocaleString();
  return `${used} / ${limit}`;
}
