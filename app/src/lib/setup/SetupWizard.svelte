<script lang="ts">
  import { onDestroy, onMount, untrack } from 'svelte';
  import { errText, type FabricCheckResult, type FabricSecretsStatus, type FabricSettings } from '../api/types';
  import Button from '../ui/Button.svelte';
  import Group from '../ui/Group.svelte';
  import Page from '../ui/Page.svelte';
  import Row from '../ui/Row.svelte';
  import Segmented from '../ui/Segmented.svelte';
  import { createSetupSource, type SetupSource } from './setup-source';
  import {
    canGoBack, DEFAULT_LOCAL_BASE_URL, DEFAULT_LOCAL_MODEL, firstIncompleteStep, markStep, MODELS, next, prev,
    PROVIDERS, stepIndex, STEPS, workSteps, type StepId,
  } from './wizard';
  import type { Provider } from '../api/types';

  let { source, initial, ondone }: { source?: SetupSource; initial?: FabricSettings; ondone?: () => void } = $props();

  const TITLES: Record<StepId, string> = {
    provider: 'Provider', folder: 'Default folder', api_key: 'Anthropic API key', model: 'Model',
    telegram_bot: 'Telegram bot', telegram_link: 'Telegram link', done: 'All set',
  };

  const seed = untrack(() => initial);
  let src: SetupSource | null = null;
  let settings = $state<FabricSettings | null>(seed ?? null);
  let step = $state<StepId>(seed ? firstIncompleteStep(seed) : 'folder');
  let loadError = $state<string | null>(null);
  let secrets = $state<FabricSecretsStatus>({ anthropic_api_key: false, telegram_bot_token: false });
  let busy = $state(false);
  let passed = $state(false);
  let reason = $state<string | null>(null);
  let info = $state<string | null>(null);

  let folder = $state(seed?.default_folder || '~/');
  let apiKey = $state('');
  let provider = $state<Provider>(seed?.provider === 'anthropic' ? 'anthropic' : 'local');
  let localBaseUrl = $state(seed?.local_base_url || DEFAULT_LOCAL_BASE_URL);
  let model = $state(seed?.model || MODELS[0].value);
  let token = $state('');
  let botName = $state<string | null>(null);
  let waiting = $state(false);
  let elapsed = $state(0);
  let timer: ReturnType<typeof setInterval> | undefined;

  const isLocal = $derived(provider === 'local');
  // Local skips the api_key step, so count and navigate over the live work steps.
  const flow = $derived([...workSteps(provider), 'done'] as StepId[]);
  const stepNo = $derived(flow.indexOf(step) + 1);
  const skip = (s: StepId): StepId => (isLocal && s === 'api_key' ? skip(next(s)) : s);
  const skipBack = (s: StepId): StepId => (isLocal && s === 'api_key' ? skipBack(prev(s)) : s);

  function enter(s: StepId) {
    step = s;
    reason = null;
    info = null;
    passed = !!settings?.setup.completed_steps.includes(s);
  }

  onMount(async () => {
    try {
      src = source ?? (await createSetupSource(window.location.search));
      secrets = await src.secretsStatus();
      if (!settings) {
        const r = await src.readSettings();
        settings = r.settings;
        folder = r.settings.default_folder || '~/';
        provider = r.settings.provider === 'anthropic' ? 'anthropic' : 'local';
        localBaseUrl = r.settings.local_base_url || DEFAULT_LOCAL_BASE_URL;
        model = r.settings.model || (provider === 'local' ? DEFAULT_LOCAL_MODEL : MODELS[0].value);
        enter(firstIncompleteStep(r.settings));
      } else {
        enter(step);
      }
    } catch (e) {
      loadError = errText(e);
    }
  });
  onDestroy(() => clearInterval(timer));

  async function persist(patch: Partial<FabricSettings>, mark?: StepId) {
    let s = { ...settings!, ...patch };
    if (mark) s = markStep(s, mark);
    settings = (await src!.writeSettings(s)).settings;
  }

  async function run(work: () => Promise<FabricCheckResult>, onok: (r: Extract<FabricCheckResult, { ok: true }>) => Promise<void>) {
    busy = true;
    reason = null;
    info = null;
    passed = false;
    try {
      const r = await work();
      if (r.ok) { await onok(r); passed = true; } else reason = r.reason;
    } catch (e) {
      reason = errText(e);
    } finally {
      busy = false;
    }
  }

  function onProviderChange(p: Provider) {
    passed = false;
    // Keep the model sensible for the chosen provider's default.
    if (p === 'local' && MODELS.some((m) => m.value === model)) model = DEFAULT_LOCAL_MODEL;
    if (p === 'anthropic' && !MODELS.some((m) => m.value === model)) model = MODELS[0].value;
  }

  // Choosing a provider is local configuration, so it persists without a network check.
  const saveProvider = () => run(
    async () => { await persist({ provider, local_base_url: localBaseUrl.trim() || DEFAULT_LOCAL_BASE_URL }, 'provider'); return { ok: true }; },
    async () => {},
  );

  const checkFolder = () => run(() => src!.check('folder', { path: folder.trim() }), async () => { await persist({ default_folder: folder.trim() }, 'folder'); folder = settings!.default_folder; });

  const checkKey = () => run(async () => {
    secrets = await src!.writeSecrets({ anthropic_api_key: apiKey });
    return src!.check('api_key');
  }, async () => {
    apiKey = '';
    secrets = await src!.secretsStatus();
    await persist({}, 'api_key');
  });

  const checkModel = () => run(async () => {
    const name = isLocal ? model.trim() : model;
    await persist({ model: name });
    // Local models (Ollama / MLX) are not reachable through the Anthropic check.
    return isLocal ? { ok: true } : src!.check('model', { model: name });
  }, async (r) => {
    info = r.reply ?? null;
    await persist({}, 'model');
  });

  const checkToken = () => run(async () => {
    secrets = await src!.writeSecrets({ telegram_bot_token: token });
    return src!.check('telegram_token');
  }, async (r) => {
    token = '';
    botName = r.bot_name ?? null;
    info = botName;
    secrets = await src!.secretsStatus();
    await persist({}, 'telegram_bot');
  });

  async function startLink() {
    waiting = true;
    elapsed = 0;
    timer = setInterval(() => (elapsed += 1), 1000);
    await run(() => src!.telegramLink(120), async (r) => {
      await persist({ telegram_user_id: r.user_id ?? null }, 'telegram_link');
    });
    clearInterval(timer);
    waiting = false;
  }

  const go = (s: StepId) => enter(s);
