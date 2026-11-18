<script lang="ts">
  import { onMount, tick } from 'svelte'
  import Icon from './Icon.svelte'
  import { clearServerLog, hardware, onServerLog, serverLog, turnOnServerLog, udevRules } from './api'
  import { ACCESS_DETAIL, ACCESS_TEXT, setUpAccess } from './access'
  import {
    clockOf,
    hardwareText,
    hex4,
    levelName,
    logText,
    merge,
    nameOf,
    pciId,
    severity,
    SHOW,
    shown,
    usbId,
    type Hardware,
    type LogLine,
    type LogState,
  } from './diagnostics.ts'

  let { onclose }: { onclose: () => void } = $props()

  let tab = $state<'log' | 'hw'>('log')
  let error = $state('')

  // ---- log
  let logState = $state<LogState | null>(null)
  let owned = $state(false)
  let lines = $state.raw<LogLine[]>([])
  let epoch: number | null = null
  let upTo = $state<number>(3)
  /** Turned on; waiting for the restarted OpenRGB to come back with its log. */
  let restarting = $state(false)
  let turning = $state(false)
  const RESTART_LIMIT = 90_000
  const visible = $derived(lines.filter((l) => shown(l, upTo)))

  let well = $state<HTMLElement>()
  let fetching = false
  let again = false
  async function pull() {
    if (fetching) {
      again = true
      return
    }
    fetching = true
    try {
      do {
        again = false
        const after = lines.at(-1)?.seq ?? 0
        const v = await serverLog(epoch, after)
        const stick = !well || well.scrollHeight - well.scrollTop - well.clientHeight < 24
        lines = merge(lines, epoch, v)
        epoch = v.epoch
        logState = v.state
        owned = v.owned
        if (v.state === 'on' || v.state === 'waiting') restarting = false
        if (stick) {
          await tick()
          well?.scrollTo({ top: well.scrollHeight })
        }
      } while (again)
    } catch (err) {
      error = String(err)
    } finally {
      fetching = false
    }
  }
  async function turnOn() {
    turning = true
    error = ''
    try {
      await turnOnServerLog()
      if (owned) {
        restarting = true
        // A restart that never brings the log back falls back to saying so.
        setTimeout(() => (restarting = false), RESTART_LIMIT)
      }
    } catch (err) {
      error = String(err)
    } finally {
      turning = false
    }
  }
  async function clear() {
    error = ''
    try {
      await clearServerLog()
    } catch (err) {
      error = String(err)
    }
  }

  // ---- hardware
  let hw = $state<Hardware | null>(null)
  let hwError = $state('')
  let hwBusy = $state(false)
  async function lookAgain() {
    hwBusy = true
    hwError = ''
    try {
      hw = await hardware()
    } catch (err) {
      hwError = String(err)
    } finally {
      hwBusy = false
    }
  }

  // ---- device access (Linux)
  let access = $state<boolean | null>(null)
  let settingUp = $state(false)
  async function setUp() {
    settingUp = true
    hwError = ''
    const err = await setUpAccess()
    settingUp = false
    if (err) {
      error = err
      return
    }
    access = true
    // OpenRGB's rescan is under way; its lists fill as it finds devices.
    setTimeout(() => void lookAgain(), 4000)
  }

  // ---- copy
  let copied = $state<'log' | 'hw' | null>(null)
  let copiedTimer: ReturnType<typeof setTimeout> | undefined
  async function copy(what: 'log' | 'hw') {
    const text = what === 'log' ? logText(visible) : hw ? hardwareText(hw) : ''
    error = ''
    try {
      if (navigator.clipboard) await navigator.clipboard.writeText(text)
      else fallbackCopy(text)
    } catch {
      try {
        fallbackCopy(text)
      } catch (err) {
        error = `Could not copy: ${err}`
        return
      }
    }
    copied = what
    clearTimeout(copiedTimer)
    copiedTimer = setTimeout(() => (copied = null), 1600)
  }
  /** Webviews without the async clipboard (or without permission for it). */
  function fallbackCopy(text: string) {
    const t = document.createElement('textarea')
    t.value = text
    t.setAttribute('readonly', '')
    t.style.position = 'fixed'
    t.style.opacity = '0'
    dialog.appendChild(t)
    t.select()
    const ok = document.execCommand('copy')
    t.remove()
    if (!ok) throw new Error('the clipboard is not available')
  }

  // ---- sheet
  let dialog: HTMLDialogElement
  onMount(() => {
    dialog.showModal()
    void pull()
    void udevRules().then((a) => (access = a), () => {})
    const off = onServerLog(() => void pull())
    return () => {
      clearTimeout(copiedTimer)
      void off.then((f) => f())
    }
  })
  function close() {
    dialog.close()
    onclose()
  }
  function pick(t: 'log' | 'hw') {
    tab = t
    if (t === 'hw' && !hw && !hwBusy) void lookAgain()
  }
  const TABS = [
    { id: 'log', label: 'Log' },
    { id: 'hw', label: 'Hardware' },
  ] as const
  function tabKey(e: KeyboardEvent, i: number) {
    const to = e.key === 'ArrowRight' || e.key === 'End' ? 1 : e.key === 'ArrowLeft' || e.key === 'Home' ? 0 : i
    if (to === i) return
    e.preventDefault()
    const id = TABS[to === 1 ? 1 : 0].id
    pick(id)
    document.getElementById(`diag-tab-${id}`)?.focus()
  }
