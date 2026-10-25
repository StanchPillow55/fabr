<script lang="ts">
  import type { Snippet } from 'svelte';
  let { open, title, onclose, children, footer, width = 560 }: {
    open: boolean;
    title: string;
    onclose: () => void;
    children: Snippet;
    footer?: Snippet;
    width?: number;
  } = $props();

  let dialog = $state<HTMLDivElement>();
  $effect(() => {
    if (open && dialog) dialog.focus();
  });
  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      e.preventDefault();
      onclose();
    }
  }
</script>

{#if open}
  <div class="sheet__backdrop" role="presentation" onclick={(e) => { if (e.target === e.currentTarget) onclose(); }} {onkeydown}>
    <div class="sheet" role="dialog" aria-modal="true" aria-label={title} tabindex="-1" bind:this={dialog} style:width="min({width}px, calc(100vw - 32px))">
      <header class="sheet__header">
        <h2 class="sheet__title">{title}</h2>
      </header>
      <div class="sheet__body">{@render children()}</div>
      {#if footer}<footer class="sheet__footer">{@render footer()}</footer>{/if}
    </div>
  </div>
{/if}
