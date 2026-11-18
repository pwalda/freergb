<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import type { SavedEffect } from './api'
  import { effectLabel, type EffectSpec } from './effects'
  import Icon from './Icon.svelte'

  /** `save`: keep `effect` under a name. `edit`: rename, replace with `current`, export or
   *  delete a saved effect. */
  export type EffectSheetMode = { kind: 'save'; effect: EffectSpec } | { kind: 'edit'; name: string; current?: EffectSpec }

  let {
    mode,
    saved,
    onsave,
    onexport,
    ondelete,
    onclose,
  }: {
    mode: EffectSheetMode
    saved: SavedEffect[]
    onsave: (old: string | null, name: string, effect: EffectSpec) => Promise<void>
    /** Write the saved effect to a file the user picks; false when they cancelled. */
    onexport: (name: string) => Promise<boolean>
    ondelete: (name: string) => Promise<void>
    onclose: () => void
  } = $props()

  const MAX = 40
  // The sheet is opened for one effect; it does not follow later changes of `mode`.
  const editing = untrack(() => (mode.kind === 'edit' ? mode.name : null))
  const kept = untrack(() => saved.find((s) => s.name === editing))
  const fresh = untrack(() => (mode.kind === 'save' ? mode.effect : mode.current))
  const suggestion = () => {
    const label = effectLabel(fresh?.type ?? 'static')
    if (!saved.some((s) => s.name === label)) return label
    let n = 2
    while (saved.some((s) => s.name === `${label} ${n}`)) n++
    return `${label} ${n}`
  }
  let name = $state(editing ?? suggestion())
  let replace = $state(false)
  let confirmDelete = $state(false)
  let busy = $state(false)
  let error = $state('')

  const clean = $derived(name.trim())
  /** Another saved effect has this name: saving replaces it, renaming is refused. */
  const taken = $derived(clean !== editing && saved.some((s) => s.name === clean))
  const valid = $derived(clean.length > 0 && clean.length <= MAX)
  const changed = $derived(editing === null || clean !== editing || replace)

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
  async function run(f: () => Promise<unknown>, closeAfter = true) {
    busy = true
    error = ''
    try {
      if ((await f()) !== false && closeAfter) close()
    } catch (e) {
      error = String(e)
    } finally {
      busy = false
    }
  }
  function submit(e: SubmitEvent) {
    e.preventDefault()
    if (!valid || !changed || busy || (editing !== null && taken)) return
    const effect = editing === null || replace ? fresh : kept?.effect
    if (!effect) return
    void run(() => onsave(editing, clean, effect))
  }
  function remove() {
    if (editing === null) return
    if (!confirmDelete) return void (confirmDelete = true)
    void run(() => ondelete(editing))
  }
</script>

<dialog
  class="sheet panel"
  bind:this={dialog}
  aria-labelledby="effect-title"
  oncancel={(e) => {
    e.preventDefault()
    close()
  }}
>
  <form onsubmit={submit}>
    <header>
      <div>
        <h2 id="effect-title">{editing === null ? 'Keep this look' : 'Edit effect'}</h2>
        <p>
          {editing === null
            ? 'It goes on the My effects shelf, ready for any device or group. Export it to share it.'
            : `${effectLabel(kept?.effect.type ?? 'static')}, kept in My effects.`}
        </p>
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
          maxlength={MAX + 10}
          autocomplete="off"
          spellcheck="false"
          aria-invalid={!valid || (editing !== null && taken)}
          aria-describedby="effect-hint"
        />
      </label>
      <p id="effect-hint" class="hint" class:warn={taken || !valid}>
        {#if !valid}
          {clean.length === 0 ? 'Give it a name.' : `Keep it under ${MAX} characters.`}
        {:else if taken && editing === null}
          Replaces the effect “{clean}”.
        {:else if taken}
          Another effect is already called “{clean}”.
        {:else}&nbsp;{/if}
      </p>
      {#if editing !== null}
        {#if fresh}
          <label class="row">
            <input type="checkbox" role="switch" bind:checked={replace} />
            <span>Replace it with the look that is playing ({effectLabel(fresh.type)})</span>
          </label>
        {/if}
        <div class="share">
          <span>Share it as a file anyone with freeRGB can import.</span>
          <button type="button" class="btn sm" disabled={busy} onclick={() => editing && run(() => onexport(editing))}>Export…</button>
        </div>
      {/if}
      {#if error}<p class="error" role="alert">{error}</p>{/if}
    </div>

    <footer>
      {#if editing !== null}
        <button type="button" class="btn" disabled={busy} onclick={remove}>
          {confirmDelete ? 'Delete for good' : 'Delete'}
        </button>
      {/if}
      <span class="grow"></span>
      <button type="button" class="btn" onclick={close}>Cancel</button>
      <button class="btn" disabled={busy || !valid || !changed || (editing !== null && taken)}>
        {editing === null && taken ? 'Replace' : 'Save'}
      </button>
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
    gap: 10px;
    font-size: 13px;
    margin-top: 4px;
  }
  .row input {
    flex: none;
  }
  .share {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    margin-top: 10px;
    padding-top: 12px;
    border-top: 1px solid var(--line);
    font-size: 12.5px;
    color: var(--muted);
  }
  .share .btn {
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
