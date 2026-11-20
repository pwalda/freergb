<script lang="ts">
  import { onMount } from 'svelte'
  import {
    autostart,
    fps,
    installUpdate,
    onUpdateProgress,
    resetAll,
    setAutostart,
    packaged,
    setFps,
    setUpdateChecks,
    updateChecks,
    type DeviceKey,
    type UpdateInfo,
  } from './api'
  import Icon from './Icon.svelte'

  let {
    update,
    hidden,
    onshow,
    ondiagnostics,
    onabout,
    onclose,
  }: {
    /** A newer freeRGB ready to install, if any. */
    update: UpdateInfo | null
    /** Devices freeRGB leaves alone; `away` when not connected now. */
    hidden: { key: DeviceKey; name: string; away: boolean }[]
    onshow: (key: DeviceKey) => Promise<void>
    /** Settings closes and the Diagnostics sheet takes its place. */
    ondiagnostics: () => void
    /** Settings closes and the About sheet takes its place. */
    onabout: () => void
    onclose: () => void
  } = $props()
  let showing = $state(false)
  async function show(key: DeviceKey) {
    showing = true
    error = ''
    try {
      await onshow(key)
    } catch (err) {
      error = String(err)
    } finally {
      showing = false
    }
  }

  /** null until the OS has answered. */
  let atLogin = $state<boolean | null>(null)
  /** null until the backend has answered. */
  let checking = $state<boolean | null>(null)
  /** The Store build updates through the Store: no check to switch. */
  let fromStore = $state(false)
  /** Frames per second of software effects; null until the backend has answered. */
  let rate = $state<number | null>(null)
  let busy = $state(false)
  let error = $state('')

  let dialog: HTMLDialogElement
  onMount(() => {
    dialog.showModal()
    autostart()
      .then((on) => (atLogin = on))
      .catch((e) => (error = String(e)))
    fps()
      .then((n) => (rate = n))
      .catch((e) => (error = String(e)))
    updateChecks()
      .then((on) => (checking = on))
      .catch((e) => (error = String(e)))
    packaged()
      .then((p) => (fromStore = p))
      .catch(() => {})
  })
  async function toggleChecks(e: Event) {
    const want = (e.currentTarget as HTMLInputElement).checked
    error = ''
    try {
      checking = await setUpdateChecks(want)
    } catch (err) {
      error = String(err)
      checking = !want
    }
  }
  async function pickRate(n: number) {
    if (n === rate) return
    const was = rate
    rate = n
    error = ''
    try {
      rate = await setFps(n)
    } catch (err) {
      error = String(err)
      rate = was
    }
  }
  function close() {
    dialog.close()
    onclose()
  }

  /** Download progress in percent; null while not installing, or before the size is known. */
  let installing = $state(false)
  let percent = $state<number | null>(null)
  async function install() {
    installing = true
    percent = null
    error = ''
    const off = await onUpdateProgress((p) => (percent = p))
    try {
      // On success the app restarts and this never returns.
      await installUpdate()
    } catch (err) {
      error = String(err)
    } finally {
      off()
      installing = false
    }
  }
  /** Asks once more in place, as Delete does in the profile sheet. */
  let confirmReset = $state(false)
  let resetting = $state(false)
  async function reset() {
    if (!confirmReset) {
      confirmReset = true
      return
    }
    resetting = true
    error = ''
    try {
      await resetAll()
      // Every panel starts over from what the backend now holds.
      location.reload()
    } catch (err) {
      error = String(err)
      resetting = false
      confirmReset = false
    }
  }
  async function toggle(e: Event) {
    const want = (e.currentTarget as HTMLInputElement).checked
    busy = true
    error = ''
    try {
      atLogin = await setAutostart(want)
    } catch (err) {
      error = String(err)
      atLogin = !want
    } finally {
      busy = false
    }
  }
</script>

<dialog
  class="sheet panel"
  bind:this={dialog}
  aria-labelledby="settings-title"
  oncancel={(e) => {
    e.preventDefault()
    close()
  }}
