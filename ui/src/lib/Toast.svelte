<script lang="ts">
  import Icon from './Icon.svelte'

  export interface ToastMessage {
    message: string
    undo?: () => void
  }

  let { toast, onclose }: { toast: ToastMessage | null; onclose: () => void } = $props()

  // Seven seconds, paused while the pointer or focus is on the toast so Undo stays reachable.
  let held = $state(false)
  let left = 7000
  $effect(() => {
    void toast
    left = 7000
    // A removed toast never sees pointerleave; each new one starts unheld.
    held = false
  })
  $effect(() => {
    if (!toast || held) return
    const start = performance.now()
    const timer = setTimeout(onclose, left)
    return () => {
      clearTimeout(timer)
      left = Math.max(0, left - (performance.now() - start))
    }
  })
  function undo() {
    toast?.undo?.()
    onclose()
  }
  // Ctrl+Z (Cmd+Z) undoes while the toast offers it, unless the user is typing in a field.
  function keydown(e: KeyboardEvent) {
    if (!toast?.undo || e.key.toLowerCase() !== 'z' || !(e.ctrlKey || e.metaKey) || e.shiftKey || e.altKey) return
    const t = e.target
    if (t instanceof HTMLElement && t.closest('input, select, textarea, [contenteditable], dialog')) return
    if (document.querySelector('dialog[open]')) return
    e.preventDefault()
    undo()
  }
</script>

<svelte:window onkeydown={keydown} />

<div class="slot" role="status" aria-live="polite">
  {#if toast}
    {#key toast}
      <!-- The line along the bottom drains over the seven seconds the toast stays. -->
      <div
        class="toast"
        class:held
        role="group"
        onpointerenter={() => (held = true)}
        onpointerleave={() => (held = false)}
        onfocusin={() => (held = true)}
        onfocusout={() => (held = false)}
      >
        <span>{toast.message}</span>
        {#if toast.undo}
          <button class="btn sm" aria-keyshortcuts="Control+Z" onclick={undo}><Icon name="undo" size={14} />Undo</button>
        {/if}
      </div>
    {/key}
  {/if}
</div>

<style>
  .slot {
    position: fixed;
    left: var(--rail-w, 0px);
    right: 0;
    bottom: 18px;
    display: flex;
    justify-content: center;
    padding: 0 16px;
    pointer-events: none;
    z-index: 3;
  }
  .toast {
    position: relative;
    display: flex;
    align-items: center;
    gap: 14px;
    max-width: 100%;
    padding: 6px 6px 6px 14px;
    border-radius: 11px;
    background: var(--raise);
    box-shadow:
      inset 0 0 0 1px var(--line2),
      0 18px 36px -18px var(--shadow);
    font-size: 13px;
    overflow: hidden;
    pointer-events: auto;
    animation: rise var(--slow) var(--ease);
  }
  .toast:not(:has(.btn)) {
    padding: 10px 14px;
  }
  .toast::after {
    content: '';
    position: absolute;
    left: 0;
    bottom: 0;
    height: 2px;
    width: 100%;
    background: var(--lamp);
    opacity: 0.5;
    transform-origin: left;
    animation: drain 7s linear forwards;
  }
  .toast.held::after {
    animation-play-state: paused;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(16px) scale(0.96);
    }
  }
  @keyframes drain {
    to {
      transform: scaleX(0);
    }
  }
</style>
