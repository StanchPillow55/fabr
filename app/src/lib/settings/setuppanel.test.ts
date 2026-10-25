import { describe, expect, it } from 'vitest';
import { StubSetupSource, emptySettings } from '../setup/setup-source';
import { SetupPanel } from './setuppanel.svelte';

const FAKE_SECRET = 'sk-ant-FAKE-FIXTURE-0000-0000';

async function panel(opts: ConstructorParameters<typeof StubSetupSource>[0] = {}) {
  const src = new StubSetupSource(opts);
  const p = new SetupPanel(async () => src);
  await p.load();
  return { src, p };
}

describe('setup panel', () => {
  it('shows only a fingerprint after saving the API credential and tests it', async () => {
    const { p } = await panel();
    expect(p.keySaved).toBe(false);
    expect(await p.saveSecret('anthropic_api_key', FAKE_SECRET)).toBe(true);
    expect(p.keyFingerprint).toBe('sk-ant-…0000');
    expect(JSON.stringify(p.secrets)).not.toContain('FIXTURE');
    await p.testKey();
    expect(p.results.key?.ok).toBe(true);
  });

  it('reports a failed check', async () => {
    const { p } = await panel({ outcomes: { api_key: { ok: false, reason: 'Scripted failure.' } } });
    await p.saveSecret('anthropic_api_key', FAKE_SECRET);
    await p.testKey();
    expect(p.results.key).toEqual({ ok: false, text: 'Scripted failure.' });
  });

  it('links, then unlinks the telegram user', async () => {
    const { src, p } = await panel();
    await p.saveSecret('telegram_bot_token', '123456:FAKE-FIXTURE-TOKEN-0000');
    await p.link();
    expect(p.linkedId).toBe(123456789);
    expect(src.settings.telegram_user_id).toBe(123456789);
    expect(p.telegramReady).toBe(true);
    await p.unlink();
    expect(src.settings.telegram_user_id).toBeNull();
    expect(p.linkedId).toBeNull();
  });

  it('validates runner numbers before writing', async () => {
    const { src, p } = await panel();
    p.budgetText = 'x';
    expect(await p.saveRunner()).toBe(false);
    expect(p.runnerError).toBeTruthy();
    p.budgetText = '7';
    p.heartbeatText = '30';
    expect(await p.saveRunner()).toBe(true);
    expect(src.settings).toMatchObject({ question_budget: 7, heartbeat_interval_s: 30 });
  });
});

describe('machine settings', () => {
  it('writes execute through the settings source and keeps the detected capabilities', async () => {
    const { src, p } = await panel({ settings: { ...emptySettings(), capabilities: { tools: ['write_file'], checks: ['git'] } } });
    expect(await p.write({ execute: 'read_only' })).toBe(true);
    expect(src.settings.execute).toBe('read_only');
    expect(src.settings.capabilities).toEqual({ tools: ['write_file'], checks: ['git'] });
    expect(p.settings?.execute).toBe('read_only');
  });

  it('renders the three modes and the capability chips', async () => {
    const { render } = await import('svelte/server');
    const ApiPage = (await import('./ApiPage.svelte')).default;
    const { p } = await panel({ settings: { ...emptySettings(), capabilities: { tools: ['write_file'], checks: ['git'] } } });
    const body = render(ApiPage, { props: { ctl: { setup: p } as never } }).body;
    for (const t of ['Full', 'Read-only', 'Off', 'write_file', 'git', 'refreshes them when it starts']) expect(body).toContain(t);
  });
});
