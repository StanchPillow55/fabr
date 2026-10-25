<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { createApi } from './lib/api/index';
  import { nodeKey } from './lib/model';
  import { needsOnboarding } from './lib/addnodes';
  import AddNodesSheet from './lib/settings/AddNodesSheet.svelte';
  import Onboarding from './lib/Onboarding.svelte';
  import ListView from './lib/list/ListView.svelte';
  import Notice from './lib/Notice.svelte';
  import Panel from './lib/panel/Panel.svelte';
  import SettingsSheet from './lib/settings/SettingsSheet.svelte';
  import { Store } from './lib/store.svelte';
  import SetupWizard from './lib/setup/SetupWizard.svelte';
  import { shouldShowSetup } from './lib/setup/gate';
  import NoticesList from './lib/fabric/NoticesList.svelte';
  import { FabricStore, localRoots } from './lib/fabric/tasks.svelte';
  import { folderOf } from './lib/fabric/folder';
  import Button from './lib/ui/Button.svelte';
  import Sheet from './lib/ui/Sheet.svelte';
  import Toolbar from './lib/Toolbar.svelte';
  import Shortcuts from './lib/ui/Shortcuts.svelte';
  import { keyAction } from './lib/viewstate';
  import TreeView from './lib/tree/TreeView.svelte';

  let store = $state<Store | null>(null);
  let treeView = $state<TreeView>();
  let toolbar = $state<Toolbar>();
  let toolbarH = $state(0);
  let helpOpen = $state(false);
  let fabric = $state<FabricStore | null>(null);
  let noticesOpen = $state(false);
  let noticeError = $state<string | null>(null);
  let setupOpen = $state(new URLSearchParams(location.search).has('setup'));
  let gateReady = $state(false);

  let gateApi: Awaited<ReturnType<typeof createApi>> | null = null;
  let setupNote = $state<string | null>(null);
  let alive = true;

  function startStore() {
    if (store || !gateApi || !alive) return;
    store = new Store(gateApi);
    store.start();
  }

  async function checkGate(api: Awaited<ReturnType<typeof createApi>>) {
    gateApi = api;
    try {
      if (shouldShowSetup(await api.fabricSettingsRead())) setupOpen = true;
    } catch {
      setupOpen = true;
      setupNote = 'Could not read your settings. Finish setup to continue.';
    }
    gateReady = true;
    if (!setupOpen) startStore();
  }

  async function setupDone() {
    let read;
    try { read = await gateApi!.fabricSettingsRead(); }
    catch (e) { setupNote = `Could not confirm your settings: ${(e as Error).message}`; return; }
    if (shouldShowSetup(read)) { setupNote = 'Setup is not finished yet. Complete the remaining steps to continue.'; return; }
    setupNote = null;
    setupOpen = false;
    startStore();
  }

  onMount(() => {
    createApi().then(checkGate);
    return () => { alive = false; store?.stop(); fabric?.stop(); };
  });

  $effect(() => {
    if (store?.tree && !fabric) {
      fabric = new FabricStore();
      void fabric.start(localRoots(store.tree.sources));
    }
  });

  function openNotice(taskPath: string) {
    const held = fabric?.locate(taskPath);
    const folder = held ? folderOf(held.path) : null;
    const node = folder && store ? [...store.byKey.values()].find((n) => fabric!.folderFor(store!.sourceOf(n.source), n.path) === folder) : undefined;
    if (!held || !node) {
      noticeError = held ? 'That folder is not in the tree.' : `No record found for ${taskPath}.`;
      return;
    }
    noticeError = null;
    noticesOpen = false;
    store!.openPanel(nodeKey(node));
    fabric!.open(held.path);
  }

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
    if (!store || store.sheetOpen || store.addNodesFor || helpOpen || noticesOpen || fabric?.openPath) return;
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

{#if gateReady && setupOpen}
  {#if setupNote}<p class="gate-note" role="alert">{setupNote}</p>{/if}
  <SetupWizard ondone={setupDone} />
{:else if store}
  <div class="app" class:app--panel={!!store.panel}>
    <main class="app__main">
      <div class="app__bar" bind:clientHeight={toolbarH}>
        <Toolbar bind:this={toolbar} {store} onrescan={rescan} onexpand={expandAll} oncollapse={collapseAll} onsearchenter={() => treeView?.nextMatch()} {fabric} onnotices={() => { noticeError = null; noticesOpen = true; }} />
      </div>
      <Notice {store} insetTop={toolbarH} />
      {#if needsOnboarding(store.tree)}
        <Onboarding {store} />
      {:else if store.view === 'tree'}
        <TreeView bind:this={treeView} {store} insetTop={toolbarH} {fabric} />
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
    <Panel {store} {fabric} />
  </div>
  <SettingsSheet {store} />
  <AddNodesSheet {store} />
  {#if fabric}
    <Sheet open={noticesOpen} title="Notices" onclose={() => (noticesOpen = false)}>
      <NoticesList events={fabric.events} readPosition={fabric.readPosition} titleFor={fabric.titleFor} onopen={openNotice} onmarkallread={() => fabric?.markAllRead()} />
      {#if noticeError}<p class="gate-note" role="alert">{noticeError}</p>{/if}
      {#snippet footer()}<Button variant="tint" onclick={() => (noticesOpen = false)}>Close</Button>{/snippet}
    </Sheet>
  {/if}
  <Shortcuts open={helpOpen} onclose={() => (helpOpen = false)} />
{/if}

<style>
  .gate-note { margin: 0; padding: 12px 16px; color: var(--red); font-size: var(--fs-13, 13px); text-align: center; }
</style>
