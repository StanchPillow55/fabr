<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { createApi } from './lib/api/index';
  import { needsOnboarding } from './lib/addnodes';
  import AddNodesSheet from './lib/settings/AddNodesSheet.svelte';
  import Onboarding from './lib/Onboarding.svelte';
  import ListView from './lib/list/ListView.svelte';
  import Notice from './lib/Notice.svelte';
  import Panel from './lib/panel/Panel.svelte';
  import SettingsSheet from './lib/settings/SettingsSheet.svelte';
  import { Store } from './lib/store.svelte';
  import Toolbar from './lib/Toolbar.svelte';
  import Shortcuts from './lib/ui/Shortcuts.svelte';
  import { keyAction } from './lib/viewstate';
  import TreeView from './lib/tree/TreeView.svelte';

  let store = $state<Store | null>(null);
  let treeView = $state<TreeView>();
  let toolbar = $state<Toolbar>();
  let toolbarH = $state(0);
  let helpOpen = $state(false);

  onMount(() => {
    let s: Store | null = null;
    createApi().then((api) => {
      s = new Store(api);
      store = s;
      s.start();
    });
    return () => s?.stop();
  });

  function fit() {
    if (store?.view === 'tree') treeView?.fit();
  }
  function rescan() {
    store?.rescan();
  }
  async function collapseAll() {
    store?.collapseAll();
    await tick();
    fit();
  }
  async function expandAll() {
    store?.expandAll();
    await tick();
    fit();
  }

  function onkeydown(e: KeyboardEvent) {
    if (!store || store.sheetOpen || store.addNodesFor || helpOpen) return;
    const t = e.target as HTMLElement;
    const typing = !!t.matches?.('input, select, textarea, [contenteditable]');
    if (typing) {
      if (e.key === 'Escape') t.blur();
      return;
    }
    const a = keyAction(e, false);
    if (!a) return;
    switch (a.type) {
      case 'filter': store.filter = a.value; store.savePrefs(); break;
      case 'view': store.view = a.value; store.savePrefs(); break;
      case 'toggleHidden': store.setShowHidden(!store.showHidden); break;
      case 'toggleDone': store.setHideDone(!store.hideDone); break;
      case 'toggleEmpty': store.setHideEmpty(!store.hideEmpty); break;
      case 'rescan': rescan(); break;
      case 'fit': fit(); break;
      case 'search': e.preventDefault(); toolbar?.focusSearch(); break;
      case 'help': e.preventDefault(); helpOpen = true; break;
      case 'escape': if (store.panel) store.closePanel(); break;
    }
  }
</script>

<svelte:window {onkeydown} />

{#if store}
  <div class="app" class:app--panel={!!store.panel}>
    <main class="app__main">
      <div class="app__bar" bind:clientHeight={toolbarH}>
        <Toolbar bind:this={toolbar} {store} onrescan={rescan} onexpand={expandAll} oncollapse={collapseAll} onsearchenter={() => treeView?.nextMatch()} />
      </div>
      <Notice {store} insetTop={toolbarH} />
      {#if needsOnboarding(store.tree)}
        <Onboarding {store} />
      {:else if store.view === 'tree'}
        <TreeView bind:this={treeView} {store} insetTop={toolbarH} />
      {:else}
        <ListView {store} insetTop={toolbarH} />
      {/if}
      {#if !store.tree && !store.loading && store.notice}
        <div class="empty-state empty-state--center">
          <p class="empty-state__title">Nothing to show</p>
          <p class="empty-state__text">{store.notice}</p>
        </div>
      {/if}
    </main>
    <Panel {store} />
  </div>
  <SettingsSheet {store} />
  <AddNodesSheet {store} />
  <Shortcuts open={helpOpen} onclose={() => (helpOpen = false)} />
{/if}
