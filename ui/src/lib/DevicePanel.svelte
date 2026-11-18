<script lang="ts">
  import { untrack, type Snippet } from 'svelte'
  import { saveMode, type Device, type SavedEffect } from './api'
  import { deviceKind } from './device-kind'
  import {
    buildEffect,
    buildScene,
    DEFAULT_PARAMS,
    EFFECTS,
    type ShelfId,
    effectLabel,
    paramsOf,
    type EffectKind,
    type EffectSpec,
    type Scene,
    type SceneParams,
  } from './effects'
  import { lookOfParams, modeNameOf, modeStart, modeValue, withZoneLook, zoneLookOf, zoneParams } from './modes'
  import { deviceScene, groupOf, groupScene, inheritedScene, sameScene, withDeviceScene, type Group } from './scenes'
  import { shortcut } from './shortcut'
  import type { ToastMessage } from './Toast.svelte'
  import Controls from './Controls.svelte'
  import Fold from './Fold.svelte'
  import Icon from './Icon.svelte'
  import ModeControls from './ModeControls.svelte'
  import Painter from './Painter.svelte'
  import Vials from './Vials.svelte'
  import ZoneSizes from './ZoneSizes.svelte'

  let {
    device,
    name,
    scenes,
    groups,
    theme,
    edit,
    notify,
    onerror,
    onlive,
    onrename,
    stage,
  }: {
    device: Device
    /** The device's name, numbered when several share it. */
    name: string
    scenes: Scene[]
    groups: Group[]
    theme: string
    edit: (fn: (s: Scene[]) => Scene[]) => Promise<void>
    notify: (t: ToastMessage) => void
    onerror: (message: string) => void
    onlive: (spec: EffectSpec | undefined) => void
    /** Open the rename-or-hide sheet. */
    onrename: () => void
    stage: Snippet
  } = $props()

  const kind = $derived(deviceKind(device.device_type))
  const own = $derived(deviceScene(scenes, device.key))
  const group = $derived(groupOf(groups, device.key))
  /** What the device follows: its group's look, else the all-devices look. */
  const shared = $derived(inheritedScene(scenes, groups, device.key))
  const inherited = $derived(own ? undefined : shared)
  const sharedFrom = $derived(group && groupScene(scenes, group.name) ? `“${group.name}”` : 'all devices')

  /** What the device is set to: 'none', an effect kind, 'painted', or `mode:<name>`. */
  let selected = $state('none')
  const selectedMode = $derived.by(() => {
    const n = modeNameOf(selected)
    return n === undefined ? undefined : device.modes.find((m) => m.name === n)
  })
  const isEffect = $derived(EFFECTS.some((e) => e.id === selected))
  let params = $state<SceneParams>({ ...DEFAULT_PARAMS, colors: [...DEFAULT_PARAMS.colors] })
  /** The scene this panel last sent or loaded; a different one from the backend is a change
   *  made elsewhere (Stop all, a look, another window) that the panel must follow. */
  let sent: Scene | undefined
  let timer: ReturnType<typeof setTimeout> | undefined

  function load(scene: Scene | undefined) {
    clearTimeout(timer)
    timer = undefined
    sent = scene
    if (scene) {
      params = paramsOf(scene)
      selected = scene.effect.type === 'device_mode' ? modeValue(scene.effect.mode ?? '') : scene.effect.type
    } else {
      selected = 'none'
    }
    syncZone()
  }

  /** Per-zone modes: the zones that may run a mode of their own under the picked built-in mode
   *  (none when that mode drives the whole device), and the one being edited, null for the
   *  whole device. */
  const hasZoneModes = $derived(device.zones.some((z) => z.modes.length > 0))
  const zoned = $derived(selectedMode && !selectedMode.whole_device ? device.zones.filter((z) => z.modes.length > 0) : [])
  let zoneSel = $state<number | null>(null)
  const zone = $derived(zoned.find((z) => z.index === zoneSel))
  const zoneLook = $derived(zone ? params.zones?.find((l) => l.zone === zone.index) : undefined)
  const zoneMode = $derived(zone && zoneLook ? zone.modes.find((m) => m.name === zoneLook.mode) : undefined)
  const zoneName = (z: { name: string; index: number }) => z.name || `Zone ${z.index + 1}`
  /** The zone's settings while it is edited; written back into `params.zones` on change. */
  let zp = $state<SceneParams>({ ...DEFAULT_PARAMS, colors: [] })
  function syncZone() {
    const l = zoneSel === null ? undefined : params.zones?.find((x) => x.zone === zoneSel)
    if (l) zp = zoneParams(l)
  }
  function selectZone(i: number | null) {
    zoneSel = i
    syncZone()
  }
  /** A zone mode by name, or null to have the zone follow the device's mode again. */
  function pickZoneMode(name: string | null) {
    if (zoneSel === null || !zone) return
    const m = name === null ? undefined : zone.modes.find((x) => x.name === name)
    if ((m?.name ?? null) === (zoneLook?.mode ?? null)) return
    params.zones = withZoneLook(params.zones, zoneSel, m ? zoneLookOf(zoneSel, m) : null)
    syncZone()
    commit()
  }
  function zoneChanged() {
    if (zoneSel === null) return
    params.zones = withZoneLook(params.zones, zoneSel, lookOfParams(zoneSel, $state.snapshot(zp)))
    later()
  }
  $effect(() => {
    const fromBackend = own
    untrack(() => {
      if (!sameScene(fromBackend, sent)) load(fromBackend)
    })
  })
  // Leaving the device with a slider edit still waiting: send it rather than lose it.
  $effect(() => () => {
    if (timer !== undefined) commit()
  })
  $effect(() => {
    const spec = isEffect ? buildEffect({ ...$state.snapshot(params), kind: selected as EffectKind }) : undefined
    untrack(() => onlive(spec))
  })
  $effect(() => () => onlive(undefined))

  /** The painter is open; it sends the device's scenes itself while it is. */
  let painting = $state(false)
  const isPainted = $derived(own?.effect.type === 'painted')
  /** A painted scene names every LED; the backend caps it (MAX_PAINTED in freergb-effects). */
  const paintable = $derived(device.colors.length > 0 && device.colors.length <= 4096)
  function closePainter() {
    painting = false
    load(own)
  }

  function commit() {
    clearTimeout(timer)
    timer = undefined
    let scene: Scene | undefined
    if (selected !== 'none') {
      const p = $state.snapshot(params)
      const n = modeNameOf(selected)
      if (n === undefined) p.kind = selected as SceneParams['kind']
      else {
        p.kind = 'device_mode'
        p.mode = n
      }
      scene = buildScene(p, { devices: [device.key] })
    }
    if (sameScene(scene, sent)) return
    sent = scene
    void edit((s) => withDeviceScene(s, device.key, scene ?? null))
  }

  // Sliders fire many times a second; send once they settle.
  function later() {
    clearTimeout(timer)
    timer = setTimeout(commit, 150)
  }

  /** `value`: 'none' (follow all devices / the device's own mode), an effect kind or `mode:<name>`. */
  function pick(value: string) {
    if (value === selected && value !== 'none') return
    const view = device.modes.find((m) => m.name === modeNameOf(value))
    if (view) {
      // Start from what the device reports for that mode; zones keep their own modes.
      const zones = params.kind === 'device_mode' ? params.zones : undefined
      params = { ...modeStart(view), ...(zones?.length ? { zones } : {}) }
    } else if (value !== 'none') {
      // Leaving a built-in mode or a painting: its colour list and direction mean nothing to an
      // effect. Start from the shared look's tuning when coming from nothing.
      if (params.kind === 'device_mode' || params.kind === 'painted' || selected === 'none')
        params = shared ? paramsOf(shared) : { ...DEFAULT_PARAMS, colors: [...DEFAULT_PARAMS.colors] }
      params.kind = value as SceneParams['kind']
    }
    selected = value
    commit()
  }

  /** Writing the built-in mode into the device's own memory. */
  let saving = $state(false)
  async function keepOnDevice() {
    commit()
    const effect = sent?.effect
    if (!effect || effect.type !== 'device_mode') return
    saving = true
    try {
      await saveMode(device.id, effect)
      notify({ message: `Saved “${effect.mode}” to ${name}. It stays when freeRGB is closed.` })
    } catch (e) {
      onerror(`Could not save to the device: ${e}`)
    } finally {
      saving = false
    }
  }

  /** A saved effect as this device's own look, exactly as it was kept. */
  function pickSaved(s: SavedEffect) {
    params = paramsOf({ effect: s.effect, targets: { devices: [device.key] }, canvas: 'shared' })
    selected = s.effect.type
    commit()
  }

  const ownName = $derived.by(() => {
    if (!own) return null
    const e = own.effect
    return e.type === 'device_mode' ? (e.mode ?? 'Built-in mode') : e.type === 'painted' ? 'Painted' : effectLabel(e.type)
  })
  const sub = $derived(
    `${kind.label} · ${device.vendor} · ${device.colors.length} LEDs · ${
      ownName
        ? `own look: ${ownName}`
        : inherited
          ? `follows ${sharedFrom} (${effectLabel(inherited.effect.type)})`
          : 'keeps its own hardware mode'
    }`,
  )
  let shelf = $state<ShelfId>('essentials')
  const sharedKind = $derived(inherited && EFFECTS.some((e) => e.id === inherited.effect.type) ? (inherited.effect.type as EffectKind) : null)
  const vialBase = $derived(isEffect ? params : shared ? paramsOf(shared) : undefined)
  const hasZones = $derived(device.zones.some((z) => z.resizable))
  /** The info panel's rows: what OpenRGB reports, empty ones left out. */
  const about = $derived(
    [
      { k: 'Type', v: kind.label },
      { k: 'Vendor', v: device.vendor },
      { k: 'Description', v: device.description },
      { k: 'Version', v: device.version, mono: true },
      { k: 'Serial', v: device.serial, mono: true },
      { k: 'Location', v: device.location, mono: true },
    ].filter((r) => (r.v ?? '').trim() !== ''),
  )
