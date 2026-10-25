import type { FabricSettings } from '../api/types';

export type StepId = 'folder' | 'api_key' | 'model' | 'telegram_bot' | 'telegram_link' | 'done';

export const STEPS: StepId[] = ['folder', 'api_key', 'model', 'telegram_bot', 'telegram_link', 'done'];
const WORK_STEPS = STEPS.slice(0, 5);

export const MODELS = [
  { value: 'claude-sonnet-5-5', label: 'Sonnet (suggested)' },
  { value: 'claude-haiku-5-5', label: 'Haiku' },
];

export const stepIndex = (s: StepId) => STEPS.indexOf(s);
export const canGoBack = (s: StepId) => stepIndex(s) > 0;
export const prev = (s: StepId): StepId => STEPS[Math.max(0, stepIndex(s) - 1)];
export const next = (s: StepId): StepId => STEPS[Math.min(STEPS.length - 1, stepIndex(s) + 1)];

export function firstIncompleteStep(settings: FabricSettings): StepId {
  const done = settings.setup?.completed_steps ?? [];
  return WORK_STEPS.find((s) => !done.includes(s)) ?? 'done';
}

export function markStep(settings: FabricSettings, step: StepId): FabricSettings {
  const have = settings.setup?.completed_steps ?? [];
  const steps = have.includes(step) ? have : [...have, step];
  const completed = WORK_STEPS.every((s) => steps.includes(s));
  return { ...settings, setup: { completed_steps: steps, completed } };
}
