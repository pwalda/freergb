<script lang="ts">
  import type { Snippet } from 'svelte'
  import Icon from './Icon.svelte'

  let { label = 'More settings', children }: { label?: string; children: Snippet } = $props()
  let open = $state(false)
  const id = `fold-${Math.random().toString(36).slice(2, 8)}`
</script>

<!-- Settings one step deeper. Closed, the content stays laid out (so the drawer can glide open)
     but inert, so it takes no focus. -->
<div class="more wide" class:open>
  <button class="toggle" aria-expanded={open} aria-controls={id} onclick={() => (open = !open)}>
    <Icon name="right" size={14} />{label}
  </button>
  <div class="fold" {id} inert={!open}>
    <div><div class="inner">{@render children()}</div></div>
  </div>
</div>

<style>
  .toggle {
    border: 0;
    background: none;
    padding: 0;
    color: var(--muted);
    display: flex;
    gap: 6px;
    align-items: center;
    font-size: 13px;
    transition: color var(--fast);
  }
  .toggle:hover {
    color: var(--text);
  }
  .toggle :global(svg) {
    transition: transform var(--mid) var(--ease);
  }
  .open .toggle :global(svg) {
    transform: rotate(90deg);
  }
  .fold {
    display: grid;
    grid-template-rows: 0fr;
    transition: grid-template-rows var(--slow) var(--ease);
  }
  .open .fold {
    grid-template-rows: 1fr;
  }
  .fold > div {
    overflow: hidden;
  }
  .inner {
    display: grid;
    grid-template-columns: var(--ctl-cols, repeat(3, minmax(0, 1fr)));
    gap: 18px 28px;
    padding-top: 16px;
    opacity: 0;
    transform: translateY(-6px);
    transition:
      opacity var(--mid),
      transform var(--slow) var(--ease);
  }
  .open .inner {
    opacity: 1;
    transform: none;
    transition-delay: 80ms;
  }
</style>
