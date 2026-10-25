<script lang="ts">
  import { isVirtualRoot, rootCounts, sourceLines } from '../model';
  import { KNOWN_META, removeSummary, type Store } from '../store.svelte';
  import Button from '../ui/Button.svelte';
  import ErrorText from '../ui/ErrorText.svelte';
  import Icon from '../ui/Icon.svelte';
  import BannerView from './Banner.svelte';
  import ItemEditor from './ItemEditor.svelte';
  import MetaEditor from './MetaEditor.svelte';
  import AddTask from '../fabric/AddTask.svelte';
  import TaskSection from '../fabric/TaskSection.svelte';
  import type { FabricStore } from '../fabric/tasks.svelte';

  let { store, fabric = null }: { store: Store; fabric?: FabricStore | null } = $props();

  let editor = $state<ItemEditor>();
  let busy = $state(false);
  let confirmRemove = $state(false);

  const p = $derived(store.panel);
  const src = $derived(p ? store.sourceOf(p.source) : undefined);
  const virtual = $derived(!!p && isVirtualRoot(p));
  const canReveal = $derived(!!p && !virtual && !src?.host && !store.revealUnsupported);
  const location = $derived(!p ? '' : virtual ? 'All sources' : [store.multiSource ? (src?.name ?? p.source) : (src?.root ?? ''), p.path].filter(Boolean).join(' / '));
  const shownRoot = $derived(virtual ? (store.viewIndex.byKey.get(':') ?? null) : null);
  const tallyRoot = $derived(shownRoot ? store.tallyNode(shownRoot) : null);
  const counts = $derived(shownRoot ? rootCounts(tallyRoot!, store.today) : null);
  const lines = $derived(shownRoot && store.tree ? sourceLines(tallyRoot!, store.tree.sources, store.tree.errors, store.today, undefined, store.tree.pending ?? []) : []);
  const taskFolder = $derived(fabric && p && !virtual ? fabric.folderFor(src, p.path) : null);
  const hidden = $derived(p ? p.meta.some(([k, v]) => k === 'hidden' && (v === true || v === 'true')) : false);
  const open = $derived(p ? p.items.filter((i) => i.status !== 'done').length : 0);

  const metaProps = $derived({
    entries: p?.meta ?? [],
    known: KNOWN_META,
    note: p?.metaNote ?? null,
    inherited: p ? store.inheritedColor(p.key) : null,
    oncommit: (k: string, v: string | number | boolean | undefined) => store.commitMeta(k, v),
    onerror: (m: string | null) => { if (store.panel) store.panel.banner = m ? { kind: 'error' as const, text: m } : null; },
  });

  const summary = $derived(removeSummary({ items: p?.items ?? [] }));
  const isSourceRoot = $derived(!!p && p.path === '');
  $effect(() => {
    void p?.key;
    confirmRemove = false;
  });

  async function run(fn: () => Promise<void>) {
    busy = true;
    try {
      await fn();
    } finally {
      busy = false;
    }
  }
</script>

