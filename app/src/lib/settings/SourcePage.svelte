<script lang="ts">
  import { rootState } from '../addnodes';
  import { hasNativePicker, pickFolder } from '../api/dialog';
  import { errText } from '../api/types';
  import type { Store } from '../store.svelte';
  import Button from '../ui/Button.svelte';
  import ErrorText from '../ui/ErrorText.svelte';
  import Group from '../ui/Group.svelte';
  import Icon from '../ui/Icon.svelte';
  import NumField from '../ui/NumField.svelte';
  import Row from '../ui/Row.svelte';
  import Switch from '../ui/Switch.svelte';
  import type { SettingsController, Draft } from './controller.svelte';

  let { store, ctl, d }: { store: Store; ctl: SettingsController; d: Draft } = $props();

  const flow = $derived(d.flow);
  const result = $derived(flow.result);
  const root = $derived(rootState(result));
  const phase = $derived(flow.phase);
  const native = hasNativePicker();
  const defaultDepth = $derived(store.settings.search_depth ?? 4);
  let confirming = $state(false);
  let pwInput = $state<HTMLInputElement | null>(null);
  let seenTick = 0;

  $effect(() => {
    const t = flow.focusTick;
    if (t !== seenTick) {
      seenTick = t;
      pwInput?.focus();
    }
  });

  async function choose() {
    try {
      const dir = await pickFolder();
      if (dir) d.src.root = dir;
    } catch (e) {
      ctl.error = errText(e);
    }
  }
  const resultText = $derived(
    result?.ok
      ? `${result.latency_ms} ms · ${result.node_count} nodes${root === 'managed' ? ' · managed root' : root === 'plain' ? ' · root not initialised' : ''}`
      : '',
  );
</script>

