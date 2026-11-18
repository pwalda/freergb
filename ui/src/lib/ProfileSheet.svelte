<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import Icon from './Icon.svelte'

  /** `save`: a new profile from what is showing now. `edit`: rename, re-save or delete one. */
  export type ProfileSheetMode = { kind: 'save' } | { kind: 'edit'; name: string }

  let {
    mode,
    names,
    onsave,
    onrename,
    ondelete,
    onclose,
  }: {
    mode: ProfileSheetMode
    names: string[]
    /** Save what is showing now under this name (replacing a profile of that name). */
    onsave: (name: string) => Promise<void>
    onrename: (from: string, to: string) => Promise<void>
    ondelete: (name: string) => Promise<void>
    onclose: () => void
  } = $props()

  const MAX = 40
  // The sheet is opened for one profile; it does not follow later changes of `mode`.
  const editing = untrack(() => (mode.kind === 'edit' ? mode.name : null))
  const suggestion = () => {
    let n = names.length + 1
    while (names.includes(`Profile ${n}`)) n++
    return `Profile ${n}`
  }
  let name = $state(editing ?? suggestion())
  let resave = $state(false)
  let confirmDelete = $state(false)
  let busy = $state(false)
  let error = $state('')

  const clean = $derived(name.trim())
  /** Another profile already has this name: saving replaces it, renaming is refused. */
  const taken = $derived(clean !== editing && names.includes(clean))
  const valid = $derived(clean.length > 0 && clean.length <= MAX)
  const changed = $derived(editing === null || clean !== editing || resave)

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
  async function run(f: () => Promise<void>) {
    busy = true
    error = ''
    try {
      await f()
      close()
    } catch (e) {
      error = String(e)
    } finally {
      busy = false
    }
  }
  function submit(e: SubmitEvent) {
    e.preventDefault()
    if (!valid || !changed || busy || (editing !== null && taken)) return
    void run(async () => {
      if (editing === null) return onsave(clean)
      if (clean !== editing) await onrename(editing, clean)
      if (resave) await onsave(clean)
    })
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
  aria-labelledby="profile-title"
  oncancel={(e) => {
    e.preventDefault()
    close()
  }}
>
  <form onsubmit={submit}>
    <header>
      <div>
        <h2 id="profile-title">{editing === null ? 'Save as a profile' : 'Edit profile'}</h2>
        <p>
          {editing === null
            ? 'Keeps what every device shows now, so you can switch back to it here or from the tray.'
            : 'Rename it, or replace its look with what the devices show now.'}
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
          aria-describedby="profile-hint"
        />
      </label>
      <p id="profile-hint" class="hint" class:warn={taken || !valid}>
        {#if !valid}
          {clean.length === 0 ? 'Give it a name.' : `Keep it under ${MAX} characters.`}
        {:else if taken && editing === null}
          Replaces the profile “{clean}”.
        {:else if taken}
          Another profile is already called “{clean}”.
        {:else}&nbsp;{/if}
      </p>
      {#if editing !== null}
        <label class="row">
          <input type="checkbox" role="switch" bind:checked={resave} />
          <span>Replace its look with what the devices show now</span>
        </label>
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
  footer {
    padding-top: 14px;
    padding-bottom: 18px;
    border-top: 1px solid var(--line);
  }
  .grow {
    flex: 1;
  }
</style>
