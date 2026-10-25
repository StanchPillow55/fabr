<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';
  type Props = Omit<HTMLButtonAttributes, 'class'> & {
    variant?: 'plain' | 'tint' | 'primary' | 'icon' | 'danger';
    on?: boolean;
    class?: string;
    children: Snippet;
  };
  let { variant = 'tint', on = false, class: cls = '', type = 'button', children, ...rest }: Props = $props();
</script>

<button
  {type}
  class="btn {cls}"
  class:btn--tint={variant === 'tint' || variant === 'icon'}
  class:btn--primary={variant === 'primary'}
  class:btn--icon={variant === 'icon'}
  class:btn--danger={variant === 'danger'}
  class:btn--on={on}
  {...rest}
>
  {@render children()}
</button>
