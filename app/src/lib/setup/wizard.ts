import type { FabricSettings, Provider } from '../api/types';

export type StepId = 'provider' | 'folder' | 'api_key' | 'model' | 'telegram_bot' | 'telegram_link' | 'done';

export const STEPS: StepId[] = ['provider', 'folder', 'api_key', 'model', 'telegram_bot', 'telegram_link', 'done'];

export const DEFAULT_PROVIDER: Provider = 'local';
export const DEFAULT_LOCAL_BASE_URL = 'http://localhost:11434/v1';
export const DEFAULT_LOCAL_MODEL = 'qwen2.5';

export const PROVIDERS = [
  { value: 'local' as Provider, label: 'Local (Ollama/MLX)' },
  { value: 'anthropic' as Provider, label: 'Anthropic (cloud)' },
];

export const MODELS = [
  { value: 'claude-sonnet-5-5', label: 'Sonnet (suggested)' },
  { value: 'claude-haiku-5-5', label: 'Haiku' },
];

// The local provider (Ollama / MLX) runs without an Anthropic key, so the
// api_key step is dropped from its work steps; the cloud path keeps it.
export function workSteps(provider: Provider): StepId[] {
  const all: StepId[] = ['provider', 'folder', 'api_key', 'model', 'telegram_bot', 'telegram_link'];
  return provider === 'local' ? all.filter((s) => s !== 'api_key') : all;
}

const providerOf = (settings: FabricSettings): Provider => (settings.provider === 'anthropic' ? 'anthropic' : 'local');

export const stepIndex = (s: StepId) => STEPS.indexOf(s);
export const canGoBack = (s: StepId) => stepIndex(s) > 0;
export const prev = (s: StepId): StepId => STEPS[Math.max(0, stepIndex(s) - 1)];
export const next = (s: StepId): StepId => STEPS[Math.min(STEPS.length - 1, stepIndex(s) + 1)];

export function firstIncompleteStep(settings: FabricSettings): StepId {
  const done = settings.setup?.completed_steps ?? [];
  return workSteps(providerOf(settings)).find((s) => !done.includes(s)) ?? 'done';
}

export function markStep(settings: FabricSettings, step: StepId): FabricSettings {
  const have = settings.setup?.completed_steps ?? [];
  const steps = have.includes(step) ? have : [...have, step];
  const completed = workSteps(providerOf(settings)).every((s) => steps.includes(s));
  return { ...settings, setup: { completed_steps: steps, completed } };
}
