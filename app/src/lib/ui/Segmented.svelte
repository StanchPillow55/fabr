<script lang="ts" generics="T extends string">
  let { options, value = $bindable(), onchange, label, size = 'md' }: {
    options: { value: T; label: string; title?: string }[];
    value: T;
    onchange?: (v: T) => void;
    label?: string;
    size?: 'sm' | 'md';
  } = $props();

  function pick(v: T) {
    if (v === value) return;
    value = v;
    onchange?.(v);
  }
  function arrows(e: KeyboardEvent) {
    const i = options.findIndex((o) => o.value === value);
    if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') {
      e.preventDefault();
      pick(options[(i + (e.key === 'ArrowRight' ? 1 : options.length - 1)) % options.length].value);
    }
  }
</script>

<div class="seg" class:seg--sm={size === 'sm'} role="radiogroup" aria-label={label} tabindex="-1" onkeydown={arrows}>
  {#each options as o (o.value)}
    <button
      type="button"
      role="radio"
      aria-checked={value === o.value}
      tabindex={value === o.value ? 0 : -1}
      class="seg__item"
      class:seg__item--on={value === o.value}
      title={o.title}
      onclick={() => pick(o.value)}
    >{o.label}</button>
  {/each}
</div>
