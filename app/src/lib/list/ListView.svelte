<script lang="ts">
  import { flattenItems, groupRows, sortRows, type SortBy } from '../model';
  import type { Store } from '../store.svelte';
  import ListRowView from './ListRow.svelte';

  let { store, insetTop = 0 }: { store: Store; insetTop?: number } = $props();

  const multi = $derived(store.multiSource);
  const rows = $derived.by(() => {
    const root = store.displayRoot;
    if (!root) return [];
    return flattenItems(root, { today: store.today, filter: store.filter, hideDone: store.hideDone }, store.query);
  });
  const sorted = $derived(sortRows(rows, store.list.sortBy, store.list.sortDir, store.today));
  const groups = $derived(groupRows(sorted, multi || store.list.groupBy !== 'source' ? store.list.groupBy : 'none', store.today));
  const cols = $derived<{ id: SortBy; label: string; cls: string }[]>([
    ...(multi ? [{ id: 'source' as const, label: 'Source', cls: 'source' }] : []),
    { id: 'node', label: 'Node', cls: 'node' },
    { id: 'pri', label: 'Priority', cls: 'pri' },
    { id: 'status', label: 'Status', cls: 'status' },
    { id: 'title', label: 'Title', cls: 'title' },
    { id: 'due', label: 'Due', cls: 'due' },
  ]);

  function sortBy(c: SortBy) {
    if (store.list.sortBy === c) store.list.sortDir = store.list.sortDir === 'asc' ? 'desc' : 'asc';
    else {
      store.list.sortBy = c;
      store.list.sortDir = 'asc';
    }
    store.savePrefs();
  }
  const groupLabel = (id: string, label: string) =>
    store.list.groupBy === 'status' ? label.toUpperCase() : store.list.groupBy === 'source' ? (store.sourceOf(id)?.name ?? label) : label;
</script>

<div class="listview" style:padding-top="{insetTop}px">
  {#if !rows.length}
    <div class="empty-state">
      <p class="empty-state__title">No items</p>
      <p class="empty-state__text">{store.query ? 'Nothing matches your search.' : store.filter !== 'all' || store.hideDone ? 'Nothing passes the current filter.' : 'This part of the tree has no tasks yet.'}</p>
    </div>
  {:else}
    <div class="listview__card">
      <table class="table">
        <colgroup>
          {#each cols as c (c.id)}<col class="table__col--{c.cls}" />{/each}
        </colgroup>
        <thead>
          <tr>
            {#each cols as c (c.id)}
              {@const on = store.list.sortBy === c.id}
              <th
                class="table__th table__th--{c.cls}"
                class:table__th--sorted={on}
                aria-sort={on ? (store.list.sortDir === 'asc' ? 'ascending' : 'descending') : 'none'}
                scope="col"
              >
                <button type="button" class="table__sort" onclick={() => sortBy(c.id)}>
                  {c.label}<span class="table__arrow" aria-hidden="true">{on ? (store.list.sortDir === 'asc' ? '▲' : '▼') : ''}</span>
                </button>
              </th>
            {/each}
          </tr>
        </thead>
        {#each groups as g (g.id)}
          <tbody>
            {#if g.label}
              <tr class="table__grouprow">
                <th class="table__group" colspan={cols.length} scope="rowgroup">
                  {groupLabel(g.id, g.label)}<span class="table__count">{g.rows.length}</span>
                </th>
              </tr>
            {/if}
            {#each g.rows as r (r.id)}
              <ListRowView
                row={r}
                today={store.today}
                showSource={multi}
                sourceName={store.sourceOf(r.source)?.name}
                selected={store.selected === r.nodeKey}
                color={store.colors.get(r.nodeKey) ?? null}
                onopen={() => store.openPanel(r.nodeKey)}
              />
            {/each}
          </tbody>
        {/each}
      </table>
    </div>
    <p class="listview__foot">{rows.length} {rows.length === 1 ? 'item' : 'items'}</p>
  {/if}
</div>
