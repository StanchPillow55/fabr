<script lang="ts">
  import { untrack } from 'svelte';
  import Button from '../ui/Button.svelte';
  import Field from '../ui/Field.svelte';
  import Sheet from '../ui/Sheet.svelte';
  import type { CreateFields } from './source';

  let { title = '', due: dueIn = null, context: contextIn = '', busy = false, error = null, onsave, ongenerate, oncancel }: {
    title?: string;
    due?: string | null;
    context?: string;
    busy?: boolean;
    error?: string | null;
    onsave: (f: CreateFields) => void;
    ongenerate: (f: CreateFields) => void;
    oncancel: () => void;
  } = $props();

  let name = $state(untrack(() => title));
  let due = $state(untrack(() => (dueIn ?? '').slice(0, 10)));
  let context = $state(untrack(() => contextIn));
  let spec = $state('');
  let success = $state('');

  const blocked = $derived(busy || !name.trim());
  const fields = (): CreateFields => ({ title: name.trim(), due: due || null, context, spec, success });
</script>

<Sheet open={true} title="New task" width={560} onclose={() => { if (!busy) oncancel(); }}>
  <Field label="Title"><input type="text" bind:value={name} /></Field>
  <Field label="Due"><input type="date" bind:value={due} /></Field>
  <Field label="Context" hint="what you know"><textarea rows="4" bind:value={context}></textarea></Field>
  <Field label="Spec" hint="what exactly should happen"><textarea rows="3" bind:value={spec}></textarea></Field>
  <Field label="Success" hint="how we know it is done"><textarea rows="3" bind:value={success}></textarea></Field>
  {#if error}<p class="hint" role="alert">{error}</p>{/if}
  {#snippet footer()}
    <Button variant="tint" disabled={busy} onclick={oncancel}>Cancel</Button>
    <Button variant="primary" disabled={blocked} onclick={() => onsave(fields())}>Save</Button>
    <Button variant="primary" disabled={blocked} title="Creates the task and asks the agent backend to define it" onclick={() => ongenerate(fields())}>Generate with AI</Button>
  {/snippet}
</Sheet>
