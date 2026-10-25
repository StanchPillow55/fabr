<script lang="ts">
  import type { Status } from '../api/types';
  let { status, onclick, disabled = false }: { status: Status; onclick?: () => void; disabled?: boolean } = $props();
  const label = $derived(status === 'wip' ? 'active' : status);
</script>

<button
  type="button"
  class="pill pill--{status}"
  title={label}
  disabled={disabled || !onclick}
  onclick={(e) => { e.stopPropagation(); onclick?.(); }}
  ondblclick={(e) => e.stopPropagation()}
>{label}</button>
