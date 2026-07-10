// Barrel export for all lib utilities

export { normalizeContextUsage, fetchContextUsage, formatTokenMeter } from './context.js';
export {
  normalizeAssistantMarkdownProse,
  normalizeAssistantMarkdown,
  parseMarkdownBlocks,
  renderInlineMarkdown,
  renderHighlightedCodeLine,
  normalizedCodeLanguage,
} from './markdown.js';
export { createConversationPdf, safeExportFileName, downloadConversationPdf } from './pdf.js';
export { scanProjectDirectory, buildProjectSnapshot, findProjectFile, shouldReadProjectFile } from './project.js';
export { extractUnifiedDiff, parsePatchTarget, applySimpleUnifiedDiff } from './diff.js';
export { extractSseEvents, sseEventData, splitAssistantStreamSegments } from './sse.js';
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
} from './storage.js';
export { cleanOutlookCalendarName, isVisibleOutlookCalendar, outlookCalendarLabel } from './calendar.js';
export { sanitizeTextForTts } from './tts.js';
export { copyTextToClipboard } from './clipboard.js';
export { fitTextareaToContent, isFatalUiError, importPhaseLabel, sessionUpdatedAtMs, formatSessionLastAccessed } from './ui.js';
export { parseWelcomeMessages, randomWelcomeMessage, profileDisplayName, personalizeWelcomeMessage } from './profile.js';
export { turnsToMessages, mergeIndexedDocuments } from './sessions.js';
