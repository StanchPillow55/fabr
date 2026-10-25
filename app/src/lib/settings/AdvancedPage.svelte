<script lang="ts">
  import { isTauri } from '../api/env';
  import { errText } from '../api/types';
  import type { Store } from '../store.svelte';
  import Group from '../ui/Group.svelte';
  import NumField from '../ui/NumField.svelte';
  import Row from '../ui/Row.svelte';
  import type { SettingsController } from './controller.svelte';
  import type { AdvField } from './units';

  let { store, ctl }: { store: Store; ctl: SettingsController } = $props();
  let copied = $state(false);
  const path = $derived(store.settings.log_path ?? '');
  const canReveal = isTauri();

  type Field = { f: AdvField; label: string; unit: string; mode: 'decimal' | 'numeric' };
  const GROUPS: { header: string; hint: string; fields: Field[] }[] = [
    {
      header: 'Refresh',
      hint: 'How often changes are looked for. A full rescan also finds nodes outside the folders already known.',
      fields: [
        { f: 'poll', label: 'Local sources', unit: 'seconds', mode: 'decimal' },
        { f: 'remote', label: 'Remote sources', unit: 'seconds', mode: 'decimal' },
        { f: 'rescan', label: 'Full rescan', unit: 'minutes', mode: 'decimal' },
      ],
    },
    {
      header: 'Scan limits',
      hint: 'A scan stops after this many nodes. A source can override the depth on its own page.',
      fields: [
        { f: 'depth', label: 'Look for nodes', unit: 'levels deep', mode: 'numeric' },
        { f: 'maxNodes', label: 'Max nodes', unit: '', mode: 'numeric' },
      ],
    },
  ];

  async function reveal() {
    try {
      await store.api.revealPath(path);
    } catch (e) {
      ctl.error = errText(e);
    }
  }
  async function copy() {
    try {
      await navigator.clipboard.writeText(path);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {}
  }
  const err = (fields: Field[]) => fields.map(({ f }) => ctl.advErrors[f]).filter(Boolean).join(' ') || undefined;
</script>

{#each GROUPS as g (g.header)}
  <Group header={g.header} footer={err(g.fields) ?? g.hint} error={!!err(g.fields)}>
    {#each g.fields as x (x.f)}
      <NumField label={x.label} unit={x.unit} mode={x.mode} bad={!!ctl.advErrors[x.f]} bind:value={ctl.adv[x.f]}
        aria="{x.label}, {x.unit}" oncommit={() => ctl.commitAdvanced()} />
    {/each}
  </Group>
{/each}

{#if path}
  <Group header="Logs">
    <Row label="Log file"><span class="log-path" title={path}>{path}</span></Row>
    <Row label={copied ? 'Copied' : 'Copy path'} tone="tint" onclick={copy} />
    {#if canReveal}<Row label="Show in folder" tone="tint" onclick={reveal} />{/if}
  </Group>
{/if}
{#if ctl.error}<div class="banner banner--error page-panel" role="alert">{ctl.error}</div>{/if}
