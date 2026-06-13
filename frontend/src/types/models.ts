// Model and provider types

export interface CatalogModel {
  name: string;
  provider: string;
  tags: string[];
  description: string;
}

export interface ModelResponse {
  name: string;
  description: string;
  active: boolean;
}

export interface ModelListResponse {
  provider: string;
  models: ModelResponse[];
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
