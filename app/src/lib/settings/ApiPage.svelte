<script lang="ts">
  import { onMount } from 'svelte';
  import Group from '../ui/Group.svelte';
  import NumField from '../ui/NumField.svelte';
  import Row from '../ui/Row.svelte';
  import Segmented from '../ui/Segmented.svelte';
  import Switch from '../ui/Switch.svelte';
  import { MODELS } from '../setup/wizard';
  import type { ExecuteMode } from '../api/types';
  import type { SettingsController } from './controller.svelte';

  let { ctl }: { ctl: SettingsController } = $props();
  const p = $derived(ctl.setup);
  let secret = $state('');
  let model = $state(MODELS[0].value);
  let auto = $state(false);
  let execute = $state<ExecuteMode>('full');
  const MODES: { value: ExecuteMode; label: string }[] = [{ value: 'full', label: 'Full' }, { value: 'read_only', label: 'Read-only' }, { value: 'off', label: 'Off' }];
  const HELP: Record<ExecuteMode, string> = { full: 'This machine runs tasks to the end, with tools and checks.', read_only: 'This machine defines, plans and gates tasks but never executes them.', off: 'The agent does not run on this machine.' };

  onMount(async () => {
    await p.load();
    model = p.settings?.model ?? MODELS[0].value;
    auto = p.settings?.auto_mode ?? false;
    execute = p.settings?.execute ?? 'full';
  });

  async function save() {
    if (await p.saveSecret('anthropic_api_key', secret)) secret = '';
  }
</script>

<Group header="Anthropic key" footer="Stored on this computer. Only the start and end of the key are shown.">
  <Row label="Saved key" value={p.keySaved ? p.keyFingerprint : 'none'} />
  <Row label="Replace with">
    <input class="row-input" type="password" bind:value={secret} placeholder="sk-ant-..." autocomplete="off" spellcheck="false" />
  </Row>
  <Row label={p.busy === 'save' ? 'Saving…' : 'Save key'} tone="tint" disabled={!secret.trim() || p.busy !== null} onclick={save} />
  <Row label={p.busy === 'key' ? 'Testing…' : 'Test key'} tone="tint" disabled={!p.keySaved || p.busy !== null} onclick={() => p.testKey()} />
  {#if p.results.key}
    <Row label="Result" wrap>
      <span class="row-result" class:row-result--bad={!p.results.key.ok}><span class="row-dot" class:row-dot--ok={p.results.key.ok} class:row-dot--error={!p.results.key.ok}></span>{p.results.key.text}</span>
    </Row>
  {/if}
</Group>

<Group header="Model">
  <Row label="Model"><Segmented label="Model" options={MODELS} bind:value={model} onchange={(m) => p.write({ model: m })} /></Row>
  <Row label={p.busy === 'model' ? 'Testing…' : 'Test model'} tone="tint" disabled={!p.keySaved || p.busy !== null} onclick={() => p.testModel(model)} />
  {#if p.results.model}
    <Row label="Result" wrap>
      <span class="row-result" class:row-result--bad={!p.results.model.ok}><span class="row-dot" class:row-dot--ok={p.results.model.ok} class:row-dot--error={!p.results.model.ok}></span>{p.results.model.text}</span>
    </Row>
  {/if}
</Group>

<Group header="Agent" footer={p.runnerError ?? 'The agent asks at most this many questions per task.'} error={!!p.runnerError}>
  <Row label="Auto mode"><Switch label="Auto mode" bind:checked={auto} onchange={(v) => p.write({ auto_mode: v })} /></Row>
  <NumField label="Question budget" mode="numeric" bad={!!p.runnerError} bind:value={p.budgetText} oncommit={() => p.saveRunner()} />
  <NumField label="Heartbeat" unit="seconds" mode="numeric" bad={!!p.runnerError} bind:value={p.heartbeatText} oncommit={() => p.saveRunner()} />
</Group>

<Group header="Machine" footer="{HELP[execute]} The runner detects tools and checks and refreshes them when it starts.">
  <Row label="Execute"><Segmented label="Execute" options={MODES} bind:value={execute} onchange={(m) => p.write({ execute: m })} /></Row>
  <Row label="Tools" wrap>
    <span class="chips" data-testid="cap-tools">{#each p.settings?.capabilities.tools ?? [] as t (t)}<span class="chip">{t}</span>{:else}<span class="chip chip--none">none detected</span>{/each}</span>
  </Row>
  <Row label="Checks" wrap>
    <span class="chips" data-testid="cap-checks">{#each p.settings?.capabilities.checks ?? [] as c (c)}<span class="chip">{c}</span>{:else}<span class="chip chip--none">none detected</span>{/each}</span>
  </Row>
</Group>

{#if p.error}<div class="banner banner--error page-panel" role="alert">{p.error}</div>{/if}
