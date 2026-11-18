<script lang="ts">
  import { flushSync, onMount, untrack } from 'svelte'
  import {
    activeScenes,
    applyProfile,
    deleteEffect,
    deleteGroup,
    deleteProfile,
    deskMap,
    setDeskMap,
    detection as loadDetection,
    deviceNotes,
    onDeviceNotes,
    setDeviceNote,
    setSegments,
    onReadings,
    readings as loadReadings,
    sources as loadSources,
    type DeviceNote,
    onDetection,
    rescanDevices,
    type Detection,
    exportEffect,
    importEffect,
    myEffects,
    onEffects,
    groups as loadGroups,
    onGroups,
    devices as loadDevices,
    onDevices,
    onNotice,
    onUpdate,
    onProfiles,
    onSchedule,
    schedule as loadSchedule,
    setSchedule,
    onScenes,
    onServerState,
    profiles as loadProfiles,
    renameProfile,
    retryServer,
    saveEffect,
    saveGroup,
    saveProfile,
    serverState,
    setScenes,
    storeNotice,
    updateStatus,
    udevRules,
    takeAboutRequest,
    onAbout,
    type UpdateInfo,
    type Device,
    type Group,
    type Placed,
    type Profiles,
    type SavedEffect,
    type Scene,
    type ServerState,
  } from './lib/api'
  import AllPanel from './lib/AllPanel.svelte'
  import ArrangePanel from './lib/ArrangePanel.svelte'
  import { draftOf, partId, partName, partsOf, startLength, templateOf, withEntries, type Draft } from './lib/desk'
  import DevicePanel from './lib/DevicePanel.svelte'
  import DeviceSheet from './lib/DeviceSheet.svelte'
  import { displayNames, noteOf, shownDevices } from './lib/notes'
  import EffectSheet, { type EffectSheetMode } from './lib/EffectSheet.svelte'
  import GroupPanel from './lib/GroupPanel.svelte'
  import Icon from './lib/Icon.svelte'
  import { provideMine } from './lib/mine'
  import { SCAN_TEXT, scanDetail, scanOutcome, scanShort } from './lib/detection'
  import { ACCESS_DETAIL, ACCESS_DONE, ACCESS_TEXT, failText, setUpAccess } from './lib/access'
  import GroupSheet, { type GroupSheetMode } from './lib/GroupSheet.svelte'
  import { effectLabel, parseHex, type EffectSpec } from './lib/effects'
  import { reducedMotion } from './lib/frames'
  import { groupId, ledLit, litAt, lookOf, setReadings, type Live } from './lib/preview'
  import type { Readings } from './lib/react'
  import { sensing } from './lib/readings.svelte'
  import ProfileSheet, { type ProfileSheetMode } from './lib/ProfileSheet.svelte'
  import Rail from './lib/Rail.svelte'
  import SettingsSheet from './lib/SettingsSheet.svelte'
  import DiagnosticsSheet from './lib/DiagnosticsSheet.svelte'
  import AboutSheet from './lib/AboutSheet.svelte'
  import ScheduleSheet from './lib/ScheduleSheet.svelte'
  import { emptySchedule, type Schedule } from './lib/schedule.ts'
  import SegmentSheet from './lib/SegmentSheet.svelte'
  import { splittable, type Run } from './lib/segments'
  import { freeSpot, turned, type Shape, type Spot, type Turn } from './lib/rig'
  import { SceneStore } from './lib/scene-store'
  import { deviceScene, globalScene, groupOf, groupScene, keyId, lightsOut, sameKey } from './lib/scenes'
  import type { DeviceKey } from './lib/effects'
  import { loadShapes } from './lib/shapes'
  import Stage from './lib/Stage.svelte'
  import { resolveTheme, type Theme } from './lib/theme'
  import Toast, { type ToastMessage } from './lib/Toast.svelte'
  import WindowFrame from './lib/WindowFrame.svelte'

  let server = $state<ServerState | null>(null)
  /** Every device OpenRGB reports; `devices` leaves out the ones the user hid. */
  let allDevices = $state<Device[]>([])
  let notes = $state<DeviceNote[]>([])
  const devices = $derived(shownDevices(allDevices, notes))
  let scenes = $state<Scene[]>([])
  let error = $state('')
  let toast = $state<ToastMessage | null>(null)

  // One store for the whole window, so edits made in different places are sent in order.
  const store = new SceneStore(
    setScenes,
    activeScenes,
    (s) => (scenes = s),
    (message) => (error = message),
  )
  const edit = (fn: (s: Scene[]) => Scene[]) => {
    error = ''
    return store.edit(fn)
  }
  const notify = (t: ToastMessage) => (toast = t)

  // OpenRGB looking for devices: a rescan asked for here, or its own detection.
  let scan = $state<Detection>({ scanning: false, percent: null, label: '' })
  /** The devices when this window asked for a rescan; the toast compares against them. */
  let asked: Device[] | null = null
  function followScan(d: Detection) {
    const ended = scan.scanning && !d.scanning
    scan = d
    if (!ended || !asked) return
    const before = asked
    asked = null
    // New devices arrive as their own list change just after the scan ends.
    setTimeout(() => notify({ message: scanOutcome(before, allDevices) }), 600)
  }
  async function rescan() {
    if (scan.scanning) return
    error = ''
    asked = $state.snapshot(allDevices) as Device[]
    try {
      await rescanDevices()
    } catch (e) {
      asked = null
      error = `Could not ask OpenRGB to look for devices: ${e}`
    }
  }

  let profiles = $state<Profiles>({ names: [], active: null })
  let profileSheet = $state<ProfileSheetMode | null>(null)
  let settingsOpen = $state(false)
  /** A newer freeRGB ready to install. */
  let update = $state<UpdateInfo | null>(null)
  let diagnosticsOpen = $state(false)
  let aboutOpen = $state(false)
  /** From the tray: About takes over from whatever sheet is open. */
  function showAbout() {
    void takeAboutRequest()
    settingsOpen = false
    diagnosticsOpen = false
    aboutOpen = true
  }
  /** Linux: OpenRGB's udev rules are missing (false), there (true), or nothing to check (null). */
  let access = $state<boolean | null>(null)
  let settingUp = $state(false)
  async function setUp() {
    settingUp = true
    // The scan that follows reports what the new access found.
    asked = $state.snapshot(allDevices) as Device[]
    const err = await setUpAccess()
    settingUp = false
    if (err) {
      asked = null
      return notify({ message: err })
    }
    access = true
    notify({ message: ACCESS_DONE })
  }
  let schedule = $state<Schedule>(emptySchedule())
  let scheduleOpen = $state(false)

  let saved = $state<SavedEffect[]>([])
  let effectSheet = $state<EffectSheetMode | null>(null)
  provideMine({
    get saved() {
      return saved
    },
    save: (effect) => (effectSheet = { kind: 'save', effect }),
    edit: (name, current) => (effectSheet = { kind: 'edit', name, current }),
    import: async () => {
      error = ''
      try {
        const s = await importEffect()
        if (s) notify({ message: `Imported “${s.name}” into My effects.` })
      } catch (e) {
        error = String(e)
      }
    },
  })
  async function keepEffect(old: string | null, name: string, effect: EffectSpec) {
    const s = await saveEffect(old, name, effect)
    notify({ message: old === null ? `Kept as “${s.name}” in My effects.` : `Saved “${s.name}”.` })
  }
  async function shareEffect(name: string) {
    const path = await exportEffect(name)
    if (!path) return false
    notify({ message: `Exported “${name}” to ${path}` })
    return true
  }
  async function forgetEffect(name: string) {
    await deleteEffect(name)
    notify({ message: `Deleted “${name}” from My effects.` })
  }

  let groups = $state<Group[]>([])
  /** Where devices sit on the desk; the engine lays effects out on it. */
  let places = $state<Placed[]>([])
  let groupSheet = $state<GroupSheetMode | null>(null)
  async function storeGroup(old: string | null, name: string, keys: DeviceKey[]) {
    await store.idle()
    const g = await saveGroup(old, name, keys)
    groups = await loadGroups()
    // A new or renamed group opens in its own view.
    target = groupId(g.name)
    notify({ message: old === null ? `Made the group “${g.name}”.` : `Saved the group “${g.name}”.` })
  }
  async function removeGroup(name: string) {
    await store.idle()
    await deleteGroup(name)
    groups = await loadGroups()
    if (target === groupId(name)) target = null
    notify({ message: `Deleted the group “${name}”. Its devices follow all devices again.` })
  }
  /** A profile is a look on every device, so applying one comes with Undo like any look. */
  async function apply(name: string) {
    error = ''
    await store.idle()
    const before = store.scenes
    try {
      await applyProfile(name)
      notify({ message: `Profile “${name}” is on.`, undo: () => void edit(() => before) })
    } catch (e) {
      error = String(e)
    }
  }
  // Saving takes the backend's scenes, so every edit must have reached it first.
  async function save(name: string) {
    await store.idle()
    const saved = await saveProfile(name)
    notify({ message: `Saved as “${saved}”.` })
  }
  async function rename(from: string, to: string) {
    await renameProfile(from, to)
  }
  async function remove(name: string) {
    await deleteProfile(name)
    notify({ message: `Deleted the profile “${name}”.` })
  }

  // The theme follows the system until the user picks one; the pick is remembered here.
  const THEME_KEY = 'freergb-theme'
  const stored = () => {
    try {
      return localStorage.getItem(THEME_KEY)
    } catch {
      return null
    }
  }
  let theme = $state<Theme>(resolveTheme(stored(), matchMedia('(prefers-color-scheme: light)').matches))
  $effect(() => {
    document.documentElement.dataset.theme = theme
  })
  /** The new theme spreads as a circle from the switch, where the webview can do it. */
  function toggleTheme(e: MouseEvent) {
    const next: Theme = theme === 'dark' ? 'light' : 'dark'
    try {
      localStorage.setItem(THEME_KEY, next)
    } catch {
      /* private window or blocked storage: the choice just won't be remembered */
    }
    const doc = document as Document & { startViewTransition?: (cb: () => void) => { ready: Promise<void> } }
    const apply = () => {
      theme = next
      flushSync()
    }
    if (!doc.startViewTransition || reducedMotion()) return apply()
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect()
    const x = r.left + r.width / 2
    const y = r.top + r.height / 2
    const radius = Math.hypot(Math.max(x, innerWidth - x), Math.max(y, innerHeight - y))
    doc
      .startViewTransition(apply)
      .ready.then(() =>
        document.documentElement.animate(
          { clipPath: [`circle(0 at ${x}px ${y}px)`, `circle(${radius}px at ${x}px ${y}px)`] },
          { duration: 700, easing: 'cubic-bezier(0.16, 1, 0.3, 1)', pseudoElement: '::view-transition-new(root)' },
        ),
      )
      .catch(() => {})
  }

  /** Readings for looks that follow one: the controls show them, the preview draws with them. */
  function takeReadings(r: Readings) {
    sensing.now = r
    setReadings(r)
  }

  onMount(() => {
    const unlisten: Array<() => void> = []
    ;(async () => {
      unlisten.push(await onServerState((s) => (server = s)))
      unlisten.push(await onDevices((d) => (allDevices = d)))
      unlisten.push(await onDeviceNotes((n) => (notes = n)))
      unlisten.push(await onScenes((s) => store.external(s)))
      unlisten.push(await onProfiles((p) => (profiles = p)))
      unlisten.push(await onSchedule((s) => (schedule = s)))
      unlisten.push(await onGroups((g) => (groups = g)))
      unlisten.push(await onEffects((e) => (saved = e)))
      unlisten.push(await onNotice((m) => (error = m)))
      unlisten.push(await onUpdate((u) => (update = u)))
      unlisten.push(await onAbout(showAbout))
      unlisten.push(await onDetection(followScan))
      unlisten.push(await onReadings(takeReadings))
      // Events only cover changes after we subscribed; fetch what we missed.
      server = (await serverState()) ?? server
      allDevices = await loadDevices()
      notes = await deviceNotes()
      store.external(await activeScenes())
      profiles = await loadProfiles()
      schedule = await loadSchedule()
      groups = await loadGroups()
      places = await deskMap()
      const offers = await loadSources()
      sensing.sources = offers.map((o) => o.id)
      sensing.needs = Object.fromEntries(offers.flatMap((o) => (o.needs ? [[o.id, o.needs]] : [])))
      takeReadings(await loadReadings())
      saved = await myEffects()
      scan = await loadDetection()
      update = (await updateStatus()) ?? update
      access = await udevRules()
      if (await takeAboutRequest()) showAbout()
      const n = await storeNotice()
      if (n) error = n
    })()
    const t = setTimeout(() => (boot = false), 1400)
    return () => {
      unlisten.forEach((u) => u())
      clearTimeout(t)
    }
  })
  /** First open: the window's parts arrive in a short stagger. */
  let boot = $state(true)

  const scanning = $derived(server?.kind === 'connected' && scan.scanning)
  const statusShort = $derived(
    scanning ? scanShort(scan) : !server ? 'Starting…' : { starting: 'Starting…', connected: 'Connected', reconnecting: 'Reconnecting…', failed: 'Not connected' }[server.kind],
  )
  const statusText = $derived.by(() => {
    if (!server) return 'Starting…'
    switch (server.kind) {
      case 'starting':
        return server.owned ? 'Starting OpenRGB…' : 'Looking for OpenRGB…'
      case 'connected':
        if (scan.scanning) return SCAN_TEXT
        return `Connected to ${server.server_name}${server.owned ? ' (managed by freeRGB)' : ''}`
      case 'reconnecting':
        return `Connection lost, retrying (attempt ${server.attempt})…`
      case 'failed':
        return failText(server)
    }
  })

  const ready = $derived(server?.kind === 'connected' && devices.length > 0)
  const names = $derived(displayNames(devices, notes))

  // Segments: a zone split into runs, each its own piece on the desk map.
  let segmentSheet = $state<{ dev: Device; zone: number | null } | null>(null)
  async function splitZone(d: Device, zone: number, runs: Run[], before: Run[]) {
    const label = names[devices.indexOf(d)] ?? d.name
    await setSegments(d.id, zone, runs)
    const undo = () => void setSegments(d.id, zone, before).catch((e) => (error = String(e)))
    notify({
      message: runs.length
        ? `Split “${label}” into ${runs.length} ${runs.length === 1 ? 'segment' : 'segments'}. Place each one, then Done.`
        : `“${label}” is one piece again.`,
      undo,
    })
  }

  // Rename or hide: the user's own settings, not OpenRGB's.
  let deviceSheet = $state<Device | null>(null)
  async function saveNote(d: Device, name: string | null, hidden: boolean) {
    const before = noteOf(notes, d)
    const label = name ?? d.name
    await setDeviceNote(d.key, name, hidden)
    const undo = () => void setDeviceNote(d.key, before?.name ?? null, before?.hidden ?? false).catch((e) => (error = String(e)))
    if (hidden && !before?.hidden) {
      target = null
      notify({ message: `“${label}” is hidden. Bring it back from Settings.`, undo })
    } else if ((before?.name ?? null) !== name) {
      notify({ message: name ? `Renamed to “${name}”.` : `Back to its own name, “${d.name}”.`, undo })
    }
  }
  /** Hidden devices for Settings, connected or not. */
  const hiddenRows = $derived(
    notes
      .filter((n) => n.hidden)
      .map((n) => {
        const d = allDevices.find((x) => sameKey(x.key, n.device))
        return { key: n.device, name: n.name ?? d?.name ?? n.device.name, away: !d }
      }),
  )
  const showDevice = (key: DeviceKey) => setDeviceNote(key, notes.find((n) => sameKey(n.device, key))?.name ?? null, false)

  /** The device in view (by key, so it survives an OpenRGB restart), a group (`groupId`), or
   *  null for all devices. */
  let target = $state<string | null>(null)
  const selected = $derived(target === null ? -1 : devices.findIndex((d) => keyId(d.key) === target))
  const select = (i: number) => (target = i < 0 ? null : keyId(devices[i].key))
  const selectedGroup = $derived(groups.find((g) => groupId(g.name) === target))
  /** Indices of the connected devices of each group, in the group's order. */
  const memberIdx = (g: Group) => g.devices.map((k) => devices.findIndex((d) => sameKey(d.key, k))).filter((i) => i >= 0)
  /** In a group's view the stage shows its members alone, left to right in group order. */
  const members = $derived(selectedGroup ? memberIdx(selectedGroup) : null)
  const focus = $derived(selectedGroup ? { name: selectedGroup.name } : null)

  // Each piece's drawing and spot on the desk map; while arranging, the edited copy. A piece is
  // a device, or one of its segments (each sits on the map by itself).
  const parts = $derived(partsOf(devices))
  const partIds = $derived(parts.map((p) => partId(devices[p.dev], p.piece)))
  const partNames = $derived(parts.map((p) => partName(names[p.dev], p.piece)))
  const ledsOf = (i: number) => parts[i].piece.leds.length
  /** The pieces on the stage (indices into `parts`): a group's members' in group order. */
  const shownParts = $derived(
    members ? members.flatMap((m) => parts.flatMap((p, i) => (p.dev === m ? [i] : []))) : parts.map((_, i) => i),
  )
  /** Shapes picked before they were kept with the desk map (this window's storage). */
  const legacyShapes = loadShapes()
  const savedDrafts = $derived(parts.map((p) => draftOf(places, devices[p.dev], p.piece, legacyShapes)))
  /** While arranging: each piece's draft by `partId`, so a split mid-way keeps the others. */
  let drafts = $state<Record<string, Draft> | null>(null)
  let arrPick = $state<string | null>(null)
  const arrSel = $derived(arrPick === null ? -1 : partIds.indexOf(arrPick))
  const desk = $derived(drafts ? parts.map((_, i) => drafts![partIds[i]] ?? savedDrafts[i]) : savedDrafts)
  const templates = $derived(parts.map((_, i) => templateOf(ledsOf(i), desk[i].shape, desk[i].length)))
  const spots = $derived(desk.map((d) => d.spot))
  let stageApi = $state<{ drawnSpots: () => Spot[] }>()

  function startArranging() {
    // The rig as drawn now becomes the starting desk, and strips get a straight length.
    const now = stageApi?.drawnSpots() ?? []
    const first = !savedDrafts.some((d) => d.spot)
    drafts = Object.fromEntries(
      savedDrafts.map((d, i) => [
        partIds[i],
        {
          ...d,
          length: d.shape === 'strip' ? (d.length ?? startLength(ledsOf(i))) : null,
          spot: d.spot ?? (first ? (now[i] ?? null) : null),
        },
      ]),
    )
    // A desk saved before devices kept apart (or redrawn since) is pulled apart once here.
    for (let i = 0; i < parts.length; i++) setDraft(i, (d) => clear(i, d))
    arrPick = null
  }
  const setDraft = (i: number, f: (d: Draft) => Draft) => {
    if (drafts) drafts = { ...drafts, [partIds[i]]: f(desk[i]) }
  }
  // Pieces that appear while arranging (a strip just split) start under the rest of their
  // device, clear of the others, instead of trailing off to the right of the desk.
  $effect(() => {
    if (!drafts) return
    const fresh = partIds.flatMap((id, i) => (id in drafts! ? [] : [i]))
    if (fresh.length) untrack(() => fresh.forEach(seed))
  })
  /** Empty dot rows left under a piece for its name on the stage. */
  const NAME_ROOM = 4
  function seed(i: number) {
    // Under the lowest piece of the device, leaving room for that piece's name.
    let sib = -1
    let bottom = -Infinity
    parts.forEach((p, j) => {
      const sp = desk[j].spot
      if (j === i || p.dev !== parts[i].dev || !sp) return
      const b = sp.y0 + turned(templates[j], sp.rot).h
      if (b > bottom) [sib, bottom] = [j, b]
    })
    const s = sib >= 0 ? desk[sib].spot! : null
    const base = { ...savedDrafts[i], length: savedDrafts[i].shape === 'strip' ? (savedDrafts[i].length ?? startLength(ledsOf(i))) : null }
    // With no piece of the device left on the desk (a strip split end to end), the first
    // segment takes the place the whole strip had.
    const whole = drafts?.[keyId(devices[parts[i].dev].key)]?.spot ?? null
    const below = s ? { x0: s.x0, y0: bottom + NAME_ROOM, rot: 0 as Turn } : whole
    setDraft(i, () => clear(i, { ...base, spot: savedDrafts[i].spot ?? below }))
  }
  /** `d` for piece `i` with its spot moved to the nearest place clear of the other pieces
   *  (pieces never overlap on the desk; one empty dot stays between them). */
  function clear(i: number, d: Draft): Draft {
    if (!d.spot || !drafts) return d
    const all = desk
    const drawn = stageApi?.drawnSpots() ?? []
    const others = parts.flatMap((_, j) => {
      const s = all[j].spot ?? drawn[j]
      if (j === i || !s) return []
      const t = turned(templateOf(ledsOf(j), all[j].shape, all[j].length), s.rot)
      return [{ x0: s.x0, y0: s.y0, w: t.w, h: t.h }]
    })
    const t = turned(templateOf(ledsOf(i), d.shape, d.length), d.spot.rot)
    return { ...d, spot: { ...d.spot, ...freeSpot(others, t.w, t.h, d.spot) } }
  }
  /** A change of drawing keeps the piece's centre where it was, on whole cells. */
  function redraw(i: number, f: (d: Draft) => Draft) {
    setDraft(i, (d) => {
      const next = f(d)
      if (!d.spot) return clear(i, next)
      const a = turned(templateOf(ledsOf(i), d.shape, d.length), d.spot.rot)
      const b = turned(templateOf(ledsOf(i), next.shape, next.length), next.spot?.rot ?? d.spot.rot)
      const rot = next.spot?.rot ?? d.spot.rot
      return clear(i, { ...next, spot: { x0: d.spot.x0 + Math.round((a.w - b.w) / 2), y0: d.spot.y0 + Math.round((a.h - b.h) / 2), rot } })
    })
  }
  const turn = (i: number) =>
    redraw(i, (d) => {
      const at = d.spot ?? stageApi?.drawnSpots()[i] ?? { x0: 0, y0: 0, rot: 0 as Turn }
      return { ...d, spot: { ...at, rot: ((at.rot + 90) % 360) as Turn } }
    })
  const reshape = (i: number, s: Shape) =>
    redraw(i, (d) => ({ ...d, shape: s, length: s === 'strip' ? (d.length ?? startLength(ledsOf(i))) : null }))
  const relength = (i: number, n: number) => redraw(i, (d) => ({ ...d, length: n }))
  async function finishArranging() {
    if (!drafts) return
    // Pieces left where the stage put them (new ones, right of the desk) are placed there.
    const now = stageApi?.drawnSpots() ?? []
    const done = desk.map((d, i) => ({ ...d, spot: d.spot ?? now[i] ?? null }))
    const before = places
    const next = withEntries(places, devices, parts, done)
    try {
      await setDeskMap(next)
    } catch (e) {
      error = String(e)
      return
    }
    places = next
    drafts = null
    notify({
      message: 'Saved the desk. Effects now sweep across it as arranged.',
      undo: async () => {
        await setDeskMap(before)
        places = before
      },
    })
  }

  let live = $state.raw<Live>({})
  const setLiveAll = (spec: EffectSpec | undefined) => (live = { ...live, all: spec })
  const setLiveDev = (id: string) => (spec: EffectSpec | undefined) =>
    (live = { ...live, dev: spec ? { id, spec } : live.dev?.id === id ? undefined : live.dev })

  /** While arranging, the stage previews effects on the desk as it is being laid out. */
  const previewMap = $derived(drafts ? withEntries(places, devices, parts, desk) : places)
  const looks = $derived(devices.map((d, i) => lookOf(devices, i, scenes, live, keyId(d.key), previewMap)))

  // The rail's strips: the shared look along the strip, and what each device shows.
  const shared = $derived(globalScene(scenes))
  const allSample = $derived.by(() => {
    const spec = live.all ?? shared?.effect
    if (!spec || spec.type === 'painted' || spec.type === 'device_mode') return null
    const fb = spec.colors?.[0] ? parseHex(spec.colors[0]) : undefined
    return (u: number, t: number) => litAt(spec, t, u, fb)
  })
  const samples = $derived(
    looks.map((look, i) => {
      const n = devices[i].colors.length
      return (u: number, t: number) => ledLit(look, Math.round(u * (n - 1)), t)
    }),
  )
  const owns = $derived(
    devices.map((d) => {
      const e = deviceScene(scenes, d.key)?.effect
      if (!e) return null
      return e.type === 'device_mode' ? (e.mode ?? 'Built-in mode') : e.type === 'painted' ? 'Painted' : effectLabel(e.type)
    }),
  )
  /** Per device: the group it is in, when it follows that group (no look of its own). */
  const inGroup = $derived(devices.map((d, i) => (owns[i] ? null : (groupOf(groups, d.key)?.name ?? null))))
  const groupRows = $derived(
    groups.map((g) => {
      const look = groupScene(scenes, g.name)
      const liveSpec = live.dev?.id === groupId(g.name) ? live.dev.spec : undefined
      const spec = look ? (liveSpec ?? look.effect) : (live.all ?? shared?.effect)
      const sample =
        spec && spec.type !== 'painted' && spec.type !== 'device_mode'
          ? ((fb) => (u: number, t: number) => litAt(spec, t, u, fb))(spec.colors?.[0] ? parseHex(spec.colors[0]) : undefined)
          : null
      return {
        name: g.name,
        count: memberIdx(g).length,
        label: look ? effectLabel(look.effect.type) : 'Follows all devices',
        own: !!look,
        sample,
      }
    }),
  )
  const allLabel = $derived(lightsOut(scenes) ? 'Lights off' : shared ? effectLabel(shared.effect.type) : scenes.length > 0 ? 'No shared look' : 'Off')

  const waiting = $derived.by(() => {
    if (server?.kind === 'connected') return 'Nothing to light yet'
    if (server?.kind === 'failed') return 'Not connected to OpenRGB'
    return 'Waiting for OpenRGB'
  })
