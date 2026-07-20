// Settings and theme types

export type ThemeMode = 'dark' | 'light' | 'system';
export type AppearanceTheme = 'default' | 'terminal' | 'ocean' | 'ember' | 'rose' | 'slate';
export type SettingsTab = 'general' | 'models' | 'command-line' | 'personalize' | 'memories' | 'voice' | 'rag' | 'tools';
export type ResponseStyle = 'default' | 'friendly' | 'concise' | 'elaborate' | 'technical';

export interface CommandLineSettings {
  agentic_loop: boolean;
  repository_detection: boolean;
  repository_instructions: boolean;
  semantic_index: boolean;
  persistent_task_plan: boolean;
  task_checkpoints: boolean;
  context_budgeting: boolean;
  patch_application: boolean;
  command_execution: boolean;
  automatic_verification: boolean;
  deep_reasoning: boolean;
  git_safety: boolean;
}
