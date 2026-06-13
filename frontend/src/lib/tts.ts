// Text-to-Speech sanitization utilities

export function sanitizeTextForTts(rawText: string): string {
  let cleanText = rawText.replace(/```[\s\S]*?```/g, '');
  cleanText = cleanText.replace(/`([^`]+)`/g, '$1');
  cleanText = cleanText.replace(/[*#_~>+\-]/g, '');
  cleanText = cleanText.replace(/\s+/g, ' ').trim();
  return cleanText;
}
