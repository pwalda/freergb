<script module lang="ts">
  /** The scene list Stop all switched off, so Resume can bring it back. */
  let stoppedFrom: Scene[] | null = null
</script>

<script lang="ts">
  import { untrack, type Snippet } from 'svelte'
  import {
    buildEffect,
    buildScene,
    DEFAULT_PARAMS,
    EFFECTS,
    type ShelfId,
    effectLabel,
    paramsOf,
    type EffectKind,
    type DeviceKey,
    type EffectSpec,
    type Scene,
    type SceneParams,
  } from './effects'
  import type { SavedEffect } from './api'
  import { lookParams } from './looks'
  import {
    deviceScene,
    globalScene,
    groupLookCount,
    groupOf,
    groupScene,
    lightsOut,
    OFF_SCENE,
    ownEffectCount,
    sameScene,
    withGlobalScene,
    type Group,
  } from './scenes'
  import type { ToastMessage } from './Toast.svelte'
  import Controls from './Controls.svelte'
  import Icon from './Icon.svelte'
  import Vials from './Vials.svelte'

  let {
    scenes,
    keys,
    groups,
    theme,
    edit,
    notify,
    onlive,
    onarrange,
    stage,
  }: {
    scenes: Scene[]
    /** The connected devices. */
    keys: DeviceKey[]
    groups: Group[]
    theme: string
    edit: (fn: (s: Scene[]) => Scene[]) => Promise<void>
    notify: (t: ToastMessage) => void
    /** The look as it is being tuned, ahead of what has been sent. */
    onlive: (spec: EffectSpec | undefined) => void
    /** Open the desk map arrangement. */
    onarrange: () => void
    stage: Snippet
  } = $props()

  const running = $derived(globalScene(scenes))
  /** Lights off (from here or the tray): shown as off, not as a black static look. */
  const dark = $derived(lightsOut(scenes))
  const deviceCount = $derived(keys.length)
  const own = $derived(ownEffectCount(scenes))
  const groupLooks = $derived(groupLookCount(scenes))
  /** Connected devices that show something other than the all-devices look. */
  const elsewhere = $derived(
    keys.filter((k) => deviceScene(scenes, k) || ((g) => g && groupScene(scenes, g.name))(groupOf(groups, k))).length,
  )

  let params = $state<SceneParams>({ ...DEFAULT_PARAMS, colors: [...DEFAULT_PARAMS.colors] })
  /** The scene this panel last sent or loaded; a different one from the backend is a change
   *  made elsewhere (a look, Stop all, Undo, another window) that the panel must follow. */
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
  // Leaving for a device with a slider edit still waiting: send it rather than lose it.
  $effect(() => () => {
    if (timer !== undefined) commit()
  })
  $effect(() => {
    const spec = running ? buildEffect($state.snapshot(params)) : undefined
    untrack(() => onlive(spec))
  })
  $effect(() => () => onlive(undefined))
  $effect(() => {
    if (scenes.length > 0) stoppedFrom = null
  })

  // Retuning the running look keeps every device's own look; sliders send once they settle.
  function commit() {
    clearTimeout(timer)
    timer = undefined
    const scene = buildScene($state.snapshot(params))
    if (sameScene(scene, sent)) return
    sent = scene
    void edit((s) => withGlobalScene(s, scene))
  }
  const later = () => {
    clearTimeout(timer)
    timer = setTimeout(commit, 150)
  }

  /** Choosing a look sets it on every device, replacing their own looks, with Undo. */
  function choose(kind: EffectKind) {
    if (running?.effect.type === kind && !dark && own === 0 && groupLooks === 0) return
    const before = scenes
    const replaced = own + groupLooks
    const base = running ? paramsOf(running) : undefined
    void edit(() => [buildScene(lookParams(kind, base))])
    notify({
      message: `${effectLabel(kind)} is on every device.${replaced > 0 ? ` ${replaced} individual ${replaced === 1 ? 'look was' : 'looks were'} replaced.` : ''}`,
      undo: () => void edit(() => before),
    })
  }

  /** A saved effect on every device, exactly as it was kept. */
  function chooseSaved(s: SavedEffect) {
    const before = scenes
    const replaced = own + groupLooks
    const scene: Scene = { effect: s.effect, targets: 'all', canvas: params.perDevice ? 'per_device' : 'shared' }
    if (sameScene(scene, running) && replaced === 0) return
    void edit(() => [scene])
    notify({
      message: `“${s.name}” is on every device.${replaced > 0 ? ` ${replaced} individual ${replaced === 1 ? 'look was' : 'looks were'} replaced.` : ''}`,
      undo: () => void edit(() => before),
    })
  }

  function stop() {
    const before = scenes
    stoppedFrom = before
    void edit(() => [])
    notify({
      message: 'Effects stopped. Devices keep their own modes.',
      undo: () => void edit(() => before),
    })
  }
  /** Every device black, with Undo; unlike Stop all, devices do not go back to their own
   *  (usually lit) modes. */
  function lightsOff() {
    const before = scenes
    void edit(() => [OFF_SCENE])
    notify({ message: 'Lights off on every device.', undo: () => void edit(() => before) })
  }
  function resume() {
    const back = stoppedFrom
    stoppedFrom = null
    if (back) void edit(() => back)
  }

  let shelf = $state<ShelfId>('essentials')
  const runningKind = $derived(running && !dark && EFFECTS.some((e) => e.id === running.effect.type) ? running.effect.type : null)
  const title = $derived(dark ? 'Lights are off' : running ? effectLabel(running.effect.type) : elsewhere > 0 ? 'No shared look' : 'Effects are off')
  const sub = $derived.by(() => {
    const theirs = `${elsewhere} ${elsewhere === 1 ? 'has its own look' : 'have their own look'}${groupLooks > 0 ? ' or a group’s' : ''}`
    if (dark) return 'Every device is dark. Pick a look to light them again.'
    if (running) {
      const following = Math.max(deviceCount - elsewhere, 0)
      return `Playing on ${following} of ${deviceCount} ${deviceCount === 1 ? 'device' : 'devices'}${elsewhere > 0 ? ` · ${theirs}` : ''}`
    }
    if (elsewhere > 0)
      return `${elsewhere} ${elsewhere === 1 ? 'device runs' : 'devices run'} their own or a group’s look. Pick a look to set one for the rest.`
    return 'Devices keep their own hardware modes. Pick a look to start.'
  })
  const canResume = $derived(scenes.length === 0 && stoppedFrom !== null)
