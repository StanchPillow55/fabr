<script lang="ts">
  import { onMount } from 'svelte';
  import Page from '../ui/Page.svelte';
  import Segmented from '../ui/Segmented.svelte';
  import Sheet from '../ui/Sheet.svelte';
  import AgentsScreen from './AgentsScreen.svelte';
  import type { RegistryEntry } from './agents';
  import BoardScreen from './BoardScreen.svelte';
  import { loadReadPosition, parseEvents, reconcileReadPosition, saveReadPosition, type FabricEvent } from './events';
  import NoticesBadge from './NoticesBadge.svelte';
  import NoticesList from './NoticesList.svelte';
  import { submitRequest, type RequestKey } from './requests';
  import { findByTaskPath, loadRegistry } from './view';
  import { folderName, folderOf, type Entry } from './board';
  import type { AnswerInput } from './answer';
  import { submitApproval, type ApprovalDecision } from './approval';
  import { parseRecord } from './parse';
  import RecordScreen from './RecordScreen.svelte';
  import { BACKEND_LABEL, backendState } from './backend';
  import { createFabricSource } from './live';
  import type { FabricSource } from './source';
  import TreeScreen from './TreeScreen.svelte';
  import { submitAnswer } from './write';

  let { onclose }: { onclose: () => void } = $props();

  type Held = Entry & { text: string };

  let source: FabricSource | null = null;
  let heartbeat = $state<string | null>(null);
  let entries = $state<Held[]>([]);
  let errors = $state<string[]>([]);
  let openPath = $state<string | null>(null);
  let view = $state<'board' | 'tree' | 'agents'>('board');
  let eventsKey = 'events';
  let registry = $state<RegistryEntry[]>([]);
  let unreadable = $state(0);
  let events = $state<FabricEvent[]>([]);
  let readPosition = $state(0);
  let showNotices = $state(false);
  let now = $state(new Date());
  let actionError = $state<string | null>(null);
  const backend = $derived(backendState(heartbeat, now));
  const open = $derived(entries.find((e) => e.path === openPath) ?? null);

  async function load() {
    if (!source) return;
    const raw = await source.listRecords();
    const ok: Held[] = [];
    const bad: string[] = [];
    for (const { path, text } of raw) {
      try { ok.push({ path, record: parseRecord(text), text }); }
      catch (e) { bad.push(`${folderName(folderOf(path))}: ${(e as Error).message}`); }
    }
    entries = ok;
    errors = bad;
    const reg = loadRegistry(await source.listRegistry());
    registry = reg.entries;
    unreadable = reg.unreadable;
    events = parseEvents(await source.readEvents());
    readPosition = reconcileReadPosition(eventsKey, readPosition, events.length);
    heartbeat = await source.readBackendHeartbeat();
  }

  const titleFor = (p: string) => findByTaskPath(entries, p)?.record.title;

  function openByPath(p: string) {
    const h = findByTaskPath(entries, p);
    actionError = h ? null : `no loaded record for ${p}`;
    if (h) { showNotices = false; openPath = h.path; }
  }

  async function request(h: Held, key: RequestKey) {
    await submitRequest(source!, h.path, h.text, key);
    await reread(h.path);
  }

  async function cancelFromAgents(p: string) {
    const h = findByTaskPath(entries, p);
    if (!h) { actionError = `no loaded record for ${p}`; return; }
    try { await request(h, 'cancel'); actionError = null; }
    catch (e) { actionError = (e as Error).message; }
  }

  function markAllRead(n: number) {
    readPosition = n;
    saveReadPosition(eventsKey, n);
  }

  async function reread(path: string) {
    const text = await source!.readRecord(path);
    const held = { path, record: parseRecord(text), text };
    entries = entries.map((e) => (e.path === path ? held : e));
  }

  async function answer(h: Held, input: AnswerInput) {
    await submitAnswer(source!, h.path, h.text, input);
    await reread(h.path);
  }

  async function approve(h: Held, d: ApprovalDecision) {
    await submitApproval(source!, h.path, h.text, d);
    await reread(h.path);
  }

  onMount(() => {
    let last = '';
    let dead = false;
    const tick = async () => {
      now = new Date();
      if (!source) return;
      try {
        const st = await source.stamp();
        if (st !== last) { last = st; await load(); }
      } catch (e) { errors = [(e as Error).message]; }
    };
    createFabricSource().then(
      (src) => {
        if (dead) return;
        source = src;
        eventsKey = src.eventsKey();
        readPosition = loadReadPosition(eventsKey);
        void tick();
      },
      (e) => { errors = [(e as Error).message]; },
    );
    const t = setInterval(tick, 1000);
    return () => { dead = true; clearInterval(t); };
  });

  function onkeydown(e: KeyboardEvent) {
    if (e.key !== 'Escape') return;
    if (open) openPath = null;
    else onclose();
  }
</script>

<svelte:window {onkeydown} />

{#if open}
  {@const h = open}
  <RecordScreen record={h.record} name={folderName(folderOf(h.path))} onback={() => (openPath = null)} onanswer={(a) => answer(h, a)} onapprove={(d) => approve(h, d)} onrequest={(k) => request(h, k)} {now} onreload={() => reread(h.path)} />
{:else}
  <Page title="Tasks" back="Back" onback={onclose}>
    <div class="fv">
      <div class="fv__head">
        <Segmented label="View" options={[{ value: 'board', label: 'Board' }, { value: 'tree', label: 'Tree' }, { value: 'agents', label: 'Agents' }]} bind:value={view} />
        <div class="fv__tools">
          <span class="fv__backend fv__backend--{backend}" title={BACKEND_LABEL[backend]}>{BACKEND_LABEL[backend]}</span>
          <NoticesBadge {events} {readPosition} onclick={() => (showNotices = !showNotices)} />
        </div>
      </div>
      {#if view === 'board'}
        <BoardScreen {entries} onopen={(e) => (openPath = e.path)} />
      {:else if view === 'agents'}
        <AgentsScreen entries={registry} {now} {titleFor} oncancel={cancelFromAgents} onopen={openByPath} />
        {#if unreadable > 0}<p class="fv__quiet">{unreadable} unreadable {unreadable === 1 ? 'entry' : 'entries'}</p>{/if}
      {:else}
        <TreeScreen {entries} onopen={(e) => (openPath = e.path)} />
      {/if}
      {#if actionError}<p class="fv__err">{actionError}</p>{/if}
      {#each errors as m (m)}<p class="fv__err">{m}</p>{/each}
    </div>
  </Page>
  <Sheet open={showNotices} title="Notices" onclose={() => (showNotices = false)}>
    <NoticesList {events} {readPosition} {titleFor} onopen={openByPath} onmarkallread={markAllRead} />
  </Sheet>
{/if}

<style>
  .fv { display: flex; flex-direction: column; gap: 14px; padding: 8px 0 24px; }
  .fv__head { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  .fv__tools { display: flex; align-items: center; gap: 10px; }
  .fv__backend { color: var(--label-3); font-size: var(--fs-12); white-space: nowrap; }
  .fv__backend--connected { color: var(--green, var(--label-2)); }
  .fv__backend--stale { color: var(--orange, var(--label-2)); }
  .fv__quiet { margin: 0; color: var(--label-3); font-size: var(--fs-12); }
  .fv__err { margin: 0; color: var(--red); font-size: var(--fs-12); }
</style>
