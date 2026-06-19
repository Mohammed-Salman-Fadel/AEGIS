// LocalStorage read helpers for persisted UI state
import type { ResponseStyle } from '../types';
import {
  THEME_STORAGE_KEY,
  APPEARANCE_THEME_STORAGE_KEY,
  INDEXED_DOCUMENTS_STORAGE_KEY,
  PINNED_SESSIONS_STORAGE_KEY,
  RESPONSE_STYLE_STORAGE_KEY,
  VOICE_LOW_RAM_MODE_STORAGE_KEY,
  VOICE_TTS_ENABLED_STORAGE_KEY,
  RAG_ENABLED_STORAGE_KEY,
  RAG_TOP_K_STORAGE_KEY,
  RAG_THRESHOLD_STORAGE_KEY,
} from '../constants';

export function loadStoredTheme(): 'dark' | 'light' | 'system' {
  if (typeof window === 'undefined') return 'dark';
  const stored = window.localStorage.getItem(THEME_STORAGE_KEY);
  if (stored === 'system') return 'system';
  if (stored === 'light') return 'light';
  return 'dark';
}

export function loadStoredVoiceLowRamMode(): boolean {
  if (typeof window === 'undefined') return false;
  try {
    const stored = window.localStorage.getItem(VOICE_LOW_RAM_MODE_STORAGE_KEY);
    return stored ? JSON.parse(stored) === true : false;
  } catch { return false; }
}

export function loadStoredTtsEnabled(): boolean {
  if (typeof window === 'undefined') return false;
  try {
    const stored = window.localStorage.getItem(VOICE_TTS_ENABLED_STORAGE_KEY);
    return stored ? JSON.parse(stored) === true : false;
  } catch { return false; }
}

export function loadStoredRagEnabled(): boolean {
  if (typeof window === 'undefined') return true;
  try {
    const stored = window.localStorage.getItem(RAG_ENABLED_STORAGE_KEY);
    return stored ? JSON.parse(stored) === true : true;
  } catch { return true; }
}

export function loadStoredRagTopK(): number {
  if (typeof window === 'undefined') return 5;
  try {
    const stored = window.localStorage.getItem(RAG_TOP_K_STORAGE_KEY);
    return stored ? Math.max(1, Math.min(10, Number(JSON.parse(stored)))) : 5;
  } catch { return 5; }
}

export function loadStoredRagThreshold(): number {
  if (typeof window === 'undefined') return 0.0;
  try {
    const stored = window.localStorage.getItem(RAG_THRESHOLD_STORAGE_KEY);
    return stored ? Math.max(0.0, Math.min(1.0, Number(JSON.parse(stored)))) : 0.0;
  } catch { return 0.0; }
}

export function loadStoredIndexedDocumentsBySession(): Record<string, any[]> {
  if (typeof window === 'undefined') return {};
  try {
    const raw = window.localStorage.getItem(INDEXED_DOCUMENTS_STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === 'object' && !Array.isArray(parsed) ? parsed : {};
  } catch { return {}; }
}

export function loadStoredPinnedSessionIds(): string[] {
  if (typeof window === 'undefined') return [];
  try {
    const raw = window.localStorage.getItem(PINNED_SESSIONS_STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter((id): id is string => typeof id === 'string') : [];
  } catch { return []; }
}

export function loadStoredResponseStyle(): ResponseStyle {
  if (typeof window === 'undefined') return 'default';
  const stored = window.localStorage.getItem(RESPONSE_STYLE_STORAGE_KEY);
  const validOptions: ResponseStyle[] = ['default', 'friendly', 'concise', 'elaborate', 'technical'];
  return validOptions.includes(stored as ResponseStyle) ? (stored as ResponseStyle) : 'default';
}

export function loadStoredAppearanceTheme(): string {
  if (typeof window === 'undefined') return 'default';
  const stored = window.localStorage.getItem(APPEARANCE_THEME_STORAGE_KEY);
  const validOptions = ['default', 'terminal', 'ocean', 'ember', 'rose', 'slate'];
  return validOptions.includes(stored ?? '') ? stored! : 'default';
}