</script>

{#snippet chevron()}
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m6 9 6 6 6-6" /></svg>
{/snippet}

{#snippet copyButton(what: 'log' | 'hw', disabled: boolean)}
  <button class="btn sm" {disabled} onclick={() => copy(what)}>
    <Icon name="copy" size={13} />{copied === what ? 'Copied' : 'Copy'}
  </button>
{/snippet}

<dialog
  class="sheet panel"
  bind:this={dialog}
  aria-labelledby="diag-title"
  oncancel={(e) => {
    e.preventDefault()
    close()
  }}
>
  <header>
    <div>
      <h2 id="diag-title">Diagnostics</h2>
      <p>What OpenRGB reports, for looking into a problem or writing a bug report. Nothing here changes your lights.</p>
    </div>
    <button class="ibtn" aria-label="Close" onclick={close}><Icon name="close" /></button>
  </header>

  <div class="bar">
    <div class="tabs" role="tablist" aria-label="Diagnostics">
      {#each TABS as t, i (t.id)}
        <button
          id="diag-tab-{t.id}"
          role="tab"
          aria-selected={tab === t.id}
          aria-controls="diag-pane"
          tabindex={tab === t.id ? 0 : -1}
          onclick={() => pick(t.id)}
          onkeydown={(e) => tabKey(e, i)}>{t.label}</button
        >
      {/each}
    </div>
    <div class="acts">
      {#if tab === 'log'}
        {#if logState === 'on'}
          <span class="select show">
            <select aria-label="Show" bind:value={upTo}>
              {#each SHOW as s (s.value)}<option value={s.value}>{s.label}</option>{/each}
            </select>
            {@render chevron()}
          </span>
          {@render copyButton('log', visible.length === 0)}
          <button class="btn sm" disabled={lines.length === 0} onclick={clear}>Clear</button>
        {/if}
      {:else}
        <button class="btn sm" disabled={hwBusy} onclick={lookAgain}><Icon name="rescan" size={13} />Look again</button>
        {@render copyButton('hw', !hw)}
      {/if}
    </div>
  </div>

  <div class="pane" id="diag-pane" role="tabpanel" aria-labelledby="diag-tab-{tab}">
    {#if tab === 'log'}
      {#if logState === 'on'}
        <div class="well" bind:this={well} tabindex="-1" aria-label="OpenRGB's log">
          {#if visible.length === 0}
            <p class="quiet">
              {lines.length === 0 ? 'No lines yet. OpenRGB adds them as it works.' : 'Nothing at this level. Pick Everything to see all lines.'}
            </p>
          {:else}
            <ol class="log">
              {#each visible as l (l.seq)}
                {@const mark = severity(l.level)}
                <li class:problem={mark !== null}>
                  <span class="t num">{clockOf(l.time_ms)}</span>
                  <span class="lv">{#if mark}<span class="mark {mark}" aria-hidden="true"></span>{/if}<span class="word">{levelName(l.level)}</span></span>
                  <span class="text">{l.text}</span>
                </li>
              {/each}
            </ol>
          {/if}
        </div>
      {:else if logState !== null}
        <div class="state">
          {#if restarting || logState === 'unavailable'}
            {#if restarting}
              <h3>Restarting OpenRGB…</h3>
              <p>Its log starts with this run. Your lights come back as soon as it answers.</p>
            {:else}
              <h3>OpenRGB is not connected here</h3>
              <p>The log is shown only while freeRGB is connected to OpenRGB on this computer.</p>
            {/if}
          {:else if logState === 'waiting'}
            <h3>Restart OpenRGB to start its log</h3>
            <p>The setting is saved. OpenRGB reads it when it starts, so close OpenRGB and start it again, then come back here.</p>
          {:else}
            <h3>OpenRGB is not keeping a log</h3>
            <p>
              It keeps one only while its “Enable Log Console” setting is on, and it reads that setting when it starts.
              {owned
                ? 'freeRGB started this OpenRGB, so it turns the setting on and restarts it; your lights may go dark for a moment.'
                : 'freeRGB turns the setting on; it takes effect the next time OpenRGB starts.'}
            </p>
            <button class="btn" disabled={turning} onclick={turnOn}>Turn on OpenRGB’s log</button>
          {/if}
        </div>
      {/if}
    {:else}
      {#if access === false}
        <div class="access">
          <p>{ACCESS_TEXT} <span class="sub">{ACCESS_DETAIL}</span></p>
          <button class="btn sm" disabled={settingUp} onclick={setUp}><Icon name="key" size={13} />Set up device access</button>
        </div>
      {/if}
      <div class="well hw" aria-busy={hwBusy}>
        {#if hwError}
          <p class="quiet">{hwError}</p>
        {:else if !hw}
          <p class="quiet">Asking OpenRGB…</p>
        {:else}
          <section aria-labelledby="hw-buses">
            <h3 id="hw-buses">SMBus buses <span class="num">{hw.buses.length}</span></h3>
            <ul>
              {#each hw.buses as b, i (i)}
                <li><span class="what">{b.name || 'Unnamed bus'}</span><span class="id num">{pciId(b) ?? ''}</span></li>
              {:else}
                <li class="none">None found. Memory and motherboard lights need these.</li>
              {/each}
            </ul>
          </section>
          <section aria-labelledby="hw-usb">
            <h3 id="hw-usb">USB devices <span class="num">{hw.usb.length}</span></h3>
            <ul>
              {#each hw.usb as d, i (i)}
                <li>
                  <span class="what">{nameOf(d)}{#if d.serial}<span class="sub num">{d.serial}</span>{/if}</span>
                  <span class="id num">{usbId(d.vendor, d.product)}</span>
                </li>
              {:else}
                <li class="none">None found.</li>
              {/each}
            </ul>
          </section>
          <section aria-labelledby="hw-hid">
            <h3 id="hw-hid">HID devices <span class="num">{hw.hid.length}</span></h3>
            <ul>
              {#each hw.hid as d, i (i)}
                <li>
                  <span class="what"
                    >{nameOf(d)}<span class="sub num"
                      >usage {hex4(d.usage_page)}:{hex4(d.usage)} · interface {d.interface}{#if d.path}<br />{d.path}{/if}</span
                    ></span
                  >
                  <span class="id num">{usbId(d.vendor, d.product)}</span>
                </li>
              {:else}
                <li class="none">None found.</li>
              {/each}
            </ul>
          </section>
          <section aria-labelledby="hw-serial">
            <h3 id="hw-serial">Serial ports <span class="num">{hw.serial.length}</span></h3>
            <ul>
              {#each hw.serial as s (s.port)}
                <li>
                  <span class="what num">{s.port}{#if s.usb_path}<span class="sub">USB port {s.usb_path}</span>{/if}</span>
                  <span class="id num">{s.vendor !== null && s.product !== null ? usbId(s.vendor, s.product) : ''}</span>
                </li>
              {:else}
                <li class="none">None found.</li>
              {/each}
            </ul>
          </section>
        {/if}
      </div>
    {/if}
  </div>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  <span class="sr" aria-live="polite">{copied ? 'Copied to the clipboard' : ''}</span>
</dialog>

<style>
  .sheet {
    position: fixed;
    inset: 0;
    margin: auto;
    width: min(820px, calc(100vw - 32px));
    /* A fixed height, so switching tabs or filling the log never moves the sheet. */
    height: min(640px, calc(100vh - 32px));
    padding: 0;
    border: 0;
    color: var(--text);
  }
  .sheet[open] {
    display: flex;
    flex-direction: column;
  }
  .sheet::backdrop {
    background: var(--scrim);
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 14px;
    padding: 20px 22px 0;
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
    max-width: 60ch;
    font-size: 13px;
    color: var(--muted);
    text-wrap: pretty;
  }
  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 10px 18px;
    padding: 14px 22px 10px;
  }
  .tabs {
    display: flex;
    gap: 18px;
  }
  /* The vial shelves' text tabs: muted, the open one in text colour over a short lamp rule. */
  .tabs button {
    position: relative;
    border: 0;
    background: none;
    padding: 6px 0;
    font: inherit;
    font-size: 13px;
    font-weight: 500;
    color: var(--muted);
    cursor: pointer;
    transition: color var(--fast) var(--ease);
  }
  .tabs button:hover,
  .tabs button[aria-selected='true'] {
    color: var(--text);
  }
  .tabs button[aria-selected='true']::after {
    content: '';
    position: absolute;
    left: 0;
    bottom: 1px;
    width: 14px;
    height: 2px;
    border-radius: 1px;
    background: var(--lamp);
  }
  .acts {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 28px;
  }
  .acts .btn {
    gap: 6px;
  }
  .show {
    width: 128px;
  }
  .show select {
    height: 28px;
    font-size: 12.5px;
  }
  .pane {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 0 22px 22px;
  }
  .well {
    flex: 1;
    min-height: 0;
    overflow: auto;
    border-radius: 12px;
    background: var(--stage);
    box-shadow: inset 0 0 0 1px var(--line);
    padding: 10px 0;
    outline: none;
    user-select: text;
  }
  .quiet {
    padding: 8px 16px;
    font-size: 13px;
    color: var(--muted);
    text-wrap: pretty;
  }
  .log {
    list-style: none;
    margin: 0;
    padding: 0;
    font-family: var(--mono);
    font-size: 12px;
    line-height: 1.55;
  }
  .log li {
    display: grid;
    grid-template-columns: 9ch 9ch minmax(0, 1fr);
    gap: 14px;
    padding: 1px 16px;
    color: var(--muted);
  }
  .log li:hover {
    background: color-mix(in srgb, var(--raise) 70%, transparent);
  }
  .log .t {
    font-size: inherit;
    text-align: right;
  }
  .log .text {
    color: var(--text);
    overflow-wrap: anywhere;
    white-space: pre-wrap;
  }
  .log .problem .lv {
    color: var(--text);
  }
  /* One line tall, so the mark sits on the first line of a wrapped message. */
  .lv {
    align-self: start;
    height: 1.55em;
    display: flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
  }
  /* The rail’s status dot: state, not decoration. */
  .mark {
    flex: none;
    width: 6px;
    height: 6px;
    border-radius: 50%;
  }
  .mark.bad {
    background: var(--bad);
  }
  .mark.warn {
    background: var(--warn);
  }
  .state {
    margin: auto 0;
    padding: 0 0 40px;
    max-width: 52ch;
    display: grid;
    gap: 8px;
    justify-items: start;
  }
  .state h3 {
    font-size: 15px;
    font-weight: 600;
  }
  .state p {
    font-size: 13px;
    color: var(--muted);
    text-wrap: pretty;
  }
  .state .btn {
    margin-top: 8px;
  }
  .hw {
    padding: 6px 16px 14px;
  }
  .access {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px 18px;
    flex-wrap: wrap;
    padding: 0 0 12px;
    font-size: 13px;
  }
  .access p {
    flex: 1 1 36ch;
    max-width: 68ch;
    text-wrap: pretty;
  }
  .access .btn {
    gap: 6px;
    flex: none;
  }
  .hw section + section {
    margin-top: 14px;
  }
  .hw h3 {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 8px 0 6px;
    font-size: 13.5px;
    font-weight: 500;
  }
  .hw h3 .num {
    color: var(--muted);
  }
  .hw ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .hw li {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 18px;
    padding: 7px 0;
    border-top: 1px solid var(--line);
    font-size: 13px;
  }
  .what {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .sub {
    display: block;
    margin-top: 2px;
    font-size: 11.5px;
    color: var(--muted);
  }
  .id {
    flex: none;
    color: var(--muted);
  }
  .none {
    color: var(--muted);
  }
  .error {
    margin: 0 22px 18px;
  }
  @media (max-width: 700px) {
    /* Only problems keep their level word (dot and word: never colour alone); the usual
       levels leave room for the text and stay for screen readers. */
    .log li {
      gap: 10px;
    }
    .log li:not(.problem) .word {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
      white-space: nowrap;
    }
  }
</style>