</script>

{#snippet notice()}
  {#if server?.kind === 'failed'}
    <p>{failText(server)}</p>
    <button class="btn" onclick={retryServer}>Retry</button>
  {:else if server?.kind === 'reconnecting'}
    <p>Lost the connection to OpenRGB. Your lights keep their last look while we reconnect.</p>
  {:else if server?.kind === 'connected'}
    {#if scanning}
      <p>{SCAN_TEXT} <span class="num">{scanDetail(scan)}</span></p>
    {:else}
      {#if allDevices.length > 0}
        <p>Every device is hidden. Bring them back from Settings.</p>
        <button class="btn" onclick={() => (settingsOpen = true)}><Icon name="gear" size={14} />Settings</button>
      {:else}
        {#if access === false}
          <p>No devices found yet. {ACCESS_TEXT}<span class="fine">{ACCESS_DETAIL}</span></p>
          <div class="pair">
            <button class="btn" disabled={settingUp} onclick={setUp}><Icon name="key" size={14} />Set up device access</button>
            <button class="btn" onclick={rescan}><Icon name="rescan" size={14} />Look again</button>
          </div>
        {:else}
          <p>No devices found yet. OpenRGB may still be detecting hardware.</p>
          <button class="btn" onclick={rescan}><Icon name="rescan" size={14} />Look again</button>
        {/if}
      {/if}
    {/if}
  {:else}
    <p>Devices will appear here once OpenRGB is connected.</p>
  {/if}
{/snippet}

{#snippet stage()}
  <Stage
    devices={members ? members.map((i) => devices[i]) : devices}
    parts={shownParts.map((i) => ({ dev: members ? members.indexOf(parts[i].dev) : parts[i].dev, leds: parts[i].piece.leds }))}
    names={shownParts.map((i) => partNames[i])}
    looks={members ? members.map((i) => looks[i]) : looks}
    templates={shownParts.map((i) => templates[i])}
    spots={shownParts.map((i) => spots[i])}
    {selected}
    {focus}
    onselect={(i) => select(members ? members[i] : i)}
    notice={ready ? undefined : notice}
    bind:this={stageApi}
    arrange={drafts && !members && selected < 0
      ? {
          sel: arrSel,
          onpick: (i) => (arrPick = partIds[i] ?? null),
          onmove: (i, s) => setDraft(i, (d) => clear(i, { ...d, spot: s })),
          onturn: turn,
        }
      : null}
  />
{/snippet}

<WindowFrame />

<div class="app" class:boot>
  <Rail
    {server}
    {statusText}
    {statusShort}
    {scan}
    {scanning}
    onrescan={rescan}
    {devices}
    {names}
    {owns}
    {inGroup}
    groups={groupRows}
    selectedGroup={selectedGroup?.name ?? null}
    {allLabel}
    {allSample}
    {samples}
    {selected}
    {theme}
    {profiles}
    onselect={select}
    onselectgroup={(name) => (target = groupId(name))}
    onnewgroup={() => (groupSheet = { kind: 'new' })}
    ontheme={toggleTheme}
    {update}
    onsettings={() => (settingsOpen = true)}
    onretry={retryServer}
    onapply={apply}
    onsaveprofile={() => (profileSheet = { kind: 'save' })}
    oneditprofile={(name) => (profileSheet = { kind: 'edit', name })}
    {schedule}
    oneditschedule={() => (scheduleOpen = true)}
  />

  <main class="main">
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    {#if ready && selected >= 0}
      {#key target}
        <DevicePanel
          device={devices[selected]}
          name={names[selected]}
          {scenes}
          {groups}
          {theme}
          {edit}
          {notify}
          onerror={(m) => (error = m)}
          onlive={setLiveDev(target ?? '')}
          onrename={() => (deviceSheet = devices[selected])}
          {stage}
        />
      {/key}
    {:else if ready && selectedGroup}
      {#key target}
        <GroupPanel
          group={selectedGroup}
          {devices}
          {scenes}
          {theme}
          {edit}
          {notify}
          onlive={setLiveDev(target ?? '')}
          onedit={() => (groupSheet = { kind: 'edit', name: selectedGroup.name })}
          {stage}
        />
      {/key}
    {:else if ready && drafts}
      <ArrangePanel
        counts={parts.map((p) => p.piece.leds.length)}
        names={partNames}
        drafts={desk}
        split={parts.map((p) => (splittable(devices[p.dev]).length ? (devices[p.dev].pieces?.some((x) => x.segment) ? 'Edit segments' : 'Split into segments') : null))}
        onsplit={(i) => (segmentSheet = { dev: devices[parts[i].dev], zone: parts[i].piece.segment?.zone ?? null })}
        sel={arrSel}
        onturn={turn}
        onshape={reshape}
        onlength={relength}
        oncancel={() => (drafts = null)}
        ondone={finishArranging}
        {stage}
      />
    {:else if ready}
      <AllPanel
        {scenes}
        keys={devices.map((d) => d.key)}
        {groups}
        {theme}
        {edit}
        {notify}
        onlive={setLiveAll}
        onarrange={startArranging}
        {stage}
      />
    {:else}
      <div class="head">
        <div class="in">
          <h1>All devices</h1>
          <p class="sub">{waiting}</p>
        </div>
      </div>
      {@render stage()}
    {/if}
  </main>
</div>

{#if profileSheet}
  <ProfileSheet
    mode={profileSheet}
    names={profiles.names}
    onsave={save}
    onrename={rename}
    ondelete={remove}
    onclose={() => (profileSheet = null)}
  />
{/if}

{#if segmentSheet}
  {@const s = segmentSheet}
  <SegmentSheet
    device={s.dev}
    name={names[devices.indexOf(s.dev)] ?? s.dev.name}
    zone={s.zone}
    onsave={(zone, runs, before) => splitZone(s.dev, zone, runs, before)}
    onclose={() => (segmentSheet = null)}
  />
{/if}

{#if deviceSheet}
  {@const d = deviceSheet}
  <DeviceSheet
    device={d}
    note={noteOf(notes, d)}
    onsave={(name, hidden) => saveNote(d, name, hidden)}
    onclose={() => (deviceSheet = null)}
  />
{/if}

{#if groupSheet}
  <GroupSheet
    mode={groupSheet}
    {groups}
    {devices}
    {names}
    hidden={hiddenRows}
    onsave={storeGroup}
    ondelete={removeGroup}
    onclose={() => (groupSheet = null)}
  />
{/if}

{#if effectSheet}
  <EffectSheet
    mode={effectSheet}
    {saved}
    onsave={keepEffect}
    onexport={shareEffect}
    ondelete={forgetEffect}
    onclose={() => (effectSheet = null)}
  />
{/if}

{#if scheduleOpen}
  <ScheduleSheet {schedule} profiles={profiles.names} onsave={setSchedule} onclose={() => (scheduleOpen = false)} />
{/if}

{#if settingsOpen}
  <SettingsSheet
    {update}
    hidden={hiddenRows}
    onshow={showDevice}
    ondiagnostics={() => {
      settingsOpen = false
      diagnosticsOpen = true
    }}
    onabout={() => {
      settingsOpen = false
      aboutOpen = true
    }}
    onclose={() => (settingsOpen = false)}
  />
{/if}
{#if aboutOpen}
  <AboutSheet onclose={() => (aboutOpen = false)} />
{/if}

{#if diagnosticsOpen}
  <DiagnosticsSheet onclose={() => (diagnosticsOpen = false)} />
{/if}

<Toast {toast} onclose={() => (toast = null)} />

<style>
  /* The stage notice's second line and its two actions. */
  .fine {
    display: block;
    margin-top: 4px;
    font-size: 12.5px;
    color: var(--muted);
    text-wrap: pretty;
  }
  .pair {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 8px;
  }
  .app {
    height: 100%;
    display: grid;
    grid-template-columns: 236px minmax(0, 1fr);
    grid-template-areas: 'rail main';
  }
  :global(:root) {
    --rail-w: 236px;
  }
  @media (min-width: 1240px) {
    .app {
      grid-template-columns: 272px minmax(0, 1fr);
    }
    :global(:root) {
      --rail-w: 272px;
    }
  }
  @media (max-width: 700px) {
    .app {
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: auto minmax(0, 1fr);
      grid-template-areas: 'rail' 'main';
    }
    :global(:root) {
      --rail-w: 0px;
    }
  }
</style>
