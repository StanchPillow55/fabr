<script lang="ts">
  import type { Snippet } from 'svelte';
  let { label, trigger, children, width = 300 }: {
    label: string;
    trigger: Snippet<[{ toggle: () => void; open: boolean; id: string }]>;
    children: Snippet;
    width?: number;
  } = $props();

  let open = $state(false);
  let root = $state<HTMLDivElement>();
  let pop = $state<HTMLDivElement>();
  const id = `pop-${Math.random().toString(36).slice(2, 8)}`;

  export function close() {
    if (!open) return;
    open = false;
    root?.querySelector<HTMLElement>('[data-popover-trigger]')?.focus();
  }
  function toggle() {
    open = !open;
  }
  $effect(() => {
    if (!open) return;
    pop?.focus();
    const down = (e: MouseEvent) => {
      if (root && !root.contains(e.target as Node)) open = false;
    };
    document.addEventListener('mousedown', down, true);
    return () => document.removeEventListener('mousedown', down, true);
  });
  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && open) {
      e.stopPropagation();
      e.preventDefault();
      close();
    }
  }
</script>

<div class="popover" bind:this={root} {onkeydown} role="presentation">
  {@render trigger({ toggle, open, id })}
  {#if open}
    <div class="popover__panel" {id} role="dialog" aria-label={label} tabindex="-1" bind:this={pop} style:width="{width}px">
      {@render children()}
    </div>
  {/if}
</div>
