<script lang="ts">
  import { untrack } from 'svelte';
  import { AddFlow } from './addflow.svelte';
  import NewTaskSheet from './NewTaskSheet.svelte';
  import type { FabricStore } from './tasks.svelte';

  let { fabric, folder, disabled = false, note = '', variant = 'panel' }: {
    fabric: FabricStore;
    folder: string | null;
    disabled?: boolean;
    note?: string;
    variant?: 'panel' | 'card';
  } = $props();

  const flow = new AddFlow(untrack(() => fabric), () => folder, () => disabled);

  function onkeys(e: KeyboardEvent) {
    if (variant === 'card') e.stopPropagation();
    if (flow.press(e.key)) e.preventDefault();
  }
  function portal(el: HTMLElement) {
    document.body.appendChild(el);
    return () => el.remove();
  }
</script>

{#if variant === 'card'}
  <div class="addtask addtask--card" data-testid="add-task" role="presentation" onclick={(e) => e.stopPropagation()} ondblclick={(e) => e.stopPropagation()}>
    <span class="row__plus" aria-hidden="true">+</span>
    <input class="row__input" type="text" placeholder={note || 'Add task'} aria-label="Add task" autocomplete="off" spellcheck="false" disabled={flow.off} bind:value={flow.text} onkeydown={onkeys} onblur={() => flow.ask()} />
  </div>
{:else}
  <div class="group" data-testid="add-task">
    <div class="erow erow--draft">
      <input class="erow__title" type="text" placeholder="Add task" aria-label="Add task" disabled={flow.off} bind:value={flow.text} onkeydown={onkeys} onblur={() => flow.ask()} />
    </div>
  </div>
{/if}
{#if note && variant !== 'card'}<p class="hint addtask__note" data-testid="add-task-note">{note}</p>{/if}
{#if flow.sheet}
  <div class="portal" {@attach portal}>
    <NewTaskSheet title={flow.text.trim()} busy={flow.busy} error={flow.error} onsave={(f) => flow.finish(f, false)} ongenerate={(f) => flow.finish(f, true)} oncancel={() => flow.cancel()} />
  </div>
{/if}
