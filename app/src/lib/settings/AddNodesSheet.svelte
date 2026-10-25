<script lang="ts">
  import { namesToInit, selectable, toggleAll, toggleOne } from '../addnodes';
  import { errText, type DirEntry } from '../api/types';
  import type { Store } from '../store.svelte';
  import Button from '../ui/Button.svelte';
  import Icon from '../ui/Icon.svelte';
  import Sheet from '../ui/Sheet.svelte';

  let { store }: { store: Store } = $props();

  const target = $derived(store.addNodesFor);
  let dirs = $state<DirEntry[]>([]);
  let selected = $state<ReadonlySet<string>>(new Set());
  let loading = $state(false);
  let saving = $state(false);
  let error = $state<string | null>(null);

  let seq = 0;
  $effect(() => {
    const t = target;
    if (!t) return;
    const mine = ++seq;
    dirs = [];
    selected = new Set();
    error = null;
    loading = true;
    (async () => {
      try {
        const d = await store.api.listDirs({ source: t.source, path: t.path });
        if (mine === seq) dirs = d;
      } catch (e) {
        if (mine === seq) error = errText(e);
      } finally {
        if (mine === seq) loading = false;
      }
    })();
  });

  const free = $derived(selectable(dirs));
  const picked = $derived(namesToInit(dirs, selected));
  const allOn = $derived(free.length > 0 && picked.length === free.length);

  function close() {
    store.addNodesFor = null;
  }
  async function submit() {
    const t = target;
    if (!t || !picked.length) return;
    saving = true;
    error = null;
    try {
      await store.addNodes(t.source, t.path, picked);
      close();
    } catch (e) {
      error = errText(e);
    } finally {
      saving = false;
    }
  }
</script>

<Sheet open={!!target} title="Add nodes" onclose={close} width={480}>
  <p class="hint">Choose the subdirectories of <b>{target?.name}</b> to turn into fabr nodes. Each gets its own tasks and metadata.</p>
  {#if loading}
    <p class="hint">Reading folders…</p>
  {:else if !dirs.length && !error}
    <p class="hint">This directory has no subdirectories.</p>
  {:else if dirs.length}
    <div class="group">
      {#each dirs as d (d.name)}
        <label class="drow" class:drow--off={d.managed}>
          <input type="checkbox" checked={d.managed || selected.has(d.name)} disabled={d.managed} onchange={() => (selected = toggleOne(selected, d.name))} />
          <Icon name="folder" size={14} />
          <span class="drow__name">{d.name}</span>
          {#if d.managed}<span class="drow__tag">already a node</span>{:else if d.pruned}<span class="drow__tag drow__tag--warn">pruned</span>{/if}
        </label>
      {/each}
    </div>
  {/if}
  {#if error}<div class="banner banner--error" role="alert"><Icon name="warning" size={14} /> <span>{error}</span></div>{/if}

  {#snippet footer()}
    <Button variant="tint" disabled={!free.length} onclick={() => (selected = toggleAll(dirs, selected))}>{allOn ? 'Select none' : 'Select all'}</Button>
    <span class="actions__spacer"></span>
    <Button variant="plain" onclick={close}>{dirs.length ? 'Cancel' : 'Close'}</Button>
    <Button variant="primary" disabled={!picked.length || saving} onclick={submit}>{saving ? 'Initializing…' : `Initialize selected${picked.length ? ` (${picked.length})` : ''}`}</Button>
  {/snippet}
</Sheet>
