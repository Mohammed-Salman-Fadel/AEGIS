import test from 'node:test';
import assert from 'node:assert/strict';

import {
  installedModelsLabel,
  modelDownloadPercent,
  modelReadyMessage,
  normalizeModelDownloadName,
  modelSearchPlaceholder,
} from '../src/lib/modelDownload.js';

test('modelDownloadPercent calculates bounded progress from completed bytes', () => {
  assert.equal(modelDownloadPercent({ completed: 50, total: 200 }), 25);
  assert.equal(modelDownloadPercent({ completed: 250, total: 200 }), 100);
  assert.equal(modelDownloadPercent({ completed: -10, total: 200 }), 0);
});

test('modelDownloadPercent treats success as complete without byte totals', () => {
  assert.equal(modelDownloadPercent({ status: 'success' }), 100);
  assert.equal(modelDownloadPercent({ status: 'downloading' }), null);
  assert.equal(modelDownloadPercent({ completed: 10 }), null);
});

test('model search placeholder follows the active provider', () => {
  assert.equal(
    modelSearchPlaceholder('lmstudio'),
    'Enter an LM Studio catalog ID or Hugging Face URL',
  );
  assert.equal(modelSearchPlaceholder('ollama'), 'Search catalog or enter an exact model tag');
  assert.equal(modelSearchPlaceholder(undefined), 'Search catalog or enter an exact model tag');
});

test('installed models label follows the active provider', () => {
  assert.equal(installedModelsLabel('lmstudio'), 'Installed LM Studio Models');
  assert.equal(installedModelsLabel('ollama'), 'Installed Ollama Models');
  assert.equal(installedModelsLabel(undefined), 'Installed Ollama Models');
});

test('modelReadyMessage includes the provider used for the download', () => {
  assert.equal(modelReadyMessage('qwen3:4b', 'ollama'), 'qwen3:4b is ready for ollama.');
  assert.equal(modelReadyMessage('local/model', undefined), 'local/model is ready for active provider.');
});

test('normalizeModelDownloadName translates common LM Studio shorthand models', () => {
  assert.equal(
    normalizeModelDownloadName('llama3.1:8b', 'lmstudio'),
    'lmstudio-community/Llama-3.1-8B-Instruct-GGUF',
  );
  assert.equal(
    normalizeModelDownloadName('https://huggingface.co/acme/model', 'lmstudio'),
    'https://huggingface.co/acme/model',
  );
  assert.equal(normalizeModelDownloadName('llama3.1:8b', 'ollama'), 'llama3.1:8b');
});
