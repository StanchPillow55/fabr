import { describe, expect, it } from 'vitest';
import { StubSetupSource } from '../setup/setup-source';
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