</script>


<div class="head">
  {#key title}
    <div class="in">
      <h1>{title}</h1>
      <p class="sub">{sub}</p>
    </div>
  {/key}
  <div class="acts">
    <button class="btn" onclick={onarrange} aria-label="Arrange your desk"><Icon name="arrange" size={14} /><span class="lab">Arrange</span></button>
    {#if keys.length > 0 && !dark}
      <button class="btn" onclick={lightsOff} aria-label="Lights off"><Icon name="power" size={14} /><span class="lab">Lights off</span></button>
    {/if}
    {#if canResume}
      <button class="btn" onclick={resume}><Icon name="play" size={14} />Resume</button>
    {:else if scenes.length > 0 && !dark}
      <!-- Hidden while dark: handing devices back to their own modes would light them again. -->
      <button class="btn" onclick={stop}><Icon name="stop" size={14} />Stop all</button>
    {/if}
  </div>
</div>

<style>
  .acts {
    display: flex;
    gap: 8px;
  }
  /* Narrow: Arrange and Lights off keep their icons so Stop all stays on the heading row. */
  @media (max-width: 700px) {
    .lab {
      display: none;
    }
  }
</style>

{@render stage()}

<Vials
  base={running && !dark ? params : undefined}
  active={runningKind}
  current={running ? buildEffect(params) : undefined}
  activeTag="Playing"
  {theme}
  bind:shelf
  onpick={choose}
  onpicksaved={chooseSaved}
/>

{#if running && !dark}
  <div class="ctl">
    <Controls bind:params showCanvas onchange={later} />
  </div>
{/if}
