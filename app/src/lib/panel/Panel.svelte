<script lang="ts">
  import { isVirtualRoot } from '../model';
  import type { Store } from '../store.svelte';
  import NodeSettingsBody from '../settings/NodeSettingsBody.svelte';
  import Button from '../ui/Button.svelte';

  let { store }: { store: Store } = $props();

  const p = $derived(store.panel);
  const src = $derived(p ? store.sourceOf(p.source) : undefined);
  const virtual = $derived(!!p && isVirtualRoot(p));
  const location = $derived(!p ? '' : virtual ? 'All sources' : [store.multiSource ? (src?.name ?? p.source) : (src?.root ?? ''), p.path].filter(Boolean).join(' / '));
</script>

{#if p}
  <aside class="panel" aria-label="Node editor">
    <header class="panel__top">
      <div class="panel__heading">
        <h2 class="panel__title">{p.name}</h2>
        {#if location}<p class="panel__path">{location}</p>{/if}
      </div>
      <div class="panel__actions">
        <Button variant="tint" class="panel__done" title="Close (Esc)" onclick={() => store.closePanel()}>Done</Button>
      </div>
    </header>

    <div class="panel__body">
      <NodeSettingsBody {store} {p} />
    </div>
  </aside>
{/if}
