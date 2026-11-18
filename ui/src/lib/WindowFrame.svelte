<script lang="ts">
  // On Windows the window has no system title bar (shell.rs turns decorations off), so the app
  // draws its own: a drag strip along the top edge and the caption buttons in the corner. Linux
  // keeps its decorated window, and this renders nothing there.
  import { onMount } from 'svelte'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import Icon from './Icon.svelte'

  const win = getCurrentWindow()
  let framed = $state(false)
  let maxed = $state(false)

  onMount(() => {
    let gone = false
    let off: (() => void) | undefined
    ;(async () => {
      if (await win.isDecorated().catch(() => true)) return
      if (gone) return
      framed = true
      document.documentElement.dataset.frame = ''
      maxed = await win.isMaximized()
      const un = await win.onResized(async () => (maxed = await win.isMaximized()))
      if (gone) un()
      else off = un
    })()
    return () => {
      gone = true
      off?.()
      delete document.documentElement.dataset.frame
    }
  })
</script>

{#if framed}
  <div class="frame" data-tauri-drag-region>
    <button class="cap" aria-label="Minimise" title="Minimise" onclick={() => win.minimize()}>
      <Icon name="minimise" size={15} />
    </button>
    <!-- The Windows 11 snap-layout flyout attaches to this id (shell.rs). -->
    <button
      id="frame-max"
      class="cap"
      aria-label={maxed ? 'Restore' : 'Maximise'}
      title={maxed ? 'Restore' : 'Maximise'}
      onclick={() => win.toggleMaximize()}
    >
      <Icon name={maxed ? 'restore' : 'maximise'} size={15} />
    </button>
    <button class="cap" aria-label="Close" title="Close" onclick={() => win.close()}>
      <Icon name="close" size={15} />
    </button>
  </div>
{/if}

<style>
  /* Transparent: the rail and the ground run to the top edge, so the bar is felt, not seen. */
  .frame {
    position: fixed;
    inset: 0 0 auto 0;
    height: var(--frame-h);
    display: flex;
    justify-content: flex-end;
    z-index: 4;
  }
  .cap {
    width: 44px;
    height: 100%;
    display: grid;
    place-items: center;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--muted);
    cursor: default;
    transition:
      background var(--fast) var(--ease),
      color var(--fast) var(--ease);
  }
  .cap:hover,
  .cap:global(.is-hovered) {
    background: var(--raise);
    color: var(--text);
  }
  .cap:active {
    background: var(--line2);
  }
  .cap:focus-visible {
    outline-offset: -3px;
  }
</style>
