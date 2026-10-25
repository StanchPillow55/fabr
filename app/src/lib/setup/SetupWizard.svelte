<script lang="ts">
  import { onDestroy, onMount, untrack } from 'svelte';
  import { errText, type FabricCheckResult, type FabricSecretsStatus, type FabricSettings } from '../api/types';
  import Button from '../ui/Button.svelte';
  import Group from '../ui/Group.svelte';
  import Page from '../ui/Page.svelte';
  import Row from '../ui/Row.svelte';
  import Segmented from '../ui/Segmented.svelte';
  import { createSetupSource, type SetupSource } from './setup-source';
  import { canGoBack, firstIncompleteStep, markStep, MODELS, next, prev, stepIndex, STEPS, type StepId } from './wizard';

  let { source, initial, ondone }: { source?: SetupSource; initial?: FabricSettings; ondone?: () => void } = $props();

  const TITLES: Record<StepId, string> = {
    folder: 'Default folder', api_key: 'Anthropic API key', model: 'Model',
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
  let model = $state(seed?.model || MODELS[0].value);
  let token = $state('');
  let botName = $state<string | null>(null);
  let waiting = $state(false);
  let elapsed = $state(0);
  let timer: ReturnType<typeof setInterval> | undefined;

  const stepNo = $derived(stepIndex(step) + 1);

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
        model = r.settings.model || MODELS[0].value;
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

  const checkFolder = () => run(() => src!.check('folder', { path: folder.trim() }), () => persist({ default_folder: folder.trim() }, 'folder'));

  const checkKey = () => run(async () => {
    secrets = await src!.writeSecrets({ anthropic_api_key: apiKey });
    return src!.check('api_key');
  }, async () => {
    apiKey = '';
    secrets = await src!.secretsStatus();
    await persist({}, 'api_key');
  });

  const checkModel = () => run(async () => {
    await persist({ model });
    return src!.check('model', { model });
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
    <p class="page-group__foot">Step {stepNo} of {STEPS.length}</p>

    {#if step === 'folder'}
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
      <Group header="Choose a model" footer={reason ?? info ?? 'Sonnet is suggested.'} error={!!reason}>
        <Row label="Model"><Segmented label="Model" options={MODELS} bind:value={model} onchange={() => (passed = false)} /></Row>
      </Group>
      <Button variant="primary" disabled={busy} onclick={checkModel}>{busy ? 'Checking' : 'Check'}</Button>
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
        <Row label="Folder" value={settings.default_folder} />
        <Row label="API key" value={secrets.anthropic_api_key ? 'saved' : 'missing'} />
        <Row label="Model" value={settings.model} />
        <Row label="Bot" value={botName ?? (secrets.telegram_bot_token ? 'saved' : 'missing')} />
        <Row label="Telegram user id" value={String(settings.telegram_user_id ?? '')} />
      </Group>
      <Button variant="primary" onclick={() => ondone?.()}>Open tasks</Button>
    {/if}

    <div class="setup-nav">
      {#if canGoBack(step)}<Button variant="plain" disabled={busy} onclick={() => go(prev(step))}>Back</Button>{/if}
      {#if step !== 'done'}<Button variant="primary" disabled={!passed || busy} onclick={() => go(next(step))}>Next</Button>{/if}
    </div>
  {/if}
</Page>

<style>
  .setup-nav { display: flex; gap: 8px; justify-content: space-between; margin: 16px; }
</style>
