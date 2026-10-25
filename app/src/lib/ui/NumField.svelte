<script lang="ts">
  import Row from './Row.svelte';
  import { commit } from './actions';

  let { label, unit = '', value = $bindable(''), bad = false, mode = 'decimal', placeholder, oncommit, aria }: {
    label: string;
    unit?: string;
    value?: string;
    bad?: boolean;
    mode?: 'decimal' | 'numeric';
    placeholder?: string;
    oncommit?: () => void;
    aria?: string;
  } = $props();
</script>

<Row {label}>
  <input class="row-input row-input--num" class:row-input--bad={bad} type="text" inputmode={mode} bind:value {placeholder}
    use:commit={() => oncommit?.()} aria-label={aria ?? (unit ? `${label}, ${unit}` : label)} />
  {#if unit}<span class="row-unit">{unit}</span>{/if}
</Row>
