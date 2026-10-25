<script lang="ts">
  import type { Store } from './store.svelte';
  import ErrorText from './ui/ErrorText.svelte';
  import Icon from './ui/Icon.svelte';
  let { store, insetTop = 0 }: { store: Store; insetTop?: number } = $props();
  const errors = $derived(store.tree?.errors ?? []);
  let dismissed = $state<string>('');
  const sig = $derived(errors.map((e) => `${e.source}:${e.message}`).join('|'));
</script>

<div class="notices" style:top="{insetTop + 10}px" aria-live="polite">
  {#if store.notice}
    <div class="notice" role="status">
      <Icon name="warning" size={14} class="notice__icon" />
      <span class="notice__text">{store.notice}</span>
      {#if store.noticeRescan}<button type="button" class="btn btn--tint notice__action" onclick={() => store.rescan()}>Rescan</button>{/if}
      <button type="button" class="btn btn--icon" aria-label="Dismiss" onclick={() => (store.notice = null)}><Icon name="x" size={12} /></button>
    </div>
  {/if}
  {#if errors.length && dismissed !== sig}
    <div class="notice notice--error" role="alert">
      <Icon name="warning" size={14} class="notice__icon" />
      <span class="notice__text">
        {#each errors as e, i (i)}<span class="notice__line"><b>{store.sourceOf(e.source)?.name ?? e.source}</b> <ErrorText message={e.message} onopen={(u) => store.openUrl(u)} /></span>{/each}
      </span>
      <button type="button" class="btn btn--icon" aria-label="Dismiss" onclick={() => (dismissed = sig)}><Icon name="x" size={12} /></button>
    </div>
  {/if}
</div>
