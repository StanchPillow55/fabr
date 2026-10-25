import { describe, expect, it } from 'vitest';
import { render } from 'svelte/server';
import SetupWizard from './SetupWizard.svelte';
import { emptySettings, StubSetupSource } from './setup-source';
import { firstIncompleteStep, type StepId } from './wizard';

const ORDER = ['folder', 'api_key', 'model', 'telegram_bot', 'telegram_link'];
const HEADINGS: [StepId, string][] = [
  ['folder', 'Default folder'], ['api_key', 'Anthropic API key'], ['model', 'Model'],
  ['telegram_bot', 'Telegram bot'], ['telegram_link', 'Telegram link'], ['done', 'All set'],
];

function settingsAt(n: number) {
  const s = emptySettings();
  s.setup = { completed_steps: ORDER.slice(0, n), completed: n === 5 };
  return s;
}

describe('SetupWizard render', () => {
  HEADINGS.forEach(([id, heading], n) => {
    it(`step ${n + 1} shows ${heading}`, () => {
      const initial = settingsAt(n);
      expect(firstIncompleteStep(initial)).toBe(id);
      const { body } = render(SetupWizard, { props: { source: new StubSetupSource({ settings: initial }), initial } });
      expect(body).toContain(heading);
      expect(body).toContain(`Step ${n + 1} of 6`);
      expect(body.includes('>Back<')).toBe(n > 0);
    });
  });
  it('offers Open tasks on the last screen', () => {
    const { body } = render(SetupWizard, { props: { initial: settingsAt(5), source: new StubSetupSource({ settings: settingsAt(5) }) } });
    expect(body).toContain('Open tasks');
  });
});

describe('stub source behaviour', () => {
  it('a scripted folder failure leaves the step incomplete and carries the reason', async () => {
    const stub = new StubSetupSource({ outcomes: { folder: { ok: false, reason: 'Folder is not writable.' } } });
    const r = await stub.check('folder', { path: '~/x' });
    expect(r).toEqual({ ok: false, reason: 'Folder is not writable.' });
    expect(firstIncompleteStep((await stub.readSettings()).settings)).toBe('folder');
  });
  it('a success marks the step and advances', async () => {
    const stub = new StubSetupSource();
    const { markStep } = await import('./wizard');
    const r = await stub.check('folder', { path: '~/' });
    expect(r.ok).toBe(true);
    const s = (await stub.readSettings()).settings;
    await stub.writeSettings(markStep({ ...s, default_folder: '~/' }, 'folder'));
    expect(firstIncompleteStep((await stub.readSettings()).settings)).toBe('api_key');
  });
});
