<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { Banner } from '../store.svelte';
  import Icon from '../ui/Icon.svelte';
  let { banner, children }: { banner: Banner | null; children?: Snippet } = $props();
</script>

{#if banner}
  <div class="banner" class:banner--error={banner.kind === 'error'} role={banner.kind === 'error' ? 'alert' : 'status'}>
    <Icon name="warning" size={14} class="banner__icon" />
    <div class="banner__body">
      <p class="banner__text">{banner.text}</p>
      {#if children}<div class="banner__actions">{@render children()}</div>{/if}
    </div>
  </div>
{/if}
