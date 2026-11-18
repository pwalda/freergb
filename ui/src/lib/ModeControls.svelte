<script lang="ts">
  import type { ModeView } from './api'
  import type { SceneParams } from './effects'
  import { modeColorCount } from './modes'

  let {
    params = $bindable(),
    mode,
    onchange,
  }: {
    params: SceneParams
    /** The device's own description of this mode: which settings it has and their ranges. */
    mode: ModeView
    onchange?: () => void
  } = $props()

  const pct = (v: number) => `${Math.round(v * 100)}%`
  const label = (d: string) => d.replace('_', ' ').replace(/^./, (c) => c.toUpperCase())
  const colorCount = $derived(modeColorCount(mode))
  const hasSettings = $derived(
    mode.speed !== null || mode.brightness !== null || mode.directions.length > 0 || colorCount > 0,
  )
</script>

<!-- Laid out by the caller's .ctl grid, like Controls. -->
{#if hasSettings}
  {#if colorCount > 0}
    <div class="field">
      <span class="lbl">{colorCount === 1 ? 'Colour' : 'Colours'}</span>
      <div class="colors" role="group" aria-label="Colours">
        {#each params.colors.slice(0, colorCount) as _, i}
          <input type="color" bind:value={params.colors[i]} oninput={onchange} aria-label={`Colour ${i + 1}`} />
        {/each}
      </div>
    </div>
  {/if}
  {#if mode.brightness !== null}
    <label class="field">
      <span class="lbl">Brightness <b class="num">{pct(params.brightness)}</b></span>
      <input
        type="range"
        min="0"
        max="1"
        step="0.01"
        bind:value={params.brightness}
        oninput={onchange}
        style:--fill={pct(params.brightness)}
      />
    </label>
  {/if}
  {#if mode.speed !== null}
    <label class="field">
      <span class="lbl">Speed <b class="num">{pct(params.speed)}</b></span>
      <input
        type="range"
        min="0"
        max="1"
        step="0.01"
        bind:value={params.speed}
        oninput={onchange}
        style:--fill={pct(params.speed)}
      />
    </label>
  {/if}
  {#if mode.directions.length > 0}
    <label class="field">
      <span class="lbl">Direction</span>
      <span class="select">
        <select bind:value={params.modeDirection} onchange={onchange}>
          {#each mode.directions as d}<option value={d}>{label(d)}</option>{/each}
        </select>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m6 9 6 6 6-6" /></svg>
      </span>
    </label>
  {/if}
{:else}
  <p class="hint wide">This mode runs on the device itself and has no settings.</p>
{/if}
