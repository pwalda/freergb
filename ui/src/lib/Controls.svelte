<script lang="ts">
  import type { Snippet } from 'svelte'
  import { showAim } from './aim.svelte'
  import Dial from './Dial.svelte'
  import { AIMED, angleOf, colorSlots, EFFECTS, type Drive, type SceneParams, type SourceId } from './effects'
  import { DRIVE_LABEL, drivesFor, firstDrive, isSound, ranged, readout, shown, SOURCE_LABEL, stored, unitOf, usualRange } from './follow'
  import { sensing } from './readings.svelte'
  import Fold from './Fold.svelte'

  let {
    params = $bindable(),
    showCanvas = false,
    onchange,
    extra,
    more,
  }: {
    params: SceneParams
    /** "Same picture on every device": only meaningful for effects spanning several devices. */
    showCanvas?: boolean
    /** Called after any control changes, so a caller can send the new settings. */
    onchange?: () => void
    /** Rows shown after the main controls (a device's modes and shape). */
    extra?: Snippet
    /** More settings of the caller's own, inside the fold (zone sizes). */
    more?: Snippet
  } = $props()

  const info = $derived(EFFECTS.find((k) => k.id === params.kind)!)
  const colorCount = $derived(colorSlots(params))
  const react = $derived(params.react ?? null)
  const fromScreen = $derived(react?.source === 'screen')
  /** Sources offered: what this machine has, plus the one the look follows (shown even when
   *  it is missing here, so it is not silently dropped). */
  const offered = $derived([...new Set([...sensing.sources, ...(react ? [react.source] : [])])])
  const drives = $derived(react ? drivesFor(params.kind, react.source) : [])
  const reading = $derived(react ? readout(react.source, sensing.now) : null)
  const hasMore = $derived(info.spatial || info.moves || params.kind === 'gradient' || showCanvas || !!more || (!!react && ranged(react.source)))

  const fill = (v: number, min: number, max: number) => `${((v - min) / (max - min)) * 100}%`
  /** Amount is named as the look names it (Comets, Stars, Drops), as in More settings. */
  const driveLabel = (d: Drive) => (d === 'amount' ? (info.amount ?? 'Density') : DRIVE_LABEL[d])
  const aimed = $derived(AIMED.includes(params.kind))
  /** Reverse runs an aimed look the other way, so the dial shows where it really moves. */
  const flip = $derived(aimed && params.reverse ? 180 : 0)
  const angle = $derived(((params.angle ?? angleOf({ type: params.kind })) + flip) % 360)
  function follow(source: SourceId | '') {
    const drives = source ? firstDrive(params.kind, source) : null
    params.react = source && drives ? { source, drives } : null
    onchange?.()
  }
  function move(d: Drive) {
    if (!react) return
    params.react = { ...react, drives: d }
    onchange?.()
  }
  /** One end of the reading's range, in the unit shown; blank goes back to the usual end. */
  function setEnd(end: 'low' | 'high', raw: string) {
    if (!react) return
    const v = raw.trim() === '' ? undefined : Number(raw)
    if (v !== undefined && !Number.isFinite(v)) return
    const next = { ...react, [end]: v === undefined ? undefined : stored(react.source, v) }
    const [ul, uh] = usualRange(react.source)
    if ((next.low ?? ul) >= (next.high ?? uh)) return
    params.react = next
    onchange?.()
  }
  /** Sensitivity or strength, 0..100 on the slider; the usual value is kept as unset. */
  function tune(key: 'sensitivity' | 'strength', pct: number) {
    if (!react) return
    const v = pct / 100
    params.react = { ...react, [key]: v === (key === 'sensitivity' ? 0.5 : 1) ? undefined : v }
    onchange?.()
  }
  /** `v` is where the look moves; null goes back to the effect's own direction. */
  function aim(v: number | null) {
    if (v == null) {
      params.angle = null
      params.reverse = false
    } else {
      params.angle = (v - flip + 360) % 360
    }
    showAim(v ?? angleOf({ type: params.kind }))
    onchange?.()
  }
</script>

