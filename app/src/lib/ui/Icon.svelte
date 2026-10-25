<script lang="ts" module>
  const PATHS: Record<string, string> = {
    'chevron-right': 'M6 3.5 10.5 8 6 12.5',
    'chevron-down': 'M3.5 6 8 10.5 12.5 6',
    'chevron-left': 'M10 3.5 5.5 8l4.5 4.5',
    search: 'M7 12.25a5.25 5.25 0 1 0 0-10.5 5.25 5.25 0 0 0 0 10.5ZM10.75 10.75 14 14',
    x: 'M4 4l8 8M12 4l-8 8',
    plus: 'M8 3v10M3 8h10',
    fit: 'M2.75 6V2.75H6M10 2.75h3.25V6M13.25 10v3.25H10M6 13.25H2.75V10',
    trash: 'M2.75 4.25h10.5M6.25 4.25V2.75h3.5v1.5M4.25 4.25l.6 8.4a1 1 0 0 0 1 .93h4.3a1 1 0 0 0 1-.93l.6-8.4',
    folder: 'M1.75 4.25a1 1 0 0 1 1-1h3.1l1.5 1.5h5.9a1 1 0 0 1 1 1v6.5a1 1 0 0 1-1 1H2.75a1 1 0 0 1-1-1Z',
    reveal: 'M9.25 2.75h4v4M13.25 2.75 7.5 8.5M11.25 9.5v3a.75.75 0 0 1-.75.75h-7a.75.75 0 0 1-.75-.75v-7a.75.75 0 0 1 .75-.75h3',
    warning: 'M8 2.25 14.25 13H1.75ZM8 6.5v3M8 11.25v.01',
    server: 'M2.75 2.75h10.5v4.5H2.75ZM2.75 8.75h10.5v4.5H2.75ZM5 5h.01M5 11h.01',
    'eye-off': 'M1.5 8s2.3-4.25 6.5-4.25S14.5 8 14.5 8 12.2 12.25 8 12.25 1.5 8 1.5 8ZM8 9.75a1.75 1.75 0 1 0 0-3.5 1.75 1.75 0 0 0 0 3.5ZM2.75 13.25l10.5-10.5',
    refresh: 'M13 8a5 5 0 1 1-1.6-3.67M13 2.75v2.9H10.1',
    check: 'M3.25 8.5 6.5 11.75 12.75 4.5',
  };
  const PATHS24: Record<string, string> = {
    gear: 'M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2zM12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z',
  };
</script>

<script lang="ts">
  let { name, size = 16, class: cls = '' }: { name: string; size?: number; class?: string } = $props();
</script>

<svg class="icon {cls}" width={size} height={size} viewBox={name in PATHS24 ? '0 0 24 24' : '0 0 16 16'} fill="none" stroke="currentColor"
  stroke-width={name in PATHS24 ? 1.5 * 24 / size : 1.5 * 16 / size} stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  <path d={PATHS24[name] ?? PATHS[name] ?? ''} />
</svg>