<Group header="Connection" footer={ctl.errors.root ?? ctl.errors.id} error={!!(ctl.errors.root || ctl.errors.id)}>
  <Row label="Name"><input class="row-input" type="text" bind:value={d.src.name} placeholder="Home" autocomplete="off" /></Row>
  <Row label="Host">
    <input class="row-input" type="text" value={d.src.host ?? ''} spellcheck="false" autocomplete="off" placeholder="this machine"
      oninput={(e) => (d.src.host = e.currentTarget.value.trim() || null)} />
  </Row>
  <Row label="Root">
    <input class="row-input" class:row-input--bad={!!ctl.errors.root} type="text" bind:value={d.src.root} spellcheck="false" placeholder="~/workspace" />
    {#if native}<Button variant="icon" title="Choose a folder…" aria-label="Choose a folder" onclick={choose}><Icon name="folder" size={16} /></Button>{/if}
  </Row>
  {#if d.src.host}
    <Row label="Password">
      <input class="row-input" type="password" bind:this={pwInput} bind:value={flow.password} autocomplete="off"
        placeholder={flow.stored ? '•••• stored' : 'keychain'} class:row-input--bad={phase === 'auth_failed'} />
      {#if flow.stored || flow.password}<Button variant="tint" onclick={() => flow.clearPassword()}>Clear</Button>{/if}
    </Row>
  {/if}
  <Row label="Prune">
    <input class="row-input" type="text" bind:value={d.pruneText} spellcheck="false" placeholder="node_modules .git" />
  </Row>
  <Row label={flow.testing ? 'Testing…' : 'Test connection'} tone="tint" disabled={flow.testing || flow.trusting || !d.src.root.trim()} onclick={() => flow.test()} />
  {#if result?.ok}
    <Row label="Result"><span class="row-result"><span class="row-dot row-dot--ok"></span>{resultText}</span></Row>
  {:else if result}
    <Row label="Result" wrap><span class="row-result row-result--bad"><span class="row-dot row-dot--error"></span><ErrorText message={result.message} onopen={(u) => store.openUrl(u)} /></span></Row>
    {#if result.failure_kind === 'login_required'}
      <Row label="Waiting for login…" wrap><span class="row-result">Finish the login in your browser, then press Test connection again. The tree also keeps waiting for it in the background.</span></Row>
    {/if}
  {:else if flow.error}
    <Row label="Result" wrap><span class="row-result row-result--bad"><span class="row-dot row-dot--error"></span>{flow.error}</span></Row>
  {/if}
  {#if root === 'plain'}
    <Row label="Initialize root"><Switch label="Initialize root on save" bind:checked={d.initRoot} /></Row>
  {/if}
  {#if flow.installedHere}
    <Row label="Key installed" value="you can clear the password" />
  {:else if flow.canInstall}
    <Row label={flow.installing ? 'Installing…' : 'Install my key on this host'} tone="tint" disabled={flow.installing} onclick={() => flow.installKey()} />
    {#if flow.install && flow.installHost === d.src.host}
      <Row wrap><span class="row-result row-result--bad">{flow.install.message}</span></Row>
    {:else if flow.installError}
      <Row wrap><span class="row-result row-result--bad">{flow.installError}</span></Row>
    {/if}
  {/if}
</Group>

{#if result && !result.ok && result.detail}
  <details class="page-note ssh-detail"><summary>ssh said</summary><code>{result.detail}</code></details>
{/if}

{#if phase === 'host_unknown'}
  <div class="banner page-panel" role="region" aria-label="Unknown host key">
    <Icon name="warning" size={14} />
    <div class="banner__body">
      <p class="banner__text"><strong>This host isn't in your known_hosts yet.</strong> Check that the fingerprint matches the one the machine's owner gives you (on the host: <code>ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub</code>).</p>
      {#if flow.hostKeys}
        <ul class="hostkeys">
          {#each flow.hostKeys as k (k.line)}
            <li><span class="hostkeys__type">{k.type}</span><code class="num">{k.fingerprint}</code></li>
          {/each}
        </ul>
      {:else if flow.hostKeyError}
        <p class="banner__text">Could not fetch the host's keys: {flow.hostKeyError}</p>
      {:else}
        <p class="banner__text">Fetching the host's keys…</p>
      {/if}
      <div class="banner__actions">
        <Button variant="tint" disabled={!flow.hostKeys?.length || flow.trusting} onclick={() => flow.trust()}>{flow.trusting ? 'Trusting…' : 'Trust this host'}</Button>
      </div>
    </div>
  </div>
{:else if phase === 'host_changed'}
  <div class="banner banner--error page-panel" role="alert">
    <Icon name="warning" size={14} />
    <div class="banner__body">
      <p class="banner__text"><strong>The host key of {d.src.host} has changed.</strong> The machine may have been reinstalled, or someone may be intercepting the connection. Fabr will not override a known key.</p>
      <p class="banner__text">If you know why it changed, remove the old key in Terminal with <code>ssh-keygen -R {d.src.host}</code>, then test again.</p>
    </div>
  </div>
{/if}

<Group header="Scanning" footer={ctl.errors.depth ?? `Blank uses the default, ${defaultDepth} (Settings › Advanced).`} error={!!ctl.errors.depth}>
  <NumField label="Look for nodes" unit="levels deep" mode="numeric" bad={!!ctl.errors.depth} bind:value={d.depthText} placeholder={String(defaultDepth)}
    aria="Look for nodes N levels deep" />
</Group>

{#if ctl.error && !ctl.errors.root && !ctl.errors.depth && !ctl.errors.id}
  <div class="banner banner--error page-panel" role="alert"><Icon name="warning" size={14} /> <span>{ctl.error}</span></div>
{/if}

<Group footer={ctl.canDelete ? undefined : 'At least one source is required.'}>
  {#if confirming}
    <Row label="Delete {d.title}?" tone="danger" onclick={async () => { confirming = false; await ctl.deleteDraft(); }} />
    <Row label="Cancel" tone="tint" onclick={() => (confirming = false)} />
  {:else}
    <Row label="Delete source" tone="danger" disabled={!ctl.canDelete} onclick={() => (confirming = true)} />
  {/if}
</Group>
