<script lang="ts">
  import { onMount, tick, untrack } from 'svelte'
  import type { Device } from './api'
  import Icon from './Icon.svelte'
  import { checkRuns, firstRuns, MAX_RUN_NAME, MAX_RUNS, nextRun, runsOf, spans, splittable, type Run } from './segments'

  let {
    device,
    name,
    zone: startZone,
    onsave,
    onclose,
  }: {
    device: Device
    /** The device's name as the window shows it. */
    name: string
    /** The zone to open on (the picked segment's), else the first that splits. */
    zone: number | null
    onsave: (zone: number, runs: Run[], before: Run[]) => Promise<void>
    onclose: () => void
  } = $props()

  // Opened for one device; later changes (the save itself) do not reset the rows.
  const zones = untrack(() => splittable(device))
  let zi = $state(untrack(() => Math.max(0, zones.findIndex((z) => z.index === startZone))))
  const zone = $derived(zones[zi])
  const was = $derived(runsOf(device, zone.index))
  let runs = $state<Run[]>(untrack(() => (runsOf(device, zones[zi].index).length ? runsOf(device, zones[zi].index) : firstRuns(zones[zi]))))
  let busy = $state(false)
  let error = $state('')

  const check = $derived(checkRuns(runs, zone))
  const span = $derived(spans(runs))
  const used = $derived(runs.reduce((a, r) => a + Math.max(r.leds, 0), 0))
  const changed = $derived(JSON.stringify(runs.map((r) => ({ name: r.name.trim(), leds: r.leds }))) !== JSON.stringify(was))
  const joining = $derived(runs.length === 0 && was.length > 0)

  function pickZone(i: number) {
    zi = i
    const r = runsOf(device, zones[i].index)
    runs = r.length ? r : firstRuns(zones[i])
  }

  let dialog: HTMLDialogElement
  let list = $state<HTMLOListElement>()
  onMount(() => {
    dialog.showModal()
    list?.querySelector('input')?.select()
  })

  function close() {
    dialog.close()
    onclose()
  }
  async function add() {
    runs = [...runs, nextRun(runs, zone)]
    await tick()
    const inputs = list?.querySelectorAll<HTMLInputElement>('input.nm')
    inputs?.[inputs.length - 1]?.select()
  }
  async function submit(e: SubmitEvent) {
    e.preventDefault()
    if (!check.ok || !changed || busy) return
    busy = true
    error = ''
    try {
      await onsave(
        zone.index,
        runs.map((r) => ({ name: r.name.trim(), leds: r.leds })),
        was,
      )
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
  aria-labelledby="seg-title"
  oncancel={(e) => {
    e.preventDefault()
    close()
  }}
>
  <form onsubmit={submit}>
    <header>
      <div>
        <h2 id="seg-title">Split into segments</h2>
        <p>
          Each segment of {name} becomes its own piece on the map, so parts of one strip can sit apart. Segments follow
          the wiring from the first LED; OpenRGB keeps them with the device.
        </p>
      </div>
      <button type="button" class="ibtn" aria-label="Close" onclick={close}><Icon name="close" /></button>
    </header>

    <div class="body">
      {#if zones.length > 1}
        <h3 id="seg-zone">Zone</h3>
        <div class="chips" role="group" aria-labelledby="seg-zone">
          {#each zones as z, i}
            <button type="button" class="chip" aria-pressed={i === zi} onclick={() => pickZone(i)}
              >{z.name} <span class="num">{z.leds}</span></button
            >
          {/each}
        </div>
      {/if}

      <h3 id="seg-list">Segments, in wiring order</h3>
      {#if runs.length}
        <ol class="list" aria-labelledby="seg-list" bind:this={list}>
          {#each runs as r, i}
            <li class="row">
              <span class="pos num">{i + 1}</span>
              <input
                class="text nm"
                bind:value={r.name}
                maxlength={MAX_RUN_NAME + 10}
                autocomplete="off"
                spellcheck="false"
                aria-label="Name of segment {i + 1}"
                aria-invalid={r.name.trim() === '' || r.name.trim().length > MAX_RUN_NAME}
              />
              <label class="count">
                <input
                  class="text num"
                  type="number"
                  min="1"
                  max={zone.leds}
                  step="1"
                  bind:value={r.leds}
                  aria-label="LEDs in segment {i + 1}"
                  aria-invalid={!Number.isInteger(r.leds) || r.leds < 1}
                />
                <span class="unit">LEDs</span>
              </label>
              <span class="span num" title="First and last LED">{r.leds >= 1 ? `${span[i][0]}–${span[i][1]}` : '–'}</span>
              <button type="button" class="ibtn" aria-label="Remove {r.name.trim() || `segment ${i + 1}`}" onclick={() => (runs = runs.filter((_, j) => j !== i))}>
                <Icon name="close" size={14} />
              </button>
            </li>
          {/each}
        </ol>
      {:else}
        <p class="none">{was.length ? `No segments: ${zone.name} goes back to one piece.` : 'No segments yet.'}</p>
      {/if}
      <div class="more">
        <button type="button" class="btn sm" disabled={runs.length >= MAX_RUNS || used >= zone.leds} onclick={add}>
          <Icon name="plus" size={13} />Add a segment
        </button>
      </div>
      <p class="hint" class:warn={!check.ok} role="status">
        {#if !check.ok}
          {check.why}
        {:else if runs.length === 0}
          &nbsp;
        {:else if check.rest > 0}
          LEDs {used + 1}–{zone.leds} ({check.rest}) stay with {name}.
        {:else}
          The segments use all {zone.leds} LEDs.
        {/if}
      </p>
      {#if error}<p class="error" role="alert">{error}</p>{/if}
    </div>

    <footer>
      <span class="grow"></span>
      <button type="button" class="btn" onclick={close}>Cancel</button>
      <button class="btn" disabled={busy || !check.ok || !changed}>{joining ? 'Join back' : was.length ? 'Save' : 'Split'}</button>
    </footer>
  </form>
</dialog>

<style>
  .sheet {
    position: fixed;
    inset: 0;
    margin: auto;
    width: min(480px, calc(100vw - 32px));
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
    padding: 14px 22px 16px;
  }
  h3 {
    margin: 10px 0 2px;
    font-size: 12px;
    font-weight: 500;
    color: var(--muted);
  }
  .chips .num {
    opacity: 0.7;
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
    gap: 8px;
    min-height: 48px;
    border-bottom: 1px solid var(--line);
  }
  .pos {
    flex: none;
    width: 18px;
    color: var(--muted);
  }
  .text {
    height: 32px;
    padding: 0 10px;
    border-radius: 8px;
    border: 1px solid var(--line2);
    background: var(--ground);
    color: var(--text);
    font: inherit;
    font-size: 13.5px;
  }
  .text:hover {
    border-color: var(--muted);
  }
  .text[aria-invalid='true'] {
    border-color: var(--text);
  }
  .nm {
    flex: 1;
    min-width: 0;
  }
  .count {
    flex: none;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .count .text {
    width: 68px;
    padding-right: 4px;
    font-variant-numeric: tabular-nums;
  }
  .unit {
    font-size: 12px;
    color: var(--muted);
  }
  .span {
    flex: none;
    width: 62px;
    font-size: 12px;
    color: var(--muted);
    text-align: right;
  }
  .row .ibtn {
    flex: none;
    width: 28px;
    height: 28px;
    border-color: transparent;
    color: var(--muted);
  }
  .row .ibtn:hover {
    color: var(--text);
  }
  .more {
    padding-top: 6px;
  }
  .none {
    font-size: 12.5px;
    color: var(--muted);
    padding: 6px 0;
  }
  .hint {
    min-height: 1.4em;
    font-size: 12px;
    color: var(--muted);
    text-wrap: pretty;
  }
  .hint.warn {
    color: var(--text);
  }
  footer {
    padding-top: 14px;
    padding-bottom: 18px;
    border-top: 1px solid var(--line);
  }
  .grow {
    flex: 1;
  }
  @media (max-width: 440px) {
    .span {
      display: none;
    }
  }
</style>