</script>

<Page title={loadError === null ? TITLES[step] : 'Setup'}>
  {#if loadError !== null}
    <Group footer={loadError} error><Row label="Could not load settings" /></Group>
  {:else if !settings}
    <Group><Row label="Loading" /></Group>
  {:else}
    <p class="page-group__foot">Step {stepNo} of {flow.length}</p>

    {#if step === 'provider'}
      <Group header="Where should Fabr run?" footer={reason ?? (isLocal ? 'Runs on this computer with Ollama or MLX. No Anthropic key needed.' : 'Uses the Anthropic API. You will add a key next.')} error={!!reason}>
        <Row label="Provider"><Segmented label="Provider" options={PROVIDERS} bind:value={provider} onchange={onProviderChange} /></Row>
        {#if isLocal}
          <Row label="Base URL"><input class="row-input" type="text" bind:value={localBaseUrl} oninput={() => (passed = false)} placeholder={DEFAULT_LOCAL_BASE_URL} spellcheck="false" autocomplete="off" /></Row>
        {/if}
      </Group>
      <Button variant="primary" disabled={busy} onclick={saveProvider}>{busy ? 'Saving' : 'Continue'}</Button>
    {:else if step === 'folder'}
      <Group header="Where your projects live" footer={reason ?? 'The folder must exist and be writable.'} error={!!reason}>
        <Row label="Folder"><input class="row-input" type="text" bind:value={folder} oninput={() => (passed = false)} spellcheck="false" autocomplete="off" /></Row>
      </Group>
      <Button variant="primary" disabled={busy || !folder.trim()} onclick={checkFolder}>{busy ? 'Checking' : 'Check'}</Button>
    {:else if step === 'api_key'}
      <Group header="Paste your key" footer={reason ?? (secrets.anthropic_api_key && passed ? 'The key is saved and hidden.' : 'It is stored on this computer and never shown again.')} error={!!reason}>
        <Row label="API key"><input class="row-input" type="password" bind:value={apiKey} oninput={() => (passed = false)} placeholder={secrets.anthropic_api_key ? '••••••••  saved' : 'sk-ant-...'} autocomplete="off" spellcheck="false" /></Row>
      </Group>
      <Button variant="primary" disabled={busy || !apiKey} onclick={checkKey}>{busy ? 'Checking' : 'Check'}</Button>
    {:else if step === 'model'}
      <Group header="Choose a model" footer={reason ?? info ?? (isLocal ? 'The name of a model you have pulled locally, e.g. qwen2.5.' : 'Sonnet is suggested.')} error={!!reason}>
        {#if isLocal}
          <Row label="Model"><input class="row-input" type="text" bind:value={model} oninput={() => (passed = false)} placeholder={DEFAULT_LOCAL_MODEL} spellcheck="false" autocomplete="off" /></Row>
        {:else}
          <Row label="Model"><Segmented label="Model" options={MODELS} bind:value={model} onchange={() => (passed = false)} /></Row>
        {/if}
      </Group>
      <Button variant="primary" disabled={busy || (isLocal && !model.trim())} onclick={checkModel}>{busy ? 'Checking' : 'Check'}</Button>
    {:else if step === 'telegram_bot'}
      <Group header="Create the bot" footer={reason ?? (info ? `Bot: ${info}` : undefined)} error={!!reason}>
        <Row label="1. Open BotFather in Telegram" />
        <Row label="2. Send /newbot" />
        <Row label="3. Answer its two prompts, then paste the token" />
        <Row label="Token"><input class="row-input" type="password" bind:value={token} oninput={() => (passed = false)} placeholder={secrets.telegram_bot_token ? '••••••••  saved' : '123456:ABC...'} autocomplete="off" spellcheck="false" /></Row>
      </Group>
      <Button variant="primary" disabled={busy || !token} onclick={checkToken}>{busy ? 'Checking' : 'Check'}</Button>
    {:else if step === 'telegram_link'}
      <Group header="Link your account" footer={reason ?? (passed ? `Linked as user ${settings.telegram_user_id}` : undefined)} error={!!reason}>
        <Row label="Open the new bot in Telegram and send it any message, then tap Confirm on its reply." wrap />
        {#if waiting}<Row label="Waiting for your message" value={`${elapsed} s`} />{/if}
      </Group>
      <Button variant="primary" disabled={busy} onclick={startLink}>{reason ? 'Retry' : 'Start'}</Button>
    {:else}
      <Group header="Summary" footer="Secrets are saved and never shown.">
        <Row label="Provider" value={isLocal ? 'Local (Ollama/MLX)' : 'Anthropic (cloud)'} />
        {#if isLocal}<Row label="Base URL" value={settings.local_base_url} />{/if}
        <Row label="Folder" value={settings.default_folder} />
        {#if !isLocal}<Row label="API key" value={secrets.anthropic_api_key ? 'saved' : 'missing'} />{/if}
        <Row label="Model" value={settings.model} />
        <Row label="Bot" value={botName ?? (secrets.telegram_bot_token ? 'saved' : 'missing')} />
        <Row label="Telegram user id" value={String(settings.telegram_user_id ?? '')} />
      </Group>
      <Button variant="primary" onclick={() => ondone?.()}>Open</Button>
    {/if}

    <div class="setup-nav">
      {#if canGoBack(step)}<Button variant="plain" disabled={busy} onclick={() => go(skipBack(prev(step)))}>Back</Button>{/if}
      {#if step !== 'done'}<Button variant="primary" disabled={!passed || busy} onclick={() => go(skip(next(step)))}>Next</Button>{/if}
    </div>
  {/if}
</Page>

<style>
  .setup-nav { display: flex; gap: 8px; justify-content: space-between; margin: 16px; }
</style>
