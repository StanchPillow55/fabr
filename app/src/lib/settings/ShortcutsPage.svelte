<script lang="ts">
  import Group from '../ui/Group.svelte';
  import Row from '../ui/Row.svelte';
  import { SHORTCUTS } from '../viewstate';
</script>

<Group footer="Shortcuts are off while you type in a field or while this sheet is open.">
  {#each SHORTCUTS as s (s.keys.join('  '))}
    <Row label={s.what}><kbd class="settings-kbd">{s.keys.join('  ')}</kbd></Row>
  {/each}
</Group>
