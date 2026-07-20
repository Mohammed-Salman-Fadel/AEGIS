// Model and provider types

export interface CatalogModel {
  name: string;
  provider: string;
  source: 'ollama' | 'huggingface';
  tags: string[];
  description: string;
}

export interface ModelResponse {
  name: string;
  description: string;
  active: boolean;
  status?: 'installed' | 'warming' | 'ready' | 'degraded';
  provider?: string;
  supports_managed_download?: boolean;
}

export interface ModelListResponse {
  provider: string;
  models: ModelResponse[];
  warning?: string;
}

export interface ProviderResponse {
  name: string;
  description: string;
  active: boolean;
}

export interface ProviderListResponse {
  providers: ProviderResponse[];
}

export type ModelDownloadState = 'idle' | 'downloading' | 'paused';