>
  <header>
    <h2 id="settings-title">Settings</h2>
    <button class="ibtn" aria-label="Close" onclick={close}><Icon name="close" /></button>
  </header>
  <div class="body">
    {#if update}
      <div class="update">
        <div class="row">
          <span>
            <span class="name">freeRGB {update.version} is ready</span>
            <span class="desc"
              >{installing
                ? percent === null
                  ? 'Downloading…'
                  : percent < 100
                    ? `Downloading · ${percent}%`
                    : 'Installing. freeRGB restarts by itself.'
                : 'freeRGB restarts to finish. Your lights pause for a moment.'}</span
            >
          </span>
          <button class="btn sm" disabled={installing} aria-busy={installing} onclick={install}>Install</button>
        </div>
        {#if installing}
          <span class="bar" class:unknown={percent === null} style:--p="{percent ?? 0}%" aria-hidden="true"></span>
        {/if}
        {#if update.notes}<p class="notes">{update.notes}</p>{/if}
      </div>
    {/if}
    <label class="row">
      <span>
        <span class="name">Start when I log in</span>
        <span class="desc">freeRGB starts in the tray, with no window, and brings back your last look.</span>
      </span>
      <input
        type="checkbox"
        role="switch"
        checked={atLogin ?? false}
        disabled={atLogin === null || busy}
        onchange={toggle}
      />
    </label>
    <div class="row">
      <span>
        <span class="name" id="rate-label">Animation smoothness</span>
        <span class="desc">60 frames a second looks smoother on fast effects and costs about twice the processor time.</span>
      </span>
      <span class="chips" role="group" aria-labelledby="rate-label">
        {#each [30, 60] as n (n)}
          <button class="chip" aria-pressed={rate === n} disabled={rate === null} onclick={() => pickRate(n)}>
            <span class="num">{n}</span> fps
          </button>
        {/each}
      </span>
    </div>
    {#if !fromStore}
      <label class="row">
        <span>
          <span class="name">Check for updates</span>
          <span class="desc">Once a day, from freeRGB’s GitHub page. Off, freeRGB contacts nothing outside this PC.</span>
        </span>
        <input type="checkbox" role="switch" checked={checking ?? false} disabled={checking === null} onchange={toggleChecks} />
      </label>
    {/if}
    {#if hidden.length > 0}
      <div class="hidden">
        <span class="name" id="hidden-label">Hidden devices</span>
        <span class="desc">freeRGB leaves these alone: no look lights them and they keep their own hardware mode.</span>
        <ul aria-labelledby="hidden-label">
          {#each hidden as h (JSON.stringify(h.key))}
            <li>
              <span class="who">{h.name}{#if h.away}<span class="desc">Not connected now</span>{/if}</span>
              <button class="btn sm" disabled={showing} onclick={() => show(h.key)}>Show</button>
            </li>
          {/each}
        </ul>
      </div>
    {/if}
    <div class="row">
      <span>
        <span class="name">Diagnostics</span>
        <span class="desc">OpenRGB’s log and the buses and USB devices it sees, for looking into a problem.</span>
      </span>
      <button
        class="btn sm"
        aria-label="Open Diagnostics"
        onclick={() => {
          dialog.close()
          ondiagnostics()
        }}>Open</button
      >
    </div>
    <div class="row">
      <span>
        <span class="name">About freeRGB</span>
        <span class="desc">Version, licences, and where freeRGB and OpenRGB come from.</span>
      </span>
      <button
        class="btn sm"
        aria-label="Open About freeRGB"
        onclick={() => {
          dialog.close()
          onabout()
        }}>Open</button
      >
    </div>
    <div class="row">
      <span>
        <span class="name">Start over</span>
        <span class="desc"
          >{confirmReset
            ? 'Profiles, My effects, groups, the desk map, the schedule, device names and these settings will be gone. This cannot be undone.'
            : 'Forget everything freeRGB keeps and stop starting at login. OpenRGB’s own settings stay as they are.'}</span
        >
      </span>
      <span class="acts">
        {#if confirmReset}<button class="btn sm" disabled={resetting} onclick={() => (confirmReset = false)}>Cancel</button>{/if}
        <button class="btn sm" disabled={resetting} onclick={reset}>{confirmReset ? 'Reset for good' : 'Reset'}</button>
      </span>
    </div>
    <p class="desc">Closing the window keeps freeRGB running in the tray. Quit it from the tray icon.</p>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
  </div>
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
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 20px 22px 0;
  }
  h2 {
    font-size: 20px;
    font-weight: 600;
    letter-spacing: -0.012em;
  }
  .body {
    display: grid;
    gap: 14px;
    padding: 18px 22px 22px;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 18px;
  }
  .row input,
  .row .btn,
  .row .chips,
  .row .acts {
    flex: none;
  }
  .acts {
    display: flex;
    gap: 6px;
  }
  .chips {
    flex-wrap: nowrap;
  }
  .name {
    display: block;
    font-size: 13.5px;
    font-weight: 500;
  }
  .hidden ul {
    list-style: none;
    margin: 8px 0 0;
    padding: 0;
  }
  .hidden li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding: 8px 0;
    border-top: 1px solid var(--line);
    font-size: 13.5px;
  }
  .who {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .hidden li .btn {
    flex: none;
  }
  /* Sits apart from the settings below: it is news, not a preference. */
  .update {
    display: grid;
    gap: 10px;
    padding-bottom: 14px;
    border-bottom: 1px solid var(--line);
  }
  .notes {
    margin: 0;
    max-height: 7.5em;
    overflow-y: auto;
    font-size: 12.5px;
    color: var(--muted);
    white-space: pre-line;
    text-wrap: pretty;
  }
  .bar {
    height: 2px;
    border-radius: 1px;
    background: linear-gradient(var(--lamp), var(--lamp)) 0 0 / var(--p) 100% no-repeat, var(--line2);
    transition: background-size var(--slow) var(--ease);
  }
  .bar.unknown {
    background: linear-gradient(90deg, transparent, var(--lamp), transparent) -40% 0 / 40% 100% no-repeat, var(--line2);
    animation: sweep 1.3s var(--ease) infinite;
  }
  @keyframes sweep {
    to {
      background-position: 140% 0, 0 0;
    }
  }
  .desc {
    display: block;
    font-size: 12.5px;
    color: var(--muted);
    text-wrap: pretty;
  }
</style>
