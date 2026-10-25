<script lang="ts">
  import { nameFromPath } from './addnodes';
  import { hasNativePicker, pickFolder } from './api/dialog';
  import { errText } from './api/types';
  import type { Store } from './store.svelte';
  import Button from './ui/Button.svelte';
  import Field from './ui/Field.svelte';
  import Icon from './ui/Icon.svelte';

  let { store }: { store: Store } = $props();

  const native = hasNativePicker();
  let root = $state('');
  let name = $state('');
  let busy = $state(false);
  let error = $state<string | null>(null);

  async function choose() {
    error = null;
    try {
      const dir = await pickFolder();
      if (dir) {
        root = dir;
        await confirm();
      }
    } catch (e) {
      error = errText(e);
    }
  }
  async function confirm() {
    if (!root.trim() || busy) return;
    busy = true;
    error = null;
    try {
      await store.onboard(root, name);
    } catch (e) {
      error = errText(e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="welcome">
  <div class="welcome__card">
    <img class="welcome__icon" src="/favicon.svg" alt="" width="64" height="64" />
    <h2 class="welcome__title">Welcome to Fabr</h2>
    <p class="welcome__text">Pick a folder to start. Any directory under it can become a node with its own tasks, kept as plain text next to your files.</p>
    {#if native}
      <Field label="Name (optional)">
        <input type="text" bind:value={name} placeholder={root ? nameFromPath(root) : 'Home'} />
      </Field>
      <Button variant="primary" disabled={busy} onclick={choose}>{busy ? 'Setting up…' : 'Choose a folder…'}</Button>
    {:else}
      <form class="welcome__form" onsubmit={(e) => { e.preventDefault(); confirm(); }}>
        <Field label="Folder">
          <input type="text" bind:value={root} spellcheck="false" placeholder="~/workspace" />
        </Field>
        <Field label="Name (optional)">
          <input type="text" bind:value={name} placeholder={root ? nameFromPath(root) : 'Home'} />
        </Field>
        <Button type="submit" variant="primary" disabled={busy || !root.trim()}>{busy ? 'Setting up…' : 'Use this folder'}</Button>
      </form>
    {/if}
    {#if error}<div class="banner banner--error" role="alert"><Icon name="warning" size={14} /> <span>{error}</span></div>{/if}
  </div>
</div>
