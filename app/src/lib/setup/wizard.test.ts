import { describe, expect, it } from 'vitest';
import { canGoBack, firstIncompleteStep, markStep, next, prev, stepIndex } from './wizard';
import { emptySettings } from './setup-source';
import type { FabricSettings } from '../api/types';

const withSteps = (steps: string[]): FabricSettings => ({ ...emptySettings(), setup: { completed_steps: steps, completed: false } });

const anthropic = (steps: string[]): FabricSettings => ({ ...emptySettings(), provider: 'anthropic', setup: { completed_steps: steps, completed: false } });

describe('wizard', () => {
  it('opens the first incomplete step (local skips api_key)', () => {
    expect(firstIncompleteStep(emptySettings())).toBe('provider');
    expect(firstIncompleteStep(withSteps(['provider', 'folder']))).toBe('model');
    expect(firstIncompleteStep(withSteps(['provider', 'folder', 'model', 'telegram_bot', 'telegram_link']))).toBe('done');
  });
  it('opens the first incomplete step for anthropic (api_key required)', () => {
    expect(firstIncompleteStep(anthropic(['provider', 'folder']))).toBe('api_key');
    expect(firstIncompleteStep(anthropic(['provider', 'folder', 'api_key']))).toBe('model');
  });
  it('navigates', () => {
    expect(canGoBack('provider')).toBe(false);
    expect(canGoBack('folder')).toBe(true);
    expect(next('provider')).toBe('folder');
    expect(next('folder')).toBe('api_key');
    expect(next('done')).toBe('done');
    expect(prev('provider')).toBe('provider');
    expect(prev('done')).toBe('telegram_link');
    expect(stepIndex('done')).toBe(6);
  });
  it('local completes without an api key', () => {
    let s = emptySettings();
    for (const id of ['provider', 'folder', 'model', 'telegram_bot'] as const) s = markStep(s, id);
    expect(s.setup.completed).toBe(false);
    s = markStep(s, 'telegram_link');
    expect(s.setup.completed).toBe(true);
    expect(s.setup.completed_steps).toHaveLength(5);
  });
  it('anthropic requires the api key to complete', () => {
    let s = anthropic([]);
    for (const id of ['provider', 'folder', 'model', 'telegram_bot', 'telegram_link'] as const) s = markStep(s, id);
    expect(s.setup.completed).toBe(false);
    s = markStep(s, 'api_key');
    expect(s.setup.completed).toBe(true);
    expect(s.setup.completed_steps).toHaveLength(6);
  });
});
