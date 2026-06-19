// Profile and welcome message utilities
import { DEFAULT_WELCOME_MESSAGES } from '../constants';

export function parseWelcomeMessages(markdown: string) {
  const messages = markdown
    .split(/\r?\n/)
    .map((line) => line.trim().replace(/^[-*]\s+/, ''))
    .filter((line) => line && !line.startsWith('#'));
  return messages.length > 0 ? messages : DEFAULT_WELCOME_MESSAGES;
}

export function randomWelcomeMessage(messages: string[]) {
  const index = Math.floor(Math.random() * Math.max(messages.length, 1));
  return messages[index] ?? DEFAULT_WELCOME_MESSAGES[0];
}

export function profileDisplayName(profileText: string, lang = 'en') {
  const patterns = lang === 'tr'
    ? /\b(?:benim adım|adım|ben)\s+([A-Za-zÇçĞğİıÖöŞşÜü][A-Za-zÇçĞğİıÖöŞşÜü '-]{0,40})/i
    : /\b(?:my name is|name is|i am|i'm)\s+([A-Za-z][A-Za-z '-]{0,40})/i;
  const match = profileText.match(patterns);
  const rawName = match?.[1]?.trim().replace(/[.!,;:].*$/, '');
  return rawName || (lang === 'tr' ? 'orada' : 'there');
}

export function personalizeWelcomeMessage(message: string, profileText: string, lang = 'en') {
  return message.replace(/\[insert_name\]/gi, profileDisplayName(profileText, lang));
}
