<script lang="ts">
  import type { Snippet } from 'svelte';
  let { label, hint, children }: { label: string; hint?: string; children: Snippet } = $props();
</script>

<label class="field">
  <span class="field__label">{label}</span>
  {@render children()}
  {#if hint}<span class="field__hint">{hint}</span>{/if}
</label>
