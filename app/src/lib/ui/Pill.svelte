<script lang="ts">
  import { PHASE_LABEL, type Phase } from '../fabric/phase';
  let { phase }: { phase: Phase } = $props();
</script>

<span class="pill pill--{phase}" title={PHASE_LABEL[phase]}>{PHASE_LABEL[phase]}</span>
