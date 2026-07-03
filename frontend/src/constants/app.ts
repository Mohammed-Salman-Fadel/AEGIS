// Application configuration constants

export const MAX_PROJECT_FILES = 120;
export const MAX_PROJECT_FILE_BYTES = 64 * 1024;
export const MAX_PROJECT_CONTEXT_CHARS = 32_000; // roughly 8K tokens for the system prompt

export const IGNORED_PROJECT_DIRECTORIES = new Set([
  '.git',
  '.next',
  '.svelte-kit',
  '.venv',
  'dist',
  'node_modules',
  'target',
  'vendor',
]);

export const IGNORED_PROJECT_FILES = new Set([
  'package-lock.json',
  'pnpm-lock.yaml',
  'yarn.lock',
  'Cargo.lock',
]);

export const CODE_PROJECT_EXTENSIONS = new Set([
  '.c', '.cpp', '.cs', '.css', '.go', '.h', '.html', '.java',
  '.js', '.json', '.jsx', '.md', '.py', '.rs', '.toml', '.ts',
  '.tsx', '.vue', '.yaml', '.yml',
]);

export const DEFAULT_WELCOME_MESSAGES = [
  'Welcome back [insert_name]!',
  'How may I assist you today?',
  'What should we build or explore next?',
  'Ready when you are.',
  'What would you like AEGIS to help with?',
];
