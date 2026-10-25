<script lang="ts">
  import { isVirtualRoot } from '../model';
  import type { Store } from '../store.svelte';
  import Page from '../ui/Page.svelte';
  import NodeSettingsBody from './NodeSettingsBody.svelte';

  let { store }: { store: Store } = $props();

  const p = $derived(store.nodeSettings);
  const src = $derived(p ? store.sourceOf(p.source) : undefined);
  const location = $derived(!p ? '' : isVirtualRoot(p) ? 'All sources' : [store.multiSource ? (src?.name ?? p.source) : (src?.root ?? ''), p.path].filter(Boolean).join(' / '));
  let sheet = $state<HTMLDivElement>();

  $effect(() => {
    if (p) sheet?.focus();
  });

  function onkeydown(e: KeyboardEvent) {
    if (!p || store.addNodesFor || e.code !== 'Escape') return;
    e.preventDefault();
    e.stopPropagation();
    store.closeNodeSettings();
  }
</script>

<svelte:window {onkeydown} />

{#if p}
  <div class="sheet__backdrop" role="presentation" onmousedown={(e) => { if (e.target === e.currentTarget) store.closeNodeSettings(); }}>
    <div class="sheet settings-sheet" role="dialog" aria-modal="true" aria-label="Node settings" tabindex="-1" bind:this={sheet}>
      <Page title={p.name} ondone={() => store.closeNodeSettings()}>
        {#if location}<p class="hint nodepath">{location}</p>{/if}
        <NodeSettingsBody {store} {p} />
      </Page>
    </div>
  </div>
{/if}

<style>
  .nodepath { margin: 0 16px 8px; overflow-wrap: anywhere; }
</style>
