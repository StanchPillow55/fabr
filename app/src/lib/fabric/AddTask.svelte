<script lang="ts">
  import NewTaskSheet from './NewTaskSheet.svelte';
  import type { CreateFields } from './source';
  import type { FabricStore } from './tasks.svelte';

  let { fabric, folder }: { fabric: FabricStore; folder: string } = $props();

  let text = $state('');
  let sheet = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);

  function ask() {
    if (sheet || !text.trim()) return;
    error = null;
    sheet = true;
  }
  function enter(e: KeyboardEvent) {
    if (e.key !== 'Enter') return;
    e.preventDefault();
    ask();
  }
  async function finish(f: CreateFields, delegate: boolean) {
    if (busy) return;
    busy = true;
    error = null;
    try {
      const held = delegate ? await fabric.createAndDelegate(folder, f) : await fabric.createTask(folder, f);
      text = '';
      sheet = false;
      fabric.open(held.path);
    } catch (e) {
      error = (e as Error).message;
      await fabric.load().catch(() => {});
    } finally {
      busy = false;
    }
  }
</script>

<div class="group" data-testid="add-task">
  <div class="erow erow--draft">
    <input class="erow__title" type="text" placeholder="Add task" aria-label="Add task" bind:value={text} onkeydown={enter} onblur={ask} />
  </div>
</div>
{#if sheet}
  <NewTaskSheet title={text.trim()} {busy} {error} onsave={(f) => finish(f, false)} ongenerate={(f) => finish(f, true)} oncancel={() => (sheet = false)} />
{/if}
