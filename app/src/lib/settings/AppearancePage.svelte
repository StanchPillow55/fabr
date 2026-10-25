<script lang="ts">
  import { TINTS, type Store, type Theme } from '../store.svelte';
  import Group from '../ui/Group.svelte';
  import Icon from '../ui/Icon.svelte';
  import Segmented from '../ui/Segmented.svelte';

  let { store }: { store: Store } = $props();
  let theme = $state<Theme>('system');
  $effect.pre(() => { theme = store.theme; });
  const THEMES: { value: Theme; label: string }[] = [
    { value: 'system', label: 'System' },
    { value: 'light', label: 'Light' },
    { value: 'dark', label: 'Dark' },
  ];
</script>

<Group header="Theme">
  <div class="page-pad"><Segmented label="Theme" options={THEMES} bind:value={theme} onchange={(t) => store.setTheme(t)} /></div>
</Group>
<Group header="Tint" footer="The colour of buttons, selections and nodes without a colour of their own.">
  <div class="page-pad">
    <div class="swatches settings-swatches" role="radiogroup" aria-label="Tint">
      {#each TINTS as t (t)}
        <button type="button" role="radio" aria-checked={store.tint === t} aria-label={t} title={t}
          class="swatch" class:swatch--on={store.tint === t} style:--swatch="var(--{t})" onclick={() => store.setTint(t)}>
          {#if store.tint === t}<Icon name="check" size={12} />{/if}
        </button>
      {/each}
    </div>
  </div>
</Group>
