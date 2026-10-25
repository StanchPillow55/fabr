<script lang="ts">
  import { linkPath, type LayoutResult } from '../layout';
  import { nodeKey, type Key } from '../model';
  let { links, colors = new Map() }: { links: LayoutResult['links']; colors?: ReadonlyMap<Key, string> } = $props();

  const stroke = (k: Key): string | undefined => {
    const c = colors.get(k);
    return c ? `color-mix(in srgb, ${c} 55%, transparent)` : undefined;
  };
</script>

<g class="links">
  {#each links as l (nodeKey(l.to.node))}
    <path class="link" d={linkPath(l.from, l.to)} style:stroke={stroke(nodeKey(l.to.node))} />
  {/each}
  {#each links.filter((l) => l.to.node.via) as l (nodeKey(l.to.node))}
    <text class="via" x={l.to.x + l.to.w / 2 + 8} y={l.to.y - 8}>via {l.to.node.via}</text>
  {/each}
</g>