<!-- Laid out by the caller's .ctl grid: each field is one cell. -->
<div class="field">
  <span class="lbl">{react?.drives === 'color' && !fromScreen ? 'Colours, low to high' : colorCount === 1 ? 'Colour' : 'Colours'}</span>
  {#if fromScreen}
    <span class="note">From the screen</span>
  {:else if colorCount > 0}
    <div class="colors" role="group" aria-label="Colours">
      {#each params.colors.slice(0, colorCount) as _, i}
        <input type="color" bind:value={params.colors[i]} oninput={onchange} aria-label={`Colour ${i + 1}`} />
      {/each}
    </div>
  {:else}
    <span class="note">Follows the spectrum</span>
  {/if}
</div>

<label class="field">
  <span class="lbl">Brightness <b class="num">{Math.round(params.brightness * 100)}%</b></span>
  <input
    type="range"
    min="0"
    max="1"
    step="0.05"
    bind:value={params.brightness}
    oninput={onchange}
    style:--fill={fill(params.brightness, 0, 1)}
  />
</label>

{#if info.moves}
  <label class="field">
    <span class="lbl">Speed <b class="num">{params.speed.toFixed(2)}×</b></span>
    <input
      type="range"
      min="0"
      max="2"
      step="0.05"
      bind:value={params.speed}
      oninput={onchange}
      style:--fill={fill(params.speed, 0, 2)}
    />
  </label>
{/if}

{#if aimed}
  <div class="field">
    <span class="lbl">Direction <b class="num">{angle}°</b></span>
    <div class="aim">
      <Dial value={angle} label="Direction across the desk" onchange={aim} />
      {#if params.angle != null || flip}
        <button class="btn sm" onclick={() => aim(null)}>{params.kind === 'rain' ? 'Fall down' : params.kind === 'fire' ? 'Rise up' : 'Left to right'}</button>
      {/if}
    </div>
  </div>
{/if}

{#if offered.length > 0}
  <label class="field">
    <span class="lbl"
      >Follows {#if react}<b class="num" class:off={!reading}>{reading ?? 'No reading yet'}</b>{/if}</span
    >
    <span class="select">
      <select value={react?.source ?? ''} onchange={(e) => follow((e.currentTarget as HTMLSelectElement).value as SourceId | '')}>
        <option value="">Nothing</option>
        {#each offered as s}<option value={s} disabled={drivesFor(params.kind, s).length === 0}>{SOURCE_LABEL[s]}</option>{/each}
      </select>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m6 9 6 6 6-6" /></svg>
    </span>
    {#if react && !reading && sensing.needs[react.source]}<span class="hint">{sensing.needs[react.source]}.</span>{/if}
  </label>
  {#if react}
    <div class="field">
      <span class="lbl" id="follow-moves">Moves</span>
      {#if drives.length > 1}
        <div class="chips" role="group" aria-labelledby="follow-moves">
          {#each drives as d}
            <button class="chip" aria-pressed={react.drives === d} onclick={() => move(d)}>{driveLabel(d)}</button>
          {/each}
        </div>
      {:else}
        <span class="note">{react.source === 'screen' ? 'Each LED takes the screen colour where it sits on the desk' : react.source === 'spectrum' ? 'Each LED’s level follows its band, low notes first' : driveLabel(react.drives)}</span>
      {/if}
    </div>
    {#if isSound(react.source)}
      {@const sens = Math.round((react.sensitivity ?? 0.5) * 100)}
      {@const str = Math.round((react.strength ?? 1) * 100)}
      <label class="field">
        <span class="lbl">Sensitivity <b class="num">{sens}%</b></span>
        <input type="range" min="0" max="100" step="5" value={sens} oninput={(e) => tune('sensitivity', +e.currentTarget.value)} style:--fill={fill(sens, 0, 100)} />
      </label>
      <label class="field">
        <span class="lbl">Strength <b class="num">{str}%</b></span>
        <input type="range" min="0" max="100" step="5" value={str} oninput={(e) => tune('strength', +e.currentTarget.value)} style:--fill={fill(str, 0, 100)} />
      </label>
    {/if}
  {/if}
{/if}

{@render extra?.()}

{#if hasMore}
  <Fold>
    {#if info.spatial}
      <label class="field">
        <span class="lbl">{info.amount ?? 'Density'} <b class="num">{params.density.toFixed(2)}</b></span>
        <input
          type="range"
          min="0.25"
          max="8"
          step="0.25"
          bind:value={params.density}
          oninput={onchange}
          style:--fill={fill(params.density, 0.25, 8)}
        />
      </label>
    {/if}
    {#if info.moves || params.kind === 'gradient'}
      <label class="check">
        Reverse direction <input type="checkbox" role="switch" bind:checked={params.reverse} {onchange} />
      </label>
    {/if}
    {#if showCanvas}
      <label class="check">
        Same picture on every device <input type="checkbox" role="switch" bind:checked={params.perDevice} {onchange} />
      </label>
    {/if}
    {#if react && ranged(react.source)}
      {@const usual = usualRange(react.source)}
      <div class="field">
        <span class="lbl" id="follow-range">Reading range <b class="num">{unitOf(react.source)}</b></span>
        <div class="range" role="group" aria-labelledby="follow-range">
          <input
            class="num"
            type="number"
            aria-label="Low end"
            placeholder={String(shown(react.source, usual[0]))}
            value={react.low != null ? shown(react.source, react.low) : ''}
            onchange={(e) => setEnd('low', (e.currentTarget as HTMLInputElement).value)}
          />
          <span class="to">to</span>
          <input
            class="num"
            type="number"
            aria-label="High end"
            placeholder={String(shown(react.source, usual[1]))}
            value={react.high != null ? shown(react.source, react.high) : ''}
            onchange={(e) => setEnd('high', (e.currentTarget as HTMLInputElement).value)}
          />
        </div>
      </div>
    {/if}
    {@render more?.()}
  </Fold>
{/if}

<style>
  .off {
    color: var(--muted);
    font-weight: 400;
  }
  .range {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .range input {
    width: 76px;
    height: 32px;
    padding: 0 8px;
    border: 1px solid var(--line2);
    border-radius: 8px;
    background: transparent;
    color: var(--text);
    font-size: 13px;
  }
  .range input:hover {
    background: var(--raise);
  }
  .to {
    font-size: 12.5px;
    color: var(--muted);
  }
  .aim {
    display: flex;
    align-items: center;
    gap: 14px;
  }
</style>
