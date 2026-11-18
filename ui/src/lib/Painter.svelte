<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import type { Device } from './api'
  import type { Scene } from './effects'
  import { fill, layoutOf, near, OFF, paintedScene, paletteOf, startColors, step, withColor, type ZoneLayout } from './paint'
  import { withDeviceScene } from './scenes'
  import type { ToastMessage } from './Toast.svelte'
  import Icon from './Icon.svelte'

  let {
    device,
    own,
    scenes,
    edit,
    notify,
    onclose,
  }: {
    device: Device
    /** The device's own scene when the painter opened. */
    own: Scene | undefined
    scenes: Scene[]
    edit: (fn: (s: Scene[]) => Scene[]) => Promise<void>
    notify: (t: ToastMessage) => void
    onclose: () => void
  } = $props()

  // Captured once: Cancel and Undo go back to the list as it was before painting.
  const before = untrack(() => $state.snapshot(scenes)) as Scene[]
  const start = untrack(() => startColors(device, own))
  const brightness = untrack(() => (own?.effect.type === 'painted' ? (own.effect.brightness ?? 1) : 1))

  let colors = $state(start)
  let brush = $state(start.some((c) => c.toLowerCase() !== '#ff3b30') ? '#ff3b30' : '#0a84ff')
  /** The device's colours, then each new colour painted with, so a custom one stays pickable. */
  let palette = $state(paletteOf(start, 7))
  const swatches = $derived([...palette, OFF])
  const custom = $derived(!swatches.includes(brush.toLowerCase()))
  function used() {
    if (!near(brush, OFF)) palette = withColor(palette, brush)
  }

  const zones = $derived(
    device.zones.length > 0
      ? device.zones
      : [{ index: 0, name: 'LEDs', leds: colors.length, start: 0, matrix: null, min_leds: 0, max_leds: 0, resizable: false, modes: [], active_mode: -1 }],
  )

  let dialog: HTMLDialogElement
  // The device is the preview: send as the user paints, at most every 80 ms while dragging.
  let timer: ReturnType<typeof setTimeout> | undefined
  let dirty = false
  function send() {
    void edit((s) => withDeviceScene(s, device.key, paintedScene(device.key, $state.snapshot(colors), brightness)))
  }
  function push() {
    if (timer) return void (dirty = true)
    send()
    timer = setTimeout(() => {
      timer = undefined
      if (dirty) {
        dirty = false
        push()
      }
    }, 80)
  }
  function flush() {
    clearTimeout(timer)
    timer = undefined
    if (dirty) send()
    dirty = false
  }

  onMount(() => {
    dialog.showModal()
    dialog.querySelector<HTMLElement>('.swatch[aria-pressed="true"], .pick.on input')?.focus()
    // Taking over the device at once freezes what it showed, so painting starts from that.
    send()
    return () => clearTimeout(timer)
  })

  function paint(i: number) {
    if (colors[i] === brush) return
    colors[i] = brush
    used()
    push()
  }
  function fillRange(from: number, count: number) {
    colors = fill(colors, from, count, brush)
    used()
    push()
  }

  const ledAt = (x: number, y: number) => document.elementFromPoint(x, y)?.closest<HTMLElement>('[data-led]')
  /** The last pointer position of a drag; a fast drag jumps past LEDs between two events, so
   *  the whole segment since then is painted. */
  let last: [number, number] | null = null
  function move(e: PointerEvent) {
    if (!last) return
    const [x0, y0] = last
    const n = Math.max(1, Math.ceil(Math.hypot(e.clientX - x0, e.clientY - y0) / 8))
    for (let k = 1; k <= n; k++) {
      const el = ledAt(x0 + ((e.clientX - x0) * k) / n, y0 + ((e.clientY - y0) * k) / n)
      if (el) paint(Number(el.dataset.led))
    }
    last = [e.clientX, e.clientY]
  }
  function down(e: PointerEvent) {
    if (e.button !== 0) return
    ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
    const el = ledAt(e.clientX, e.clientY)
    if (el) paint(Number(el.dataset.led))
    last = [e.clientX, e.clientY]
  }

  // One tab stop per zone; arrow keys move between its LEDs.
  let cursor = $state<Record<number, number>>({})
  const first = (l: ZoneLayout) => (l.kind === 'strip' ? l.leds[0] : l.cells.find((c) => c !== null))
  const stop = (zone: number, l: ZoneLayout, led: number) => ((cursor[zone] ?? first(l)) === led ? 0 : -1)
  function keys(e: KeyboardEvent, zone: number, l: ZoneLayout) {
    const led = Number((e.target as HTMLElement).dataset.led)
    if (Number.isNaN(led)) return
    const to = step(l, led, e.key)
    if (to === led) return
    e.preventDefault()
    cursor[zone] = to
    ;(e.currentTarget as HTMLElement).querySelector<HTMLElement>(`[data-led="${to}"]`)?.focus()
  }
  const ledLabel = (led: number) => `LED ${led + 1}, ${colors[led] === OFF ? 'off' : colors[led]}`

  async function cancel() {
    clearTimeout(timer)
    timer = undefined
    dirty = false
    dialog.close()
    await edit(() => before)
    onclose()
  }
  function done() {
    flush()
    dialog.close()
    notify({ message: `Painting saved on ${device.name}`, undo: () => void edit(() => before) })
    onclose()
  }
