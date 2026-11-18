<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import type { SavedEffect } from './api'
  import {
    buildEffect,
    effectLabel,
    parseHex,
    SHELVES,
    shelfEffects,
    shelfOf,
    type EffectKind,
    type EffectSpec,
    type SceneParams,
    type ShelfId,
  } from './effects'
  import { onFrame, reducedMotion } from './frames'
  import Icon from './Icon.svelte'
  import { lookParams } from './looks'
  import { useMine } from './mine'
  import { glow, glowAt, type Glow } from './preview'
  import { sameSpec } from './scenes'
  import { shortcut } from './shortcut'
  import { Tweens } from './tween'
  import { drawVial, fit, newVial, pointer, tickPointer, trackPointer, type VialState } from './vial'

  let {
    base,
    active,
    current,
    activeTag,
    shared = null,
    theme,
    shelf = $bindable('essentials'),
    onpick,
    onpicksaved,
  }: {
    /** Tuning the vials are coloured with (the running look's colours and speed). */
    base: SceneParams | undefined
    /** The look playing on this target, filled to overflowing. */
    active: EffectKind | null
    /** The software effect playing on this target, if any: what "Save this look" keeps, and
     *  how a saved vial knows it is the one playing. */
    current?: EffectSpec
    activeTag: string
    /** In a device's view: the look it would follow, marked "Shared". */
    shared?: EffectKind | null
    theme: string
    /** The open shelf; number keys pick from it. Follows the playing look when that changes. */
    shelf?: ShelfId
    onpick: (kind: EffectKind) => void
    onpicksaved: (saved: SavedEffect) => void
  } = $props()

  const mine = useMine()
  const saved = $derived(mine?.saved ?? [])

  interface Item {
    id: string
    label: string
    meta: string
    spec: EffectSpec
    on: boolean
    ghost: boolean
    pick: () => void
    saved?: SavedEffect
  }
  const items = $derived.by((): Item[] => {
    if (shelf === 'mine')
      return saved.map((s) => ({
        id: `mine:${s.name}`,
        label: s.name,
        meta: effectLabel(s.effect.type),
        spec: { ...s.effect, brightness: 1 },
        on: sameSpec(current, s.effect),
        ghost: false,
        pick: () => onpicksaved(s),
        saved: s,
      }))
    return shelfEffects(shelf).map((e) => ({
      id: e.id,
      label: e.label,
      meta: e.meta,
      spec: buildEffect({ ...lookParams(e.id, base), brightness: 1 }),
      on: active === e.id,
      ghost: shared === e.id,
      pick: () => onpick(e.id),
    }))
  })

  const uid = $props.id()
  const playingShelf = $derived(active ? shelfOf(active) : undefined)
  const minePlaying = $derived(saved.some((s) => sameSpec(current, s.effect)))
  /** The one shelf that holds the playing look: a kept effect wins over its kind's shelf. */
  const dotShelf = $derived(minePlaying ? 'mine' : playingShelf)

  // Open the shelf of a look that starts playing, but leave a shelf the user opened alone, and
  // stay on My effects while one of its vials is the one playing.
  let mounted = false
  $effect(() => {
    const s = playingShelf
    const here = untrack(() => shelf)
    if (s && here !== s && !(here === 'mine' && minePlaying)) {
      if (mounted) untrack(() => open(s))
      else shelf = s
    }
    mounted = true
  })

  // Set once the user (or a new playing look) changes shelf, so only a shelf change deals.
  let dealing = $state(false)
  function open(id: ShelfId) {
    if (id === shelf) return
    dealing = true
    shelf = id
  }

  function tabKey(e: KeyboardEvent, i: number) {
    const step = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0
    const to = e.key === 'Home' ? 0 : e.key === 'End' ? SHELVES.length - 1 : (i + step + SHELVES.length) % SHELVES.length
    if (!step && e.key !== 'Home' && e.key !== 'End') return
    e.preventDefault()
    open(SHELVES[to].id)
    document.getElementById(`${uid}-tab-${to}`)?.focus()
  }

  let hover = $state<string | null>(null)
  const canvases: HTMLCanvasElement[] = []
  const states = new Map<string, VialState>()
  const tw = new Tweens()

  // Resting 0.6, hovered 0.8, playing brims over (fills in over a second).
  $effect(() => {
    for (const it of items) {
      const v = it.on ? 1.12 : hover === it.id ? 0.8 : 0.6
      if (!tw.has(it.id)) tw.set(it.id, v)
      else tw.to(it.id, v, it.on ? 1100 : 600)
    }
  })

  const glows = new Map<string, Glow>()

  let muted = '#958f83'
  $effect(() => {
    void theme
    muted = getComputedStyle(document.documentElement).getPropertyValue('--muted').trim() || muted
  })

  onMount(() => {
    trackPointer()
    const calm = reducedMotion()
    let last = performance.now()
    return onFrame((seconds) => {
      const now = performance.now()
      const dt = Math.min(0.05, (now - last) / 1000)
      last = now
      tickPointer(dt)
      // Only the vial nearest the pointer leans.
      let near = -1
      let best = 1e9
      canvases.forEach((c, i) => {
        if (!c || !c.isConnected) return
        const b = c.getBoundingClientRect()
        const d = Math.hypot(pointer.x - (b.left + b.width / 2), pointer.y - (b.top + b.height / 2))
        if (d < best && d < Math.max(140, b.width)) {
          best = d
          near = i
        }
      })
      const dark = document.documentElement.dataset.theme !== 'light'
      items.forEach((it, i) => {
        const c = canvases[i]
        const box = c && fit(c)
        if (!box) return
        let st = states.get(it.id)
        if (!st) states.set(it.id, (st = newVial()))
        const t = calm ? 0 : seconds
        const fb = it.spec.colors?.[0] ? parseHex(it.spec.colors[0]) : undefined
        const gl = glow(it.spec, t, dt, glows.get(it.id), fb)
        glows.set(it.id, gl)
        drawVial(box[0], box[1], box[2], st, {
          t,
          dt,
          level: tw.get(it.id, 0.6),
          near: near === i,
          dark,
          calm,
          label: String(i + 1),
          labelColor: muted,
          light: (u) => glowAt(gl, u),
        })
      })
    })
  })
