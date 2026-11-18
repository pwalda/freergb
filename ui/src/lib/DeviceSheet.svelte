<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import type { Device, DeviceNote } from './api'
  import Icon from './Icon.svelte'

  let {
    device,
    note,
    onsave,
    onclose,
  }: {
    device: Device
    note: DeviceNote | undefined
    /** `name` null: the device's own name again. */
    onsave: (name: string | null, hidden: boolean) => Promise<void>
    onclose: () => void
  } = $props()

  const MAX = 40
  // The sheet is opened for one device; it does not follow later changes.
  const was = untrack(() => ({ name: note?.name ?? '', hidden: note?.hidden ?? false }))
  let name = $state(was.name)
  let hidden = $state(was.hidden)
  let busy = $state(false)
  let error = $state('')

  const clean = $derived(name.trim())
  const valid = $derived(clean.length <= MAX)
  const changed = $derived(clean !== was.name || hidden !== was.hidden)

  let dialog: HTMLDialogElement
  let input: HTMLInputElement
  onMount(() => {
    dialog.showModal()
    input.focus()
    input.select()
  })

  function close() {
    dialog.close()
    onclose()
  }
  async function submit(e: SubmitEvent) {
    e.preventDefault()
    if (!valid || !changed || busy) return
    busy = true
    error = ''
    try {
      await onsave(clean === '' ? null : clean, hidden)
      close()
    } catch (err) {
      error = String(err)
    } finally {
      busy = false
    }
  }
</script>

<dialog
  class="sheet panel"
  bind:this={dialog}
  aria-labelledby="device-title"
  oncancel={(e) => {
    e.preventDefault()
    close()
  }}
>
  <form onsubmit={submit}>
    <header>
      <div>
        <h2 id="device-title">Rename or hide</h2>
        <p>The name is freeRGB’s own; OpenRGB and other apps keep calling it “{device.name}”.</p>
      </div>
      <button type="button" class="ibtn" aria-label="Close" onclick={close}><Icon name="close" /></button>
    </header>

    <div class="body">
      <label class="field">
        <span class="label">Name <span class="num count">{clean.length}/{MAX}</span></span>
        <input
          class="text"
          bind:this={input}
          bind:value={name}
          placeholder={device.name}
          maxlength={MAX + 10}
          autocomplete="off"
          spellcheck="false"
          aria-invalid={!valid}
          aria-describedby="device-hint"
        />
      </label>
      <p id="device-hint" class="hint" class:warn={!valid}>
        {#if !valid}
          Keep it under {MAX} characters.
        {:else if clean === ''}
          Left blank, it goes by its own name.
        {:else}&nbsp;{/if}
      </p>
      <label class="row">
        <span>
          <span class="name">Hide from freeRGB</span>
          <span class="desc">It leaves the window and no look lights it; it keeps its own hardware mode. Bring it back from Settings.</span>
        </span>
        <input type="checkbox" role="switch" bind:checked={hidden} />
      </label>
      {#if error}<p class="error" role="alert">{error}</p>{/if}
    </div>

    <footer>
      <span class="grow"></span>
      <button type="button" class="btn" onclick={close}>Cancel</button>
      <button class="btn" disabled={busy || !valid || !changed}>{hidden && !was.hidden ? 'Hide' : 'Save'}</button>
    </footer>
  </form>
</dialog>

<style>
  .sheet {
    position: fixed;
    inset: 0;
    margin: auto;
    width: min(440px, calc(100vw - 32px));
    max-height: calc(100vh - 32px);
    padding: 0;
    border: 0;
    color: var(--text);
  }
  .sheet:not([open]) {
    display: none;
  }
  .sheet::backdrop {
    background: var(--scrim);
  }
  header,
  footer {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 0 22px;
  }
  header {
    justify-content: space-between;
    align-items: flex-start;
    padding-top: 20px;
  }
  header .ibtn {
    flex: none;
  }
  h2 {
    font-size: 20px;
    font-weight: 600;
    letter-spacing: -0.012em;
  }
  header p {
    font-size: 13px;
    color: var(--muted);
    text-wrap: pretty;
  }
  .body {
    display: grid;
    gap: 6px;
    padding: 18px 22px 16px;
  }
  .label {
    display: flex;
    justify-content: space-between;
    font-size: 13px;
  }
  .count {
    color: var(--muted);
  }
  .text {
    height: 34px;
    padding: 0 10px;
    border-radius: 8px;
    border: 1px solid var(--line2);
    background: var(--ground);
    color: var(--text);
    font: inherit;
    font-size: 14px;
  }
  .text:hover {
    border-color: var(--muted);
  }
  .hint {
    min-height: 1.4em;
    font-size: 12px;
    color: var(--muted);
  }
  .hint.warn {
    color: var(--text);
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 18px;
    margin-top: 6px;
  }
  .name {
    display: block;
    font-size: 13.5px;
    font-weight: 500;
  }
  .desc {
    display: block;
    font-size: 12.5px;
    color: var(--muted);
    text-wrap: pretty;
  }
  .row input {
    flex: none;
  }
  footer {
    padding-top: 14px;
    padding-bottom: 18px;
    border-top: 1px solid var(--line);
  }
  .grow {
    flex: 1;
  }
</style>
