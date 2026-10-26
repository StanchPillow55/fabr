import { describe, expect, it } from 'vitest';
import { render } from 'svelte/server';
import SetupWizard from './SetupWizard.svelte';
import { emptySettings, StubSetupSource } from './setup-source';
import { firstIncompleteStep, type StepId } from './wizard';

// Anthropic keeps every step, so it exercises all seven headings.
const ORDER = ['provider', 'folder', 'api_key', 'model', 'telegram_bot', 'telegram_link'];
const HEADINGS: [StepId, string][] = [
  ['provider', 'Provider'], ['folder', 'Default folder'], ['api_key', 'Anthropic API key'], ['model', 'Model'],
  ['telegram_bot', 'Telegram bot'], ['telegram_link', 'Telegram link'], ['done', 'All set'],
];

function settingsAt(n: number) {
  const s = emptySettings();
  s.provider = 'anthropic';
  s.setup = { completed_steps: ORDER.slice(0, n), completed: n === 6 };
  return s;
}

describe('SetupWizard render', () => {
  HEADINGS.forEach(([id, heading], n) => {
    it(`step ${n + 1} shows ${heading}`, () => {
      const initial = settingsAt(n);
      expect(firstIncompleteStep(initial)).toBe(id);
      const { body } = render(SetupWizard, { props: { source: new StubSetupSource({ settings: initial }), initial } });
      expect(body).toContain(heading);
      expect(body).toContain(`Step ${n + 1} of 7`);
      expect(body.includes('>Back<')).toBe(n > 0);
    });
  });
  it('offers Open on the last screen', () => {
    const { body } = render(SetupWizard, { props: { initial: settingsAt(6), source: new StubSetupSource({ settings: settingsAt(6) }) } });
    expect(body).toContain('>Open<');
  });
  it('local setup skips the api key and shows six steps', () => {
    const s = emptySettings();
    s.setup = { completed_steps: ['provider'], completed: false };
    const { body } = render(SetupWizard, { props: { source: new StubSetupSource({ settings: s }), initial: s } });
    expect(firstIncompleteStep(s)).toBe('folder');
    expect(body).toContain('Step 2 of 6');
  });
});

describe('stub source behaviour', () => {
  it('a scripted folder failure leaves the step incomplete and carries the reason', async () => {
    const { markStep } = await import('./wizard');
    const stub = new StubSetupSource({ outcomes: { folder: { ok: false, reason: 'Folder is not writable.' } } });
    await stub.writeSettings(markStep((await stub.readSettings()).settings, 'provider'));
    const r = await stub.check('folder', { path: '~/x' });
    expect(r).toEqual({ ok: false, reason: 'Folder is not writable.' });
    expect(firstIncompleteStep((await stub.readSettings()).settings)).toBe('folder');
  });
  it('a success marks the step and advances (local skips api_key)', async () => {
    const stub = new StubSetupSource();
    const { markStep } = await import('./wizard');
    let s = markStep((await stub.readSettings()).settings, 'provider');
    const r = await stub.check('folder', { path: '~/' });
    expect(r.ok).toBe(true);
    s = markStep({ ...s, default_folder: '~/' }, 'folder');
    await stub.writeSettings(s);
    expect(firstIncompleteStep((await stub.readSettings()).settings)).toBe('model');
  });
});
