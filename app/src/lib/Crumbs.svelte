<script lang="ts">
  import type { Store } from './store.svelte';
  import Icon from './ui/Icon.svelte';

  let { store }: { store: Store } = $props();

  const segs = $derived(store.pathSegments());
  const linkable = (k: string) => k === ':' || store.viewIndex.byKey.has(k);
  const go = (k: string) => store.setFocus(k === ':' ? null : k);
</script>

<nav class="crumbs" aria-label="Path">
  {#each segs as s, i (s.key)}
    {#if i}<Icon name="chevron-right" size={10} class="crumbs__sep" />{/if}
    {#if i < segs.length - 1 && linkable(s.key)}
      <button type="button" class="crumbs__link" title="Show from here" onclick={() => go(s.key)}>{s.label}</button>
    {:else}
      <span class="crumbs__here" class:crumbs__here--last={i === segs.length - 1}>{s.label}</span>
    {/if}
  {/each}
</nav>