</script>

<dialog
  class="sheet panel"
  bind:this={dialog}
  aria-labelledby="paint-title"
  oncancel={(e) => {
    e.preventDefault()
    void cancel()
  }}
>
  <header>
    <div>
      <h2 id="paint-title">Paint {device.name}</h2>
      <p>Click or drag over the LEDs. The device shows each change as you paint.</p>
    </div>
    <button class="ibtn" aria-label="Cancel painting" onclick={cancel}><Icon name="close" /></button>
  </header>

  <div class="tools">
    <span class="lbl" id="brush-label">Brush</span>
    <div class="swatches" role="group" aria-labelledby="brush-label">
      {#each swatches as c}
        <button
          class="swatch"
          class:off={c === OFF}
          style:--c={c}
          aria-pressed={brush.toLowerCase() === c}
          aria-label={c === OFF ? 'Off (black)' : c}
          title={c === OFF ? 'Off' : c}
          onclick={() => (brush = c)}
        ></button>
      {/each}
      <label class="swatch pick" class:on={custom} style:--brush={brush} title="Choose a colour">
        <input type="color" bind:value={brush} aria-label="Choose a brush colour" />
        {#if !custom}<Icon name="plus" size={14} />{/if}
      </label>
    </div>
    <span class="grow"></span>
    <button class="btn sm" onclick={() => fillRange(0, colors.length)}>Fill all</button>
  </div>

  <div class="zones">
    {#each zones as z (z.index)}
      {@const layout = layoutOf(z)}
      <section>
        <div class="zhead">
          <h3>{z.name}</h3>
          <span>{z.leds} LEDs</span>
          <span class="grow"></span>
          <button class="btn sm" aria-label={`Fill ${z.name} with the brush colour`} onclick={() => fillRange(z.start, z.leds)}>
            Fill zone
          </button>
        </div>
        <!-- The handlers serve the LED buttons inside: one drag paints across many of them. -->
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
        <div
          class="stage"
          role="group"
          aria-label={`LEDs of ${z.name}`}
          onpointerdown={down}
          onpointermove={move}
          onpointerup={() => (last = null)}
          onpointercancel={() => (last = null)}
          onkeydown={(e) => keys(e, z.index, layout)}
          onfocusin={(e) => {
            const led = Number((e.target as HTMLElement).dataset.led)
            if (!Number.isNaN(led)) cursor[z.index] = led
          }}
        >
          {#if layout.kind === 'matrix'}
            <div class="grid" style:--w={layout.width}>
              {#each layout.cells as led}
                {#if led === null}
                  <span></span>
                {:else}
                  <button
                    class="led key"
                    class:off={colors[led] === OFF}
                    data-led={led}
                    style:--c={colors[led]}
                    tabindex={stop(z.index, layout, led)}
                    aria-label={ledLabel(led)}
                    onclick={(e) => e.detail === 0 && paint(led)}
                  ></button>
                {/if}
              {/each}
            </div>
          {:else}
            <div class="strip">
              {#each layout.leds as led}
                <button
                  class="led dot"
                  class:off={colors[led] === OFF}
                  data-led={led}
                  style:--c={colors[led]}
                  tabindex={stop(z.index, layout, led)}
                  aria-label={ledLabel(led)}
                  onclick={(e) => e.detail === 0 && paint(led)}
                ></button>
              {/each}
            </div>
          {/if}
        </div>
      </section>
    {/each}
  </div>

  <footer>
    <button class="btn" onclick={cancel}>Cancel</button>
    <button class="btn" onclick={done}>Done</button>
  </footer>
</dialog>

<style>
  .sheet {
    position: fixed;
    inset: 0;
    margin: auto;
    width: min(920px, calc(100vw - 32px));
    max-height: calc(100vh - 32px);
    padding: 0;
    border: 0;
    color: var(--text);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .sheet:not([open]) {
    display: none;
  }
  .sheet::backdrop {
    background: var(--scrim);
  }
  header,
  .tools,
  footer {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 0 22px;
  }
  header {
    justify-content: space-between;
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
  }
  .tools {
    padding-top: 16px;
    padding-bottom: 14px;
    flex-wrap: wrap;
    border-bottom: 1px solid var(--line);
  }
  .lbl {
    font-size: 12px;
    font-weight: 560;
    color: var(--muted);
  }
  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .swatch {
    position: relative;
    width: 28px;
    height: 28px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: var(--c);
    box-shadow: inset 0 0 0 1px var(--led-edge);
    display: grid;
    place-items: center;
  }
  .swatch.off {
    background: transparent;
    box-shadow: inset 0 0 0 1.5px var(--muted);
  }
  .swatch.off::after {
    content: '';
    width: 1.5px;
    height: 18px;
    border-radius: 1px;
    background: var(--muted);
    transform: rotate(45deg);
  }
  .swatch[aria-pressed='true'],
  .swatch.pick.on {
    box-shadow:
      inset 0 0 0 1px var(--led-edge),
      0 0 0 2px var(--raise),
      0 0 0 4px var(--lamp);
  }
  .swatch.pick {
    background: none;
    box-shadow: inset 0 0 0 1px var(--line2);
    color: var(--muted);
    cursor: pointer;
  }
  .swatch.pick.on {
    background: var(--brush, none);
  }
  .swatch.pick input {
    position: absolute;
    inset: 0;
    opacity: 0;
    cursor: pointer;
    width: 100%;
    height: 100%;
  }
  .swatch.pick:has(input:focus-visible) {
    outline: 2px solid var(--lamp);
    outline-offset: 2px;
  }
  .grow {
    flex: 1;
  }
  .zones {
    overflow: auto;
    padding: 6px 22px 8px;
    display: grid;
    gap: 18px;
    scrollbar-color: var(--line2) transparent;
  }
  section {
    display: grid;
    gap: 10px;
    padding-top: 12px;
  }
  .zhead {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  .zhead .btn {
    align-self: center;
  }
  h3 {
    font-size: 15px;
    font-weight: 600;
  }
  .zhead > span:not(.grow) {
    font-size: 12px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .stage {
    padding: 16px;
    border-radius: 12px;
    background: var(--stage);
    box-shadow: inset 0 0 0 1px var(--line);
    touch-action: none;
    user-select: none;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(var(--w), minmax(0, 1fr));
    gap: 5px;
    max-width: calc(var(--w) * 38px);
  }
  .strip {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .led {
    padding: 0;
    border: 0;
    background: var(--c);
    box-shadow:
      inset 0 0 0 1px var(--led-edge),
      0 2px 8px -3px var(--c);
    cursor: crosshair;
  }
  .led:hover {
    box-shadow:
      inset 0 0 0 2px var(--text),
      0 2px 8px -3px var(--c);
  }
  .led.off {
    background: var(--led-off);
    box-shadow: inset 0 0 0 1px var(--line2);
  }
  .led.off:hover {
    box-shadow: inset 0 0 0 2px var(--muted);
  }
  .key {
    aspect-ratio: 1;
    border-radius: 5px;
  }
  .dot {
    width: 20px;
    height: 20px;
    border-radius: 50%;
  }
  footer {
    justify-content: flex-end;
    gap: 10px;
    padding-top: 14px;
    padding-bottom: 18px;
    border-top: 1px solid var(--line);
  }
  @media (max-width: 520px) {
    header,
    .tools,
    footer,
    .zones {
      padding-left: 14px;
      padding-right: 14px;
    }
    .stage {
      padding: 10px;
    }
    .grid {
      gap: 3px;
    }
  }
</style>