</script>

<svelte:window
  onkeydown={(e) =>
    shortcut(e, (n) => {
      if (n === 0) pick('none')
    })}
/>

<div class="head">
  <div class="in">
    <h1>{name}</h1>
    <p class="sub">{sub}</p>
  </div>
  <div class="acts">
    {#if own}
      <button class="btn" aria-keyshortcuts="0" onclick={() => pick('none')}>
        {inherited || shared ? `Follow ${sharedFrom}` : 'Back to the device’s own mode'}
      </button>
    {/if}
    <button class="btn" aria-label="Rename or hide" onclick={onrename}><Icon name="pencil" size={14} /><span class="long">Rename or hide</span></button>
  </div>
</div>

{@render stage()}

<Vials
  base={vialBase}
  active={isEffect ? (selected as EffectKind) : null}
  current={isEffect ? buildEffect({ ...params, kind: selected as EffectKind }) : undefined}
  activeTag="Own look"
  shared={sharedKind}
  {theme}
  bind:shelf
  onpick={pick}
  onpicksaved={pickSaved}
/>

{#snippet rows()}
  <div class="rowset wide">
    <h4 id="modes-label">Built-in modes of this device</h4>
    <div class="chips" role="group" aria-labelledby="modes-label">
      {#each device.modes as m}
        <button class="chip" aria-pressed={selected === modeValue(m.name)} onclick={() => pick(modeValue(m.name))}>{m.name}</button>
      {:else}
        <span class="hint">This device reports no modes of its own.</span>
      {/each}
      {#if paintable || isPainted}
        <span class="sep" aria-hidden="true"></span>
        <button class="btn sm" aria-pressed={isPainted} onclick={() => (painting = true)}>
          <Icon name="brush" size={14} />{isPainted ? 'Edit painting' : 'Paint by hand'}
        </button>
      {/if}
    </div>
  </div>
{/snippet}

{#snippet zones()}
  <ZoneSizes {device} {onerror} />
{/snippet}

<div class="ctl">
  {#if isEffect}
    <Controls bind:params onchange={later} extra={rows} more={hasZones ? zones : undefined} />
  {:else}
    {#if selectedMode}
      {#if zoned.length > 0}
        <div class="rowset wide">
          <h4 id="zones-label">Zones</h4>
          <div class="chips" role="group" aria-labelledby="zones-label">
            <button class="chip" aria-pressed={zone === undefined} onclick={() => selectZone(null)}>Whole device</button>
            {#each zoned as z}
              {@const look = params.zones?.find((l) => l.zone === z.index)}
              <button class="chip" aria-pressed={zone?.index === z.index} onclick={() => selectZone(z.index)}
                >{zoneName(z)}{#if look}<span class="own">{look.mode}</span>{/if}</button
              >
            {/each}
          </div>
        </div>
      {:else if hasZoneModes && selectedMode.whole_device}
        <p class="hint wide">{selectedMode.name} drives the whole device, so every zone follows it.</p>
      {/if}
      {#if zone}
        {#if zoneMode}
          <ModeControls bind:params={zp} mode={zoneMode} onchange={zoneChanged} />
        {:else if zoneLook}
          <p class="hint wide">{zoneName(zone)} does not report a mode called “{zoneLook.mode}” right now.</p>
        {:else}
          <p class="hint wide">{zoneName(zone)} follows the device’s mode ({selectedMode.name}). Pick one of its own modes below to set it apart.</p>
        {/if}
      {:else}
        <ModeControls bind:params mode={selectedMode} onchange={later} />
        {#if selectedMode.saveable}
          <div class="keep wide">
            <button class="btn sm" disabled={saving} onclick={keepOnDevice}><Icon name="save" size={14} />Save to device</button>
            <p class="hint">Keeps this mode on the device after freeRGB closes or the PC turns off.</p>
          </div>
        {/if}
      {/if}
    {:else if selected === 'painted'}
      <p class="hint wide">Every LED keeps the colour you painted.</p>
    {:else if modeNameOf(selected) !== undefined}
      <p class="hint wide">This device does not report a mode called “{modeNameOf(selected)}” right now.</p>
    {:else}
      <p class="hint wide">
        {inherited
          ? `This device follows the shared look (${effectLabel(inherited.effect.type)}). Pick a look above, a built-in mode or paint it by hand to give it its own.`
          : 'This device keeps its own hardware mode. Pick a look above, a built-in mode or paint it by hand to take it over.'}
      </p>
    {/if}
    {#if zone}
      <div class="rowset wide">
        <h4 id="zone-modes-label">Modes of {zoneName(zone)}</h4>
        <div class="chips" role="group" aria-labelledby="zone-modes-label">
          <button class="chip" aria-pressed={!zoneLook} onclick={() => pickZoneMode(null)}>Same as device</button>
          {#each zone.modes as m}
            <button class="chip" aria-pressed={zoneLook?.mode === m.name} onclick={() => pickZoneMode(m.name)}>{m.name}</button>
          {/each}
        </div>
      </div>
    {:else}
      {@render rows()}
    {/if}
    {#if hasZones}
      <Fold>{@render zones()}</Fold>
    {/if}
  {/if}
  <Fold label="About this device">
    <dl class="about wide">
      {#each about as r}
        <div><dt>{r.k}</dt><dd class:num={r.mono}>{r.v}</dd></div>
      {/each}
      <div>
        <dt>{device.zones.length === 1 ? 'Zone' : 'Zones'}</dt>
        <dd>
          {#each device.zones as z}
            <span class="zone">{z.name || `Zone ${z.index + 1}`}
              <span class="num">{z.leds} LEDs{z.matrix ? ` · ${z.matrix.width}×${z.matrix.height} grid` : ''}</span></span>
          {:else}
            <span class="num">None</span>
          {/each}
        </dd>
      </div>
    </dl>
  </Fold>
</div>

{#if painting}
  <Painter {device} {own} {scenes} {edit} {notify} onclose={closePainter} />
{/if}

<style>
  .acts {
    display: flex;
    gap: 8px;
    flex: none;
  }
  /* Narrow: the pencil alone, so the actions stay on the heading's row. */
  @media (max-width: 700px) {
    .long {
      display: none;
    }
  }
  .own {
    margin-left: 6px;
    opacity: 0.7;
  }
  .keep {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 14px;
  }
  .keep .hint {
    max-width: 52ch;
  }
  .about {
    margin: 0;
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    column-gap: 24px;
  }
  .about > div {
    display: grid;
    grid-template-columns: subgrid;
    grid-column: 1 / -1;
    padding: 9px 0;
    border-top: 1px solid var(--line);
  }
  .about > div:first-child {
    border-top: 0;
    padding-top: 0;
  }
  dt {
    font-size: 13px;
    color: var(--muted);
  }
  dd {
    margin: 0;
    font-size: 13px;
    overflow-wrap: anywhere;
    user-select: text;
  }
  dd.num {
    font-size: 12.5px;
  }
  .zone {
    display: block;
  }
  .zone + .zone {
    margin-top: 4px;
  }
  .zone .num {
    color: var(--muted);
    margin-left: 6px;
    white-space: nowrap;
  }
</style>
