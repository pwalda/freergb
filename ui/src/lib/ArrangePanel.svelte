<script lang="ts">
  import type { Snippet } from 'svelte'
  import type { Draft } from './desk'
  import Icon from './Icon.svelte'
  import { SHAPES, type Shape } from './rig'

  let {
    counts,
    names,
    drafts,
    sel,
    onturn,
    onshape,
    onlength,
    split,
    onsplit,
    oncancel,
    ondone,
    stage,
  }: {
    /** Per piece on the map (a device, or one of its segments): LED count, name, drawing. */
    counts: number[]
    names: string[]
    drafts: Draft[]
    /** The piece picked on the map, -1 for none. */
    sel: number
    onturn: (i: number) => void
    onshape: (i: number, s: Shape) => void
    onlength: (i: number, n: number) => void
    /** Per piece: how its device's segments are offered ("Split into segments" or "Edit
     *  segments"), null when it has no zone that splits. */
    split: (string | null)[]
    onsplit: (i: number) => void
    oncancel: () => void
    ondone: () => void
    stage: Snippet
  } = $props()

  const fresh = $derived(drafts.filter((d) => !d.spot).length)
  const draft = $derived(drafts[sel])
  const fill = (v: number, min: number, max: number) => `${((v - min) / (max - min)) * 100}%`
  const MAX_LEN = 60
</script>

<div class="head">
  <div class="in">
    <h1>Arrange your desk</h1>
    <p class="sub">
      Drag each device to where it sits; effects then sweep across the room as laid out here.{fresh > 0
        ? ` ${fresh} ${fresh === 1 ? 'piece is' : 'pieces are'} not placed yet.`
        : ''}
    </p>
  </div>
  <div class="acts">
    <button class="btn" onclick={oncancel}>Cancel</button>
    <button class="btn" onclick={ondone}>Done</button>
  </div>
</div>

{@render stage()}

<div class="ctl">
  {#if draft}
    <div class="field picked">
      <span class="lbl">Picked</span>
      <span class="who">{names[sel]} <span class="num">{counts[sel]} LEDs</span></span>
    </div>
    <div class="field">
      <span class="lbl">Turn <b class="num">{draft.spot?.rot ?? 0}°</b></span>
      <div class="acts">
        <button class="btn sm" onclick={() => onturn(sel)}><Icon name="turn" size={14} />Turn 90°</button>
        {#if split[sel]}
          <button class="btn sm" onclick={() => onsplit(sel)}><Icon name="split" size={14} />{split[sel]}</button>
        {/if}
      </div>
    </div>
    {#if draft.shape === 'strip'}
      <label class="field">
        <span class="lbl">Length on the map <b class="num">{draft.length ?? 0} dots</b></span>
        <input
          type="range"
          min="1"
          max={MAX_LEN}
          step="1"
          value={draft.length ?? 1}
          oninput={(e) => onlength(sel, Number((e.currentTarget as HTMLInputElement).value))}
          style:--fill={fill(draft.length ?? 1, 1, MAX_LEN)}
        />
      </label>
    {/if}
    <div class="rowset wide">
      <h4 id="arr-shape">Drawn as</h4>
      <div class="chips" role="group" aria-labelledby="arr-shape">
        {#each SHAPES as s}
          <button class="chip" aria-pressed={draft.shape === s.id} onclick={() => onshape(sel, s.id)}>{s.label}</button>
        {/each}
      </div>
    </div>
  {:else}
    <p class="hint wide">
      Pick a device on the map to move, turn, redraw or split it. With the map focused, arrow keys move the
      picked device, Shift moves it four dots, R turns it and Tab picks the next one.
    </p>
  {/if}
</div>

<style>
  .acts {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  /* Narrow: the pair keeps to one row under the subline instead of squeezing the heading. */
  @media (max-width: 700px) {
    .head {
      flex-wrap: wrap;
    }
    .acts {
      flex-wrap: nowrap;
    }
  }
  .who {
    font-weight: 500;
    font-size: 13.5px;
    min-height: 28px;
    display: flex;
    align-items: center;
    gap: 8px;
    overflow-wrap: anywhere;
  }
  .who .num {
    color: var(--muted);
    font-weight: 400;
    white-space: nowrap;
  }
</style>
