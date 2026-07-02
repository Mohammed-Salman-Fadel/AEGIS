export interface PullModelChunk {
  status?: string;
  digest?: string;
  total?: number;
  completed?: number;
  error?: string;
}

export function modelDownloadPercent(chunk: PullModelChunk) {
  if (chunk.total && chunk.total > 0 && typeof chunk.completed === 'number') {
    return Math.max(0, Math.min(100, Math.round((chunk.completed / chunk.total) * 100)));
  }

  if (chunk.status === 'success') {
    return 100;
  }

  return null;
}

export function modelSearchPlaceholder(providerName?: string) {
  return providerName === 'lmstudio'
    ? 'Enter an LM Studio catalog ID or Hugging Face URL'
    : 'Search catalog or enter an exact model tag';
}

export function installedModelsLabel(providerName?: string) {
  return `Installed ${providerName === 'lmstudio' ? 'LM Studio' : 'Ollama'} Models`;
}

export function modelReadyMessage(modelName: string, providerName?: string) {
  return `${modelName} is ready for ${providerName || 'active provider'}.`;
}

const LM_STUDIO_MODEL_ALIASES: Record<string, string> = {
  'llama3.1:8b': 'lmstudio-community/Llama-3.1-8B-Instruct-GGUF',
  'llama3.1:70b': 'lmstudio-community/Llama-3.1-70B-Instruct-GGUF',
  'llama3.1:405b': 'lmstudio-community/Llama-3.1-405B-Instruct-GGUF',
  'llama3:8b': 'lmstudio-community/Llama-3-8B-Instruct-GGUF',
  'llama3:70b': 'lmstudio-community/Llama-3-70B-Instruct-GGUF',
  'llama3.2:1b': 'lmstudio-community/Llama-3.2-1B-Instruct-GGUF',
  'llama3.2:3b': 'lmstudio-community/Llama-3.2-3B-Instruct-GGUF',
  'qwen3:8b': 'lmstudio-community/Qwen3-8B-Instruct-GGUF',
  'qwen2.5:7b': 'lmstudio-community/Qwen2.5-7B-Instruct-GGUF',
  'mistral:7b': 'lmstudio-community/Mistral-7B-Instruct-v0.3-GGUF',
  'gemma2:9b': 'lmstudio-community/gemma-2-9b-it-GGUF',
};

function looksLikeLmStudioModelIdentifier(value: string) {
  return value.includes('/') || value.startsWith('http://') || value.startsWith('https://');
}

export function normalizeModelDownloadName(modelName: string, providerName?: string) {
  const trimmed = modelName.trim();
  if (providerName !== 'lmstudio' || !trimmed) {
    return trimmed;
  }

  if (looksLikeLmStudioModelIdentifier(trimmed)) {
    return trimmed;
  }

  return LM_STUDIO_MODEL_ALIASES[trimmed.toLowerCase()] ?? trimmed;
}
