<script lang="ts">
  import { untrack, type Snippet } from 'svelte'
  import type { Device, SavedEffect } from './api'
  import Controls from './Controls.svelte'
  import {
    buildEffect,
    buildScene,
    DEFAULT_PARAMS,
    EFFECTS,
    effectLabel,
    paramsOf,
    type EffectKind,
    type EffectSpec,
    type Scene,
    type SceneParams,
    type ShelfId,
  } from './effects'
  import Icon from './Icon.svelte'
  import { lookParams } from './looks'
  import { deviceScene, globalScene, groupScene, sameKey, sameScene, withGroupScene, type Group } from './scenes'
  import { shortcut } from './shortcut'
  import type { ToastMessage } from './Toast.svelte'
  import Vials from './Vials.svelte'

  let {
    group,
    devices,
    scenes,
    theme,
    edit,
    notify,
    onlive,
    onedit,
    stage,
  }: {
    group: Group
    devices: Device[]
    scenes: Scene[]
    theme: string
    edit: (fn: (s: Scene[]) => Scene[]) => Promise<void>
    notify: (t: ToastMessage) => void
    /** The look as it is being tuned, ahead of what has been sent. */
    onlive: (spec: EffectSpec | undefined) => void
    onedit: () => void
    stage: Snippet
  } = $props()

  const running = $derived(groupScene(scenes, group.name))
  const shared = $derived(globalScene(scenes))
  const present = $derived(group.devices.filter((k) => devices.some((d) => sameKey(d.key, k))))
  const ownCount = $derived(present.filter((k) => deviceScene(scenes, k)).length)
  const targets = $derived({ devices: group.devices })

  let shelf = $state<ShelfId>('essentials')
  let params = $state<SceneParams>({ ...DEFAULT_PARAMS, colors: [...DEFAULT_PARAMS.colors] })
  /** The scene this panel last sent or loaded; a different one from the backend is a change
   *  made elsewhere that the panel must follow. */
  let sent: Scene | undefined
  let timer: ReturnType<typeof setTimeout> | undefined

  $effect(() => {
    const fromBackend = running
    untrack(() => {
      if (sameScene(fromBackend, sent)) return
      clearTimeout(timer)
      timer = undefined
      sent = fromBackend
      if (fromBackend) params = paramsOf(fromBackend)
    })
  })
  // Leaving with a slider edit still waiting: send it rather than lose it.
  $effect(() => () => {
    if (timer !== undefined) commit()
  })
  $effect(() => {
    const spec = running ? buildEffect($state.snapshot(params)) : undefined
    untrack(() => onlive(spec))
  })
  $effect(() => () => onlive(undefined))

  function commit() {
    clearTimeout(timer)
    timer = undefined
    const scene = buildScene($state.snapshot(params), targets, group.name)
    if (sameScene(scene, sent)) return
    sent = scene
    void edit((s) => withGroupScene(s, group.name, scene))
  }
  const later = () => {
    clearTimeout(timer)
    timer = setTimeout(commit, 150)
  }

  /** A look for the group's devices as one surface; their own looks stay on top, with Undo. */
  function choose(kind: EffectKind) {
    if (running?.effect.type === kind) return
    const before = scenes
    const from = running ?? shared
    const scene = buildScene(lookParams(kind, from ? paramsOf(from) : undefined), targets, group.name)
    void edit((s) => withGroupScene(s, group.name, scene))
    notify({ message: `${effectLabel(kind)} is on “${group.name}”.`, undo: () => void edit(() => before) })
  }

  function chooseSaved(s: SavedEffect) {
    const before = scenes
    const scene: Scene = { effect: s.effect, targets, canvas: 'shared', group: group.name }
    if (sameScene(scene, running)) return
    void edit((x) => withGroupScene(x, group.name, scene))
    notify({ message: `“${s.name}” is on “${group.name}”.`, undo: () => void edit(() => before) })
  }

  function follow() {
    if (!running) return
    const before = scenes
    void edit((s) => withGroupScene(s, group.name, null))
    notify({ message: `“${group.name}” follows all devices again.`, undo: () => void edit(() => before) })
  }

  const runningKind = $derived(running && EFFECTS.some((e) => e.id === running.effect.type) ? running.effect.type : null)
  const sharedKind = $derived(shared && EFFECTS.some((e) => e.id === shared.effect.type) ? shared.effect.type : null)
  const sub = $derived.by(() => {
    const n = present.length
    const missing = group.devices.length - n
    const count = `${n} ${n === 1 ? 'device' : 'devices'} as one surface${missing > 0 ? ` · ${missing} not connected` : ''}`
    const own = ownCount > 0 ? ` · ${ownCount} ${ownCount === 1 ? 'has its own look' : 'have their own look'}` : ''
    if (running) return `${count}${own}`
    return `${count} · follows all devices${shared ? ` (${effectLabel(shared.effect.type)})` : ''}. Pick a look to give the group its own.${own}`
  })
</script>

<svelte:window
  onkeydown={(e) =>
    shortcut(e, (n) => {
      if (n === 0) follow()
    })}
/>

<div class="head">
  <div class="in">
    <h1>{group.name}</h1>
    <p class="sub">{sub}</p>
  </div>
  <div class="acts">
    {#if running}
      <button class="btn" aria-keyshortcuts="0" onclick={follow}>Follow all devices</button>
    {/if}
    <button class="btn" aria-label="Edit group" onclick={onedit}><Icon name="pencil" size={14} /><span class="wide">Edit group</span></button>
  </div>
</div>

{@render stage()}

<Vials
  base={running ? params : shared ? paramsOf(shared) : undefined}
  active={runningKind}
  current={running ? buildEffect(params) : undefined}
  activeTag="Group look"
  shared={sharedKind}
  {theme}
  bind:shelf
  onpick={choose}
  onpicksaved={chooseSaved}
/>

{#if running}
  <div class="ctl">
    <Controls bind:params showCanvas onchange={later} />
  </div>
{/if}

<style>
  .acts {
    display: flex;
    gap: 8px;
    flex: none;
  }
  /* Narrow: the pencil alone, so both actions stay on the heading's row. */
  @media (max-width: 700px) {
    .wide {
      display: none;
    }
  }
</style>
