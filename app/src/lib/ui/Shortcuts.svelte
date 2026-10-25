<script lang="ts">
  import { SHORTCUTS } from '../viewstate';
  import Sheet from './Sheet.svelte';
  let { open, onclose }: { open: boolean; onclose: () => void } = $props();
</script>

<Sheet {open} title="Keyboard shortcuts" {onclose} width={420}>
  <div class="group">
    {#each SHORTCUTS as s (s.what)}
      <div class="krow">
        <span class="krow__keys">{#each s.keys as k (k)}<kbd class="kbd">{k}</kbd>{/each}</span>
        <span class="krow__what">{s.what}</span>
      </div>
    {/each}
  </div>
  {#snippet footer()}<button type="button" class="btn btn--tint" onclick={onclose}>Done</button>{/snippet}
</Sheet>
