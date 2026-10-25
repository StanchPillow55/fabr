<script lang="ts">
  import type { Snippet } from 'svelte';
  let { header, footer, error = false, children }: { header?: string; footer?: string | Snippet; error?: boolean; children: Snippet } = $props();
</script>

<section class="page-group">
  {#if header}<h3 class="page-group__head">{header}</h3>{/if}
  <div class="page-card">{@render children()}</div>
  {#if footer}
    <p class="page-group__foot" class:page-group__foot--error={error}>{#if typeof footer === 'string'}{footer}{:else}{@render footer()}{/if}</p>
  {/if}
</section>
