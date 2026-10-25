<script lang="ts">
  let { checked = $bindable(false), label, onchange }: { checked?: boolean; label: string; onchange?: (v: boolean) => void } = $props();
</script>

<button type="button" role="switch" aria-checked={checked} aria-label={label} class="row-switch" class:row-switch--on={checked}
  onclick={(e) => { e.preventDefault(); checked = !checked; onchange?.(checked); }}><span class="row-switch__knob"></span></button>
