export function modelDownloadPercent(chunk) {
    if (chunk.total && chunk.total > 0 && typeof chunk.completed === 'number') {
        return Math.max(0, Math.min(100, Math.round((chunk.completed / chunk.total) * 100)));
    }
    if (chunk.status === 'success') {
        return 100;
    }
    return null;
}
export function modelSearchPlaceholder(providerName) {
    return providerName === 'lmstudio'
        ? 'Enter an LM Studio catalog ID or Hugging Face URL'
        : 'Search catalog or enter an exact model tag';
}
export function installedModelsLabel(providerName) {
    return `Installed ${providerName === 'lmstudio' ? 'LM Studio' : 'Ollama'} Models`;
}
export function modelReadyMessage(modelName, providerName) {
    return `${modelName} is ready for ${providerName || 'active provider'}.`;
}
