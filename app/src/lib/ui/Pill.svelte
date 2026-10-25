<script lang="ts">
  import type { Status } from '../api/types';
  let { status, onclick, disabled = false }: { status: Status; onclick?: () => void; disabled?: boolean } = $props();
</script>

<button
  type="button"
  class="pill pill--{status}"
  title={onclick ? `${status}: click to change status` : status}
  disabled={disabled || !onclick}
  onclick={(e) => { e.stopPropagation(); onclick?.(); }}
  ondblclick={(e) => e.stopPropagation()}
>{status}</button>
