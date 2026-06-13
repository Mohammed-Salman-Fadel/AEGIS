// Barrel export for all types

export type {
  Role,
  RetrievalChunk,
  Message,
  ChatMode,
  MarkdownHeadingLevel,
  MarkdownBlock,
  ImportPhase,
} from './chat';

export type {
  CatalogModel,
  ModelResponse,
  ModelListResponse,
  ProviderResponse,
  ProviderListResponse,
  ModelDownloadState,
} from './models';

export type {
  EngineSessionSummary,
  EngineSessionsResponse,
  EngineTurn,
  EngineSession,
} from './sessions';

export type {
  CalendarResult,
  CalendarCreateResponse,
  OutlookCalendar,
  OutlookCalendarsResponse,
  OutlookCalendarSelectionResponse,
} from './calendar';

export type {
  FileSystemHandlePermissionDescriptor,
  FileSystemHandle,
  FileSystemFileHandle,
  FileSystemDirectoryHandle,
  FileSystemWritableFileStream,
  ProjectFileSnapshot,
  CodeProject,
} from './projects';

export type {
  ThemeMode,
  AppearanceTheme,
  SettingsTab,
  ResponseStyle,
} from './settings';

export type {
  SystemStats,
  ContextUsage,
  IndexedDocument,
  IngestResponse,
  DeleteIndexedDocumentResponse,
  InferenceStats,
} from './system';
