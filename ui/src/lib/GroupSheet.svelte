<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import type { Device } from './api'
  import type { DeviceKey } from './effects'
  import { deviceKind } from './device-kind'
  import Icon from './Icon.svelte'
  import { groupOf, keyId, sameKey, type Group } from './scenes'

  /** `new`: a group from scratch. `edit`: rename, re-order, change devices or delete one. */
  export type GroupSheetMode = { kind: 'new' } | { kind: 'edit'; name: string }

  let {
    mode,
    groups,
    devices,
    names,
    hidden = [],
    onsave,
    ondelete,
    onclose,
  }: {
    mode: GroupSheetMode
    groups: Group[]
    devices: Device[]
    /** Display names, parallel to `devices`. */
    names: string[]
    /** Devices the user hid; a member among them is named and marked as hidden. */
    hidden?: { key: DeviceKey; name: string }[]
    onsave: (old: string | null, name: string, devices: DeviceKey[]) => Promise<void>
    ondelete: (name: string) => Promise<void>
    onclose: () => void
  } = $props()

  const MAX = 40
  // The sheet is opened for one group; it does not follow later changes of `mode`.
  const editing = untrack(() => (mode.kind === 'edit' ? mode.name : null))
  const original = untrack(() => groups.find((g) => g.name === editing))
  const suggestion = () => {
    let n = groups.length + 1
    while (groups.some((g) => g.name === `Group ${n}`)) n++
    return `Group ${n}`
  }
  let name = $state(editing ?? suggestion())
  /** Members, left to right. Devices that are not connected now stay in their place. */
  let members = $state<DeviceKey[]>(untrack(() => [...(original?.devices ?? [])]))
  let confirmDelete = $state(false)
  let busy = $state(false)
  let error = $state('')

  const clean = $derived(name.trim())
  const taken = $derived(clean !== editing && groups.some((g) => g.name === clean))
  const valid = $derived(clean.length > 0 && clean.length <= MAX)
  const enough = $derived(members.length >= 2)

  const indexOf = (k: DeviceKey) => devices.findIndex((d) => sameKey(d.key, k))
  const hiddenOf = (k: DeviceKey) => hidden.find((h) => sameKey(h.key, k))
  const labelOf = (k: DeviceKey) => {
    const i = indexOf(k)
    return i >= 0 ? names[i] : (hiddenOf(k)?.name ?? k.name)
  }
  /** Devices to add: connected, not in this group; those in another group cannot join. */
  const others = $derived(
    devices
      .map((d, i) => ({ d, i, other: groupOf(groups, d.key) }))
      .filter(({ d }) => !members.some((k) => sameKey(k, d.key)))
      .map((o) => ({ ...o, other: o.other && o.other.name !== editing ? o.other : undefined })),
  )

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
  function move(at: number, by: number, e: MouseEvent) {
    const to = at + by
    if (to < 0 || to >= members.length) return
    const next = [...members]
    ;[next[at], next[to]] = [next[to], next[at]]
    members = next
    // Keep focus on the same arrow of the row that moved, so repeated presses keep moving it.
    const which = by < 0 ? 'up' : 'down'
    const row = (e.currentTarget as HTMLElement).closest('ol')
    requestAnimationFrame(() => row?.querySelectorAll<HTMLButtonElement>(`button.${which}`)[to]?.focus())
  }
  function submit(e: SubmitEvent) {
    e.preventDefault()
    if (!valid || taken || !enough || busy) return
    void run(() => onsave(editing, clean, $state.snapshot(members)))
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
  aria-labelledby="group-title"
  oncancel={(e) => {
    e.preventDefault()
    close()
  }}
>
  <form onsubmit={submit}>
    <header>
      <div>
        <h2 id="group-title">{editing === null ? 'New group' : 'Edit group'}</h2>
        <p>A group's devices play one look as a single surface, left to right in this order.</p>
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
          aria-invalid={!valid || taken}
          aria-describedby="group-hint"
        />
      </label>
      <p id="group-hint" class="hint" class:warn={taken || !valid}>
        {#if !valid}
          {clean.length === 0 ? 'Give it a name.' : `Keep it under ${MAX} characters.`}
        {:else if taken}
          Another group is already called “{clean}”.
        {:else}&nbsp;{/if}
      </p>

      <h3 id="members-label">In this group, left to right</h3>
      {#if members.length}
        <ol class="list" aria-labelledby="members-label">
          {#each members as k, i (keyId(k))}
            {@const at = indexOf(k)}
            <li class="row">
              <span class="pos num">{i + 1}</span>
              <span class="who">
                <span class="nm">{labelOf(k)}</span>
                <span class="kd">{at >= 0 ? `${deviceKind(devices[at].device_type).label} · ${devices[at].colors.length} LEDs` : hiddenOf(k) ? 'Hidden' : 'Not connected now'}</span>
              </span>
              <button type="button" class="ibtn up" aria-label="Move {labelOf(k)} left" disabled={i === 0} onclick={(e) => move(i, -1, e)}>
                <Icon name="up" size={14} />
              </button>
              <button type="button" class="ibtn down" aria-label="Move {labelOf(k)} right" disabled={i === members.length - 1} onclick={(e) => move(i, 1, e)}>
                <Icon name="down" size={14} />
              </button>
              <button type="button" class="ibtn" aria-label="Take {labelOf(k)} out of the group" onclick={() => (members = members.filter((_, j) => j !== i))}>
                <Icon name="close" size={14} />
              </button>
            </li>
          {/each}
        </ol>
      {:else}
        <p class="none">Add at least two devices below.</p>
      {/if}

      {#if others.length}
        <h3 id="others-label">Add devices</h3>
        <ul class="list" aria-labelledby="others-label">
          {#each others as o (keyId(o.d.key))}
            <li class="row" class:off={o.other}>
              <span class="who">
                <span class="nm">{names[o.i]}</span>
                <span class="kd">{o.other ? `In “${o.other.name}”` : `${deviceKind(o.d.device_type).label} · ${o.d.colors.length} LEDs`}</span>
              </span>
              <button
                type="button"
                class="btn sm"
                disabled={!!o.other}
                aria-label={o.other ? `${names[o.i]} is in “${o.other.name}”` : `Add ${names[o.i]}`}
                onclick={() => (members = [...members, o.d.key])}
              >
                <Icon name="plus" size={13} />Add
              </button>
            </li>
          {/each}
        </ul>
      {/if}
      {#if !enough && members.length > 0}<p class="hint warn">A group needs at least two devices.</p>{/if}
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
      <button class="btn" disabled={busy || !valid || taken || !enough}>{editing === null ? 'Create group' : 'Save'}</button>
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
  form {
    display: flex;
    flex-direction: column;
    max-height: calc(100vh - 32px);
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
    overflow: auto;
    min-height: 0;
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
  h3 {
    margin: 10px 0 2px;
    font-size: 12px;
    font-weight: 500;
    color: var(--muted);
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    border-top: 1px solid var(--line);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 44px;
    border-bottom: 1px solid var(--line);
  }
  .pos {
    flex: none;
    width: 18px;
    color: var(--muted);
  }
  .who {
    flex: 1;
    min-width: 0;
    display: grid;
  }
  .nm {
    font-size: 13.5px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .kd {
    font-size: 12px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .row .ibtn {
    flex: none;
    width: 28px;
    height: 28px;
    border-color: transparent;
    color: var(--muted);
  }
  .row .ibtn:hover:not(:disabled) {
    color: var(--text);
  }
  .row .ibtn:disabled {
    opacity: 0.3;
  }
  .row.off .nm {
    color: var(--muted);
  }
  .none {
    font-size: 12.5px;
    color: var(--muted);
    padding: 6px 0;
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
