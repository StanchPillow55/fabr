import { describe, expect, it, vi } from 'vitest';
import { createMockApi } from '../../mock/api';
import type { Source } from '../api/types';
import { SshFlow } from './sshflow.svelte';
import sourceForm from './SourcePage.svelte?raw';

const ssh = (host: string, id = 'box'): Source => ({ id, name: 'Box', host, root: '~/work', prune: null });

function setup(host: string, id?: string) {
  const api = createMockApi({ latency: 0 });
  let source = ssh(host, id);
  const flow = new SshFlow(api, () => source);
  return { api, flow, set: (s: Source) => (source = s) };
}

describe('SshFlow (SourceForm connection state machine)', () => {
  it('unknown host: shows the keys, Trust this host appends them and re-tests', async () => {
    const { api, flow } = setup('newbox');
    const trust = vi.spyOn(api, 'trustHost');
    const test = vi.spyOn(api, 'testSource');
    await flow.test();
    expect(flow.phase).toBe('host_unknown');
    expect(flow.result?.message).toMatch(/^Unknown host key/);
    expect(flow.hostKeys?.map((k) => k.type)).toEqual(['ssh-ed25519', 'ecdsa-sha2-nistp256']);
    expect(flow.hostKeys?.[0].fingerprint).toMatch(/^SHA256:/);
    await flow.trust();
    expect(trust).toHaveBeenCalledWith('newbox', expect.any(Array));
    expect(trust.mock.calls[0][1]).toHaveLength(2);
    expect(test).toHaveBeenCalledTimes(2);
    expect(flow.phase).toBe('ok');
    expect(flow.hostKeys).toBeNull();
  });

  it('trusting clears the failure at once, reloads via onTrusted, and a new flow starts clean', async () => {
    const api = createMockApi({ latency: 0 });
    const order: string[] = [];
    const flow = new SshFlow(api, () => ssh('newbox'), () => { order.push(`trusted:${flow.result === null}`); });
    await flow.test();
    expect(flow.result?.failure_kind).toBe('host_key_unknown');
    await flow.trust();
    expect(order).toEqual(['trusted:true']);
    expect(flow.result?.ok).toBe(true);
    expect(flow.result?.message).not.toMatch(/Unknown host key/);
    expect(flow.hostKeyError).toBeNull();
    expect(new SshFlow(api, () => ssh('newbox')).result).toBeNull();
    expect(new SshFlow(api, () => ssh('newbox')).phase).toBe('idle');
  });

  it('changed host key: red warning, no override offered, trust refused', async () => {
    const { flow } = setup('changedbox');
    await flow.test();
    expect(flow.phase).toBe('host_changed');
    expect(flow.hostKeys).toBeNull();
    await flow.trust();
    expect(flow.phase).toBe('host_changed');
    expect(sourceForm).toContain('ssh-keygen -R {d.src.host}');
    const panel = sourceForm.slice(sourceForm.indexOf("phase === 'host_changed'"), sourceForm.indexOf('{/if}', sourceForm.indexOf("phase === 'host_changed'")));
    expect(panel).not.toContain('<Button');
  });

  it('auth failure focuses the password field; a transient password is sent with Test only', async () => {
    const { api, flow } = setup('pwbox');
    const test = vi.spyOn(api, 'testSource');
    await flow.test();
    expect(flow.phase).toBe('auth_failed');
    expect(flow.focusTick).toBe(1);
    expect(flow.result?.message).toBe('Authentication failed: the host did not accept your key or password');
    expect(flow.result?.detail).toContain('Permission denied');
    expect(flow.canInstall).toBe(false);
    flow.password = 'secret';
    await flow.test();
    expect(test.mock.calls[1][0].password).toBe('secret');
    expect(flow.phase).toBe('ok');
    expect(flow.usedPassword).toBe(true);
    expect(flow.canInstall).toBe(true);
    expect(await api.hasSourcePassword('box')).toEqual({ stored: false });
  });

  it('save stores the password in the keychain and clears the draft; Clear deletes it', async () => {
    const { api, flow } = setup('pwbox');
    flow.password = 'secret';
    await flow.savePassword();
    expect(flow.password).toBe('');
    expect(flow.stored).toBe(true);
    const fresh = new SshFlow(api, () => ssh('pwbox'));
    await fresh.loadStored();
    expect(fresh.stored).toBe(true);
    expect(fresh.canInstall).toBe(true);
    await fresh.test();
    expect(fresh.phase).toBe('ok');
    expect(fresh.usedPassword).toBe(true);
    await fresh.clearPassword();
    expect(fresh.stored).toBe(false);
    expect(await api.hasSourcePassword('box')).toEqual({ stored: false });
  });

  it('install my key: then key auth works without a password', async () => {
    const { api, flow } = setup('pwbox');
    flow.password = 'secret';
    await flow.test();
    await flow.installKey();
    expect(flow.install?.installed).toBe(true);
    expect(flow.canInstall).toBe(false);
    await flow.clearPassword();
    const again = new SshFlow(api, () => ssh('pwbox'));
    await again.test();
    expect(again.phase).toBe('ok');
    expect(again.usedPassword).toBe(false);
    expect(again.canInstall).toBe(false);
  });

  it('connect failures are classified; local sources never send a password', async () => {
    const { api, flow, set } = setup('badhost');
    await flow.test();
    expect(flow.phase).toBe('failed');
    expect(flow.kind).toBe('connect_failed');
    expect(flow.result?.message).toMatch(/^Could not connect: /);
    const test = vi.spyOn(api, 'testSource');
    set({ id: 'local', name: 'Home', host: null, root: '~/fabr-sample', prune: null });
    flow.password = 'leftover';
    await flow.test();
    expect(test.mock.calls[0][0]).not.toHaveProperty('password');
    expect(flow.isSsh).toBe(false);
    expect(flow.canInstall).toBe(false);
  });
});

describe('SshFlow install state follows the host', () => {
  it('a key installed on one host is not claimed for another', async () => {
    const { flow, set } = setup('pwbox');
    flow.password = 'secret';
    await flow.test();
    await flow.installKey();
    expect(flow.installedHere).toBe(true);
    set(ssh('pwother'));
    expect(flow.installedHere).toBe(false);
    await flow.test();
    expect(flow.install).toBeNull();
    expect(flow.canInstall).toBe(true);
  });

  it('login required (Tailscale check mode): shows the link, no host-key or password step, Test stays usable', async () => {
    const { api, flow } = setup('tailbox');
    const msg = 'Log in to Tailscale to reach tailbox: https://login.tailscale.com/a/abc';
    const hostKey = vi.spyOn(api, 'hostKey');
    vi.spyOn(api, 'testSource').mockResolvedValue({ ok: false, message: msg, latency_ms: 30000, node_count: 0, failure_kind: 'login_required' });
    await flow.test();
    expect(flow.phase).toBe('login_required');
    expect(flow.result?.message).toBe(msg);
    expect(flow.hostKeys).toBeNull();
    expect(hostKey).not.toHaveBeenCalled();
    expect(flow.testing).toBe(false);
    expect(sourceForm).toContain("failure_kind === 'login_required'");
  });
});