</script>

<svelte:window onkeydown={(e) => shortcut(e, (n) => n >= 1 && n <= 6 && items[n - 1]?.pick())} />

<div class="rack">
  <div class="top">
    <div class="shelves" role="tablist" aria-label="Looks">
      {#each SHELVES as sh, i (sh.id)}
        {@const isOpen = shelf === sh.id}
        <button
          id="{uid}-tab-{i}"
          role="tab"
          aria-selected={isOpen}
          aria-controls="{uid}-rack"
          tabindex={isOpen ? 0 : -1}
          onclick={() => open(sh.id)}
          onkeydown={(e) => tabKey(e, i)}
        >
          {sh.label}
          {#if !isOpen && dotShelf === sh.id}<span class="dot" aria-hidden="true"></span><span class="sr">, {activeTag.toLowerCase()} here</span>{/if}
        </button>
      {/each}
    </div>
    {#if shelf === 'mine' && mine}
      <div class="acts">
        <button
          class="btn sm"
          disabled={!current || minePlaying}
          title={!current ? 'Pick a look first' : minePlaying ? 'Already kept in My effects' : undefined}
          onclick={() => current && mine.save(current)}
        >
          <Icon name="plus" size={13} />Save this look
        </button>
        <button class="btn sm" onclick={() => mine.import()}>Import…</button>
      </div>
    {/if}
  </div>
  {#key shelf}
    <div class="gels" class:deal={dealing} id="{uid}-rack" role="tabpanel" aria-labelledby="{uid}-tab-{SHELVES.findIndex((x) => x.id === shelf)}">
      {#each items as it, i (it.id)}
        <div class="cell">
          <button
            class="gel"
            aria-pressed={it.on}
            aria-keyshortcuts={i < 6 ? String(i + 1) : undefined}
            onclick={it.pick}
            onpointerenter={() => (hover = it.id)}
            onpointerleave={() => (hover = null)}
            onfocus={() => (hover = it.id)}
            onblur={() => (hover = null)}
          >
            <canvas bind:this={canvases[i]} aria-hidden="true"></canvas>
            <span class="cap">{it.label}</span>
            <span class="meta">
              {#if it.on}<span class="tag">{activeTag}</span>
              {:else if it.ghost}<span class="tag ghost">Shared</span>
              {:else}{it.meta}{/if}
            </span>
          </button>
          {#if it.saved && mine}
            {@const name = it.saved.name}
            <button class="ibtn edit" aria-label="Edit {name}" title="Rename, export or delete" onclick={() => mine.edit(name, current)}>
              <Icon name="pencil" size={13} />
            </button>
          {/if}
        </div>
      {:else}
        <p class="none">Looks you keep appear here. Save the look that is playing, or import an effect file someone shared.</p>
      {/each}
    </div>
  {/key}
</div>

<style>
  .rack {
    grid-area: gels;
    min-width: 0;
  }
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin: 0 0 2px;
    min-height: 30px;
  }
  .shelves {
    display: flex;
    gap: 18px;
  }
  .acts {
    display: flex;
    gap: 8px;
  }
  .acts .btn {
    gap: 6px;
  }
  .shelves button {
    position: relative;
    border: 0;
    background: none;
    padding: 6px 0;
    font: inherit;
    font-size: 12px;
    font-weight: 500;
    color: var(--muted);
    cursor: pointer;
    transition: color var(--fast) var(--ease);
  }
  .shelves button:hover {
    color: var(--text);
  }
  .shelves button[aria-selected='true'] {
    color: var(--text);
  }
  /* The open shelf: a short lamp rule under its name, like the rail's own-look dash. */
  .shelves button[aria-selected='true']::after {
    content: '';
    position: absolute;
    left: 0;
    bottom: 1px;
    width: 14px;
    height: 2px;
    border-radius: 1px;
    background: var(--lamp);
  }
  .dot {
    display: inline-block;
    width: 5px;
    height: 5px;
    margin-left: 6px;
    border-radius: 50%;
    background: var(--lamp);
    vertical-align: 2px;
  }
  .gels {
    display: grid;
    grid-template-columns: repeat(6, minmax(0, 1fr));
    gap: 8px;
  }
  .cell {
    position: relative;
    min-width: 0;
    display: grid;
  }
  .gel {
    position: relative;
    border: 0;
    background: none;
    padding: 0 0 4px;
    text-align: center;
    color: var(--text);
    border-radius: 12px;
    min-width: 0;
    display: grid;
  }
  /* Wider than the cell, so the table glow and splashes fade out before any edge. */
  canvas {
    display: block;
    width: 160%;
    margin-left: -30%;
    height: clamp(118px, 14vw, 190px);
    pointer-events: none;
  }
  .cap {
    font-weight: 500;
    font-size: 13px;
    padding: 0 4px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .meta {
    font-size: 12px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    height: 18px;
    margin-top: 2px;
  }
  :global(.boot) .cell {
    animation: deal 0.8s var(--ease) backwards;
  }
  :global(.boot) .cell:nth-child(1) {
    animation-delay: 0.25s;
  }
  :global(.boot) .cell:nth-child(2) {
    animation-delay: 0.3s;
  }
  :global(.boot) .cell:nth-child(3) {
    animation-delay: 0.35s;
  }
  :global(.boot) .cell:nth-child(4) {
    animation-delay: 0.4s;
  }
  :global(.boot) .cell:nth-child(5) {
    animation-delay: 0.45s;
  }
  :global(.boot) .cell:nth-child(6) {
    animation-delay: 0.5s;
  }
  /* Opening a shelf deals its vials in, quicker than the boot. */
  .gels.deal .cell {
    animation: deal 0.5s var(--ease) backwards;
  }
  .gels.deal .cell:nth-child(2) {
    animation-delay: 0.03s;
  }
  .gels.deal .cell:nth-child(3) {
    animation-delay: 0.06s;
  }
  .gels.deal .cell:nth-child(4) {
    animation-delay: 0.09s;
  }
  .gels.deal .cell:nth-child(5) {
    animation-delay: 0.12s;
  }
  .gels.deal .cell:nth-child(6) {
    animation-delay: 0.15s;
  }
  /* A saved vial's pencil sits at its top left, shown on hover or focus (always on touch). */
  .edit {
    position: absolute;
    top: 4px;
    left: 4px;
    width: 26px;
    height: 26px;
    border-color: transparent;
    color: var(--muted);
    opacity: 0;
    transition: opacity var(--fast);
  }
  .cell:hover .edit,
  .cell:focus-within .edit {
    opacity: 1;
  }
  .edit:hover {
    color: var(--text);
  }
  @media (hover: none) {
    .edit {
      opacity: 1;
    }
  }
  .none {
    grid-column: 1 / -1;
    padding: 28px 0 22px;
    max-width: 52ch;
    font-size: 12.5px;
    color: var(--muted);
    text-wrap: pretty;
  }
  @keyframes deal {
    from {
      opacity: 0;
      transform: translateY(28px) rotate(6deg);
    }
  }
  @media (max-width: 700px) {
    .top {
      flex-wrap: wrap;
    }
    .gels {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
    canvas {
      height: clamp(118px, 30vw, 170px);
    }
  }
</style>
