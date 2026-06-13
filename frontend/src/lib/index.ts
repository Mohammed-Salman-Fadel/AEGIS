// Barrel export for all lib utilities

export { normalizeContextUsage, fetchContextUsage, formatTokenMeter } from './context';
export {
  normalizeAssistantMarkdownProse,
  normalizeAssistantMarkdown,
  parseMarkdownBlocks,
  renderInlineMarkdown,
  renderHighlightedCodeLine,
  normalizedCodeLanguage,
} from './markdown';
export { createConversationPdf, safeExportFileName, downloadConversationPdf } from './pdf';
export { scanProjectDirectory, buildProjectSnapshot, findProjectFile, shouldReadProjectFile } from './project';
export { extractUnifiedDiff, parsePatchTarget, applySimpleUnifiedDiff } from './diff';
export { extractSseEvents, sseEventData, splitAssistantStreamSegments } from './sse';
export {
  loadStoredTheme,
  loadStoredVoiceLowRamMode,
  loadStoredTtsEnabled,
  loadStoredRagEnabled,
  loadStoredRagTopK,
  loadStoredRagThreshold,
  loadStoredIndexedDocumentsBySession,
  loadStoredPinnedSessionIds,
  loadStoredResponseStyle,
  loadStoredAppearanceTheme,
} from './storage';
export { cleanOutlookCalendarName, isVisibleOutlookCalendar, outlookCalendarLabel } from './calendar';
export { sanitizeTextForTts } from './tts';
export { copyTextToClipboard } from './clipboard';
export { fitTextareaToContent, isFatalUiError, importPhaseLabel, sessionUpdatedAtMs, formatSessionLastAccessed } from './ui';
export { parseWelcomeMessages, randomWelcomeMessage, profileDisplayName, personalizeWelcomeMessage } from './profile';
export { turnsToMessages, mergeIndexedDocuments } from './sessions';