{#if p}
  <aside class="panel" aria-label="Node editor">
    <header class="panel__top">
      <div class="panel__heading">
        <h2 class="panel__title">{p.name}</h2>
        {#if location}<p class="panel__path">{location}</p>{/if}
      </div>
      <div class="panel__actions">
      {#if p.managed && !virtual}
        <Button variant="tint" title={hidden ? 'Show this node and its subtree again' : 'Omit this node and its subtree from the tree, list, search and counts'}
          disabled={busy} onclick={() => run(async () => { await store.setHidden(!hidden); })}>{hidden ? 'Unhide' : 'Hide'}</Button>
      {/if}
      <Button variant="tint" class="panel__done" title="Close (Esc)" onclick={() => store.closePanel()}>Done</Button>
      </div>
    </header>

    <div class="panel__body">
      <BannerView banner={p.banner} />
      {#if p.warnings.length && !(p.managed && !virtual)}
        <ul class="panel__warnings">
          {#each p.warnings as w, i (i)}<li><Icon name="warning" size={12} /> {w}</li>{/each}
        </ul>
      {/if}

      {#if fabric && taskFolder}
        <TaskSection {fabric} folder={taskFolder} />
        <section class="panel__section"><AddTask {fabric} folder={taskFolder} /></section>
      {/if}

      {#if virtual}
        <section class="panel__section">
          <h3 class="section-title">Todos</h3>
          {#if counts}
            <p class="rootcounts">
              <span><b>{counts.open}</b> open</span><span class="dotsep">·</span>
              <span class:card__wip={counts.wip > 0}><b>{counts.wip}</b> wip</span><span class="dotsep">·</span>
              <span class:card__overdue={counts.overdue > 0}><b>{counts.overdue}</b> overdue</span>
            </p>
          {/if}
        </section>
        <section class="panel__section">
          <h3 class="section-title">Sources <span class="section-title__count">{lines.length}</span></h3>
          <div class="group">
            {#each lines as l (l.key)}
              <button type="button" class="arow arow--source" title={l.error ?? `Open ${l.name}`} onclick={() => store.openPanel(l.key)}>
                <span class="dot" class:dot--ok={!l.error && !l.pending} class:dot--wait={!l.error && l.pending} class:dot--bad={!!l.error}></span>
                <span class="arow__label">{l.name} · {l.where}</span>
                {#if l.error}<span class="srcline__error"><ErrorText message={l.error} compact onopen={(u) => store.openUrl(u)} /></span>{:else if l.pending}<span class="srcline__error srcline__pending">Connecting…</span>{:else}<span class="srcline__open">{l.open} open</span>{/if}
              </button>
            {/each}
            <button type="button" class="arow" onclick={() => store.openSettings('sources')}>Manage sources…</button>
          </div>
          <p class="hint">Every configured source, side by side. Add, edit or test sources in Settings.</p>
        </section>
      {:else if !p.managed}
        <div class="empty-node">
          <Icon name="folder" size={28} />
          <p class="empty-node__title">Plain directory</p>
          <p class="hint">It has no <code>.fabr/</code> folder yet. Make it a node to give it todos and metadata.</p>
          <Button variant="tint" onclick={() => store.openAddNodes(p.key)}>Add nodes…</Button>
        </div>
        <section class="panel__section">
          <h3 class="section-title">Actions</h3>
          <div class="group">
            <button type="button" class="arow" disabled={busy} onclick={() => run(() => store.init(p.key))}>Make it a node</button>
            {#if canReveal}<button type="button" class="arow" onclick={() => store.reveal()}>Reveal in Finder</button>{/if}
          </div>
        </section>
      {:else}
        <section class="panel__section">
          <h3 class="section-title">Identity</h3>
          <MetaEditor section="identity" {...metaProps} />
        </section>

        <section class="panel__section">
          <h3 class="section-title">Todos <span class="section-title__count">{open} open · {p.items.length}</span></h3>
          <ItemEditor bind:this={editor} items={p.items} {fabric} folder={taskFolder} oncommit={(m) => store.commitItems(p.key, m)} onfocus={(on) => store.setItemFocus(on)} />
          <div class="actions">
            <Button variant="tint" onclick={() => editor?.add()}><Icon name="plus" size={14} /> Add item</Button>
            <span class="actions__spacer"></span>
            {#if p.itemsNote}<span class="mrow__note" class:mrow__note--error={p.itemsNote.kind === 'error'} role="status">{p.itemsNote.text}</span>{/if}
          </div>
        </section>

        <section class="panel__section">
          <h3 class="section-title">Display</h3>
          <MetaEditor section="display" {...metaProps} />
          <p class="hint">Hidden omits this node and its whole subtree (see Show in the toolbar).</p>
        </section>

        <section class="panel__section">
          <h3 class="section-title">Children</h3>
          <p class="hint">Turn subdirectories of this node into nodes of their own.</p>
          <div class="actions"><Button variant="tint" onclick={() => store.openAddNodes(p.key)}>Add nodes…</Button></div>
        </section>

        <section class="panel__section">
          <h3 class="section-title">Actions</h3>
          {#if canReveal}
            <div class="group"><button type="button" class="arow" onclick={() => store.reveal()}>Reveal in Finder</button></div>
          {/if}
          <div class="group group--danger">
            {#if confirmRemove}
              <div class="confirm" role="alertdialog" aria-label="Remove node">
                <p class="confirm__text">
                  Removes <code>.fabr/</code> from {p.path || src?.root || p.name}. The folder and its files stay; {summary.text}.
                  Subfolder nodes keep theirs.{#if isSourceRoot} The source will show as a plain folder.{/if}
                </p>
                <div class="confirm__buttons">
                  <button type="button" class="arow arow--danger" disabled={busy}
                    onclick={() => run(async () => { await store.removeNode(); })}>Remove</button>
                  <button type="button" class="arow" disabled={busy} onclick={() => (confirmRemove = false)}>Cancel</button>
                </div>
              </div>
            {:else}
              <button type="button" class="arow arow--danger" disabled={busy} onclick={() => (confirmRemove = true)}>Remove node…</button>
            {/if}
          </div>
        </section>

        <details class="more">
          <summary class="more__summary">More…</summary>
          <div class="more__body">
            {#if p.warnings.length}
              <ul class="panel__warnings">
                {#each p.warnings as w, i (i)}<li><Icon name="warning" size={12} /> {w}</li>{/each}
              </ul>
            {/if}
            <h3 class="section-title">Custom keys</h3>
            <MetaEditor section="more" {...metaProps} />
            <p class="hint">Fields save on Enter or when you leave them.</p>
          </div>
        </details>
      {/if}
    </div>

  </aside>
{/if}
