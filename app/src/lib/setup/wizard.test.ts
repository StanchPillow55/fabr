import { describe, expect, it } from 'vitest';
import { canGoBack, firstIncompleteStep, markStep, next, prev, stepIndex } from './wizard';
import { emptySettings } from './setup-source';

const withSteps = (steps: string[]) => ({ ...emptySettings(), setup: { completed_steps: steps, completed: false } });

describe('wizard', () => {
  it('opens the first incomplete step', () => {
    expect(firstIncompleteStep(emptySettings())).toBe('folder');
    expect(firstIncompleteStep(withSteps(['folder', 'api_key']))).toBe('model');
    expect(firstIncompleteStep(withSteps(['folder', 'model']))).toBe('api_key');
    expect(firstIncompleteStep(withSteps(['folder', 'api_key', 'model', 'telegram_bot', 'telegram_link']))).toBe('done');
  });
  it('navigates', () => {
    expect(canGoBack('folder')).toBe(false);
    expect(canGoBack('api_key')).toBe(true);
    expect(next('folder')).toBe('api_key');
    expect(next('done')).toBe('done');
    expect(prev('folder')).toBe('folder');
    expect(prev('done')).toBe('telegram_link');
    expect(stepIndex('done')).toBe(5);
  });
  it('completes after the fifth step only', () => {
    let s = emptySettings();
    for (const id of ['folder', 'api_key', 'model', 'telegram_bot'] as const) s = markStep(s, id);
    expect(s.setup.completed).toBe(false);
    s = markStep(markStep(s, 'telegram_link'), 'telegram_link');
    expect(s.setup.completed).toBe(true);
    expect(s.setup.completed_steps).toHaveLength(5);
  });
});
