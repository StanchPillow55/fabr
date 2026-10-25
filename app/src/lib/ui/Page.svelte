<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';
  let { title, back, onback, ondone, doneLabel = 'Done', children }: {
    title: string;
    back?: string;
    onback?: () => void;
    ondone?: () => void;
    doneLabel?: string;
    children: Snippet;
  } = $props();
</script>

<div class="page">
  <header class="page-bar">
    <span class="page-bar__side">
      {#if back}<button type="button" class="page-bar__btn" onclick={onback}><Icon name="chevron-left" size={14} /> {back}</button>{/if}
    </span>
    <h2 class="page-bar__title">{title}</h2>
    <span class="page-bar__side page-bar__side--end">
      {#if ondone}<button type="button" class="page-bar__btn page-bar__btn--done" onclick={ondone}>{doneLabel}</button>{/if}
    </span>
  </header>
  <div class="page-body">{@render children()}</div>
</div>
