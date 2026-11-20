import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { Schedule } from './schedule.ts'
import type { DeviceKey, EffectSpec, ModeDirection, Scene, SourceId } from './effects.ts'
import type { Readings } from './react.ts'
import type { Group } from './scenes.ts'
import type { Hardware, LogView } from './diagnostics.ts'
export type { Group }

export type ServerState =
  | { kind: 'starting'; owned: boolean }
  | { kind: 'connected'; owned: boolean; server_name: string }
  | { kind: 'reconnecting'; attempt: number; retry_in_secs: number }
  | { kind: 'failed'; reason: string; message: string }

/** Where a device sits on the desk map: centre, size of its unit square and clockwise rotation
 *  in degrees (mirrors `layout::Placement`). */
export interface Placement {
  x: number
  y: number
  w: number
  h: number
  rot: number
}

/** One device's entry in the desk map (mirrors `layout::Placed`). */
export interface Placed {
  device: DeviceKey
  /** Absent until the device is placed; it then takes a slot right of the placed ones. */
  at?: Placement
  /** Each LED's spot in the unit square as the stage draws the device; used while it has one
   *  entry per LED. */
  points?: [number, number][]
  /** How the stage draws it (a `Shape`) and a strip's length in dots. */
  shape?: string
  length?: number
  /** Set on a segment's own entry (each segment is its own piece on the map). */
  segment?: SegmentKey | null
}

/** A segment by its zone and first LED in the zone (mirrors `layout::SegmentKey`). */
export interface SegmentKey {
  zone: number
  start: number
}

/** One piece of a device on the desk map: an OpenRGB segment, or (segment null) the LEDs no
 *  segment holds. Mirrors `PieceView`. */
export interface Piece {
  segment: SegmentKey | null
  name: string
  /** Device-wide LED indices, ascending. */
  leds: number[]
}

export interface Zone {
  /** Index in the device's zone list; what `resizeZone` takes. */
  index: number
  name: string
  leds: number
  min_leds: number
  max_leds: number
  /** False for zones the device does not let us resize; the server would ignore the request. */
  resizable: boolean
  /** Device-wide index of the zone's first LED: zones are concatenated in order, the same
   *  indices as `Device.colors` and a painted scene's colours. */
  start: number
  /** A keyboard's grid, or null for a zone that is a plain row of LEDs. */
  matrix: Matrix | null
  /** The zone's own built-in modes; empty unless the device has per-zone modes. */
  modes: ModeView[]
  /** Index into the zone's own modes, -1 when it follows the device's mode. */
  active_mode: number
}

/** Row-major cells holding device-wide LED indices, null for an empty cell. */
export interface Matrix {
  width: number
  height: number
  cells: (number | null)[]
}

/** One of a device's own built-in modes worth picking. Speed and brightness are positions 0..1
 *  in the mode's own range, null when the mode has no such setting. */
export interface ModeView {
  /** Index in the device's own mode list. */
  index: number
  name: string
  speed: number | null
  brightness: number | null
  directions: ModeDirection[]
  direction: ModeDirection | null
  colors_min: number
  colors_max: number
  colors: string[]
  /** The device can keep this mode in its own memory. */
  saveable: boolean
  /** The mode drives the whole device: its zones cannot run modes of their own. */
  whole_device: boolean
}

export interface Device {
  /** Valid for this OpenRGB process only; never persist. */
  id: number
  /** Survives an OpenRGB restart; scenes name devices by this. */
  key: DeviceKey
  name: string
  vendor: string
  /** What the info panel lists; any may be empty. */
  description: string
  version: string
  serial: string
  location: string
  device_type: number
  active_mode: number
  modes: ModeView[]
  zones: Zone[]
  /** How it sits on the desk map: one piece, or one per segment plus the LEDs left over. */
  pieces: Piece[]
  colors: string[]
}

export const serverState = () => invoke<ServerState | null>('server_state')
export const devices = () => invoke<Device[]>('devices')
export const resizeZone = (device: number, zone: number, size: number) =>
  invoke<void>('resize_zone', { device, zone, size })
/** Keep a built-in mode look in the device's own memory (only modes with `saveable`). */
export const saveMode = (device: number, effect: EffectSpec) => invoke<void>('save_mode', { device, effect })
/** Split a zone into runs from its first LED (OpenRGB segments); none joins it back. */
export const setSegments = (device: number, zone: number, runs: { name: string; leds: number }[]) =>
  invoke<void>('set_segments', { device, zone, runs })
export const retryServer = () => invoke<void>('retry_server')

/** The user's name for a device and whether freeRGB leaves it alone (mirrors `DeviceNote`). */
export interface DeviceNote {
  device: DeviceKey
  name?: string | null
  hidden: boolean
}
export const deviceNotes = () => invoke<DeviceNote[]>('device_notes')
export const setDeviceNote = (device: DeviceKey, name: string | null, hidden: boolean) =>
  invoke<void>('set_device_note', { device, name, hidden })
export const onDeviceNotes = (f: (n: DeviceNote[]) => void) => listen<DeviceNote[]>('device-notes', (e) => f(e.payload))

/** Whether OpenRGB is looking for devices; `percent` is null until it reports progress and
 *  `label` names the detector running. */
export interface Detection {
  scanning: boolean
  percent: number | null
  label: string
}
export const detection = () => invoke<Detection>('detection')
export const rescanDevices = () => invoke<void>('rescan_devices')
export const onDetection = (f: (d: Detection) => void) => listen<Detection>('detection', (e) => f(e.payload))

export const onServerState = (f: (s: ServerState) => void) =>
  listen<ServerState>('server-state', (e) => f(e.payload))
export const onDevices = (f: (d: Device[]) => void) =>
  listen<Device[]>('devices', (e) => f(e.payload))

/** What a look can follow on this machine, and the readings now (later ones as events). */
/** One thing a look can follow here; `needs` says what a reading takes (another program running). */
export interface Offer {
  id: SourceId
  needs?: string
}
export const sources = () => invoke<Offer[]>('sources')
export const readings = () => invoke<Readings>('readings')
export const onReadings = (f: (r: Readings) => void) => listen<Readings>('readings', (e) => f(e.payload))

export type { DeviceKey, ModeDirection, Scene }

export const setScenes = (scenes: Scene[]) => invoke<void>('set_scenes', { scenes })
export const activeScenes = () => invoke<Scene[]>('scenes')
export const onScenes = (f: (s: Scene[]) => void) =>
  listen<Scene[]>('scenes', (e) => f(e.payload))

export interface Profiles {
  names: string[]
  /** The profile whose look is running now. */
  active: string | null
}
export const profiles = () => invoke<Profiles>('profiles')
export const onProfiles = (f: (p: Profiles) => void) =>
  listen<Profiles>('profiles', (e) => f(e.payload))
/** Saves what is running now; resolves to the cleaned-up name. */
export const saveProfile = (name: string) => invoke<string>('save_profile', { name })
export const applyProfile = (name: string) => invoke<void>('apply_profile', { name })
export const renameProfile = (from: string, to: string) => invoke<string>('rename_profile', { from, to })
export const deleteProfile = (name: string) => invoke<void>('delete_profile', { name })
/** Why saved profiles could not be loaded; given once. */
export const storeNotice = () => invoke<string | null>('store_notice')
/** Something the backend did on its own failed (a tray action). */
export const onNotice = (f: (message: string) => void) => listen<string>('notice', (e) => f(e.payload))
/** Whether the app starts (in the tray) when the user logs in. */
export const schedule = () => invoke<Schedule>('schedule')
export const setSchedule = (schedule: Schedule) => invoke<void>('set_schedule', { schedule })
export const onSchedule = (f: (s: Schedule) => void) => listen<Schedule>('schedule', (e) => f(e.payload))

export const autostart = () => invoke<boolean>('autostart')
export const setAutostart = (on: boolean) => invoke<boolean>('set_autostart', { on })
/** The daily update check, freeRGB's only contact outside this PC. */
export const updateChecks = () => invoke<boolean>('update_checks')
export const setUpdateChecks = (on: boolean) => invoke<boolean>('set_update_checks', { on })
/** The Microsoft Store build, which the Store updates. */
export const packaged = () => invoke<boolean>('packaged')
/** Forget everything freeRGB keeps (profiles, My effects, groups, map, schedule, names,
 *  settings), stop effects and starting at login. OpenRGB's own settings stay. */
export const resetAll = () => invoke<void>('reset_all')
export const fps = () => invoke<number>('fps')
export const deskMap = () => invoke<Placed[]>('desk_map')
export const groups = () => invoke<Group[]>('groups')
export const onGroups = (f: (g: Group[]) => void) => listen<Group[]>('groups', (e) => f(e.payload))
/** Create a group (`old` null) or change one; its look follows. Returns the group as saved. */
export const saveGroup = (old: string | null, name: string, devices: DeviceKey[]) =>
  invoke<Group>('save_group', { old, name, devices })
export const deleteGroup = (name: string) => invoke<void>('delete_group', { name })

/** A look kept under a name in "My effects" (mirrors `SavedEffect` in the app's store). */
export interface SavedEffect {
  name: string
  effect: EffectSpec
}
export const myEffects = () => invoke<SavedEffect[]>('my_effects')
export const onEffects = (f: (e: SavedEffect[]) => void) => listen<SavedEffect[]>('effects', (e) => f(e.payload))
export const saveEffect = (old: string | null, name: string, effect: EffectSpec) =>
  invoke<SavedEffect>('save_effect', { old, name, effect })
export const deleteEffect = (name: string) => invoke<void>('delete_effect', { name })
/** Opens a save dialog; the path written, or null when cancelled. */
export const exportEffect = (name: string) => invoke<string | null>('export_effect', { name })
/** Opens an open dialog; the effect added to My effects, or null when cancelled. */
export const importEffect = () => invoke<SavedEffect | null>('import_effect')
export const setFps = (fps: number) => invoke<number>('set_fps', { fps })
export const setDeskMap = (map: Placed[]) => invoke<void>('set_desk_map', { map })

export const serverLog = (epoch: number | null, after: number) => invoke<LogView>('server_log', { epoch, after })
export const onServerLog = (f: () => void) => listen<null>('server-log', () => f())
export const turnOnServerLog = () => invoke<void>('turn_on_server_log')
export const clearServerLog = () => invoke<void>('clear_server_log')
export const hardware = () => invoke<Hardware>('hardware')

/** A newer freeRGB, found by the backend's daily check (mirrors `UpdateView`). */
export interface UpdateInfo {
  version: string
  /** The release's "What's new", as plain lines. */
  notes: string | null
}
export const updateStatus = () => invoke<UpdateInfo | null>('update_status')
export const onUpdate = (f: (u: UpdateInfo) => void) => listen<UpdateInfo>('update', (e) => f(e.payload))
/** Downloads, installs and restarts the app; resolves only if it could not. */
export const installUpdate = () => invoke<void>('install_update')
/** Download progress in whole percent. */
export const onUpdateProgress = (f: (percent: number) => void) => listen<number>('update-progress', (e) => f(e.payload))
/** Linux only (`null` elsewhere): whether OpenRGB's udev rules, which let it reach devices
 *  without root, are installed. */
export const udevRules = () => invoke<boolean | null>('udev_rules')
/** Installs OpenRGB's udev rules after a system password prompt; rejects when cancelled. */
export const installUdevRules = () => invoke<void>('install_udev_rules')
/** What the About sheet shows (mirrors `AboutView`). */
export interface About {
  version: string
  /** The bundled OpenRGB's version, when this install has one (Windows). */
  bundled_openrgb: string | null
}
export const about = () => invoke<About>('about')
/** The pages the window can open in the browser; the backend holds their addresses. */
export type LinkId = 'source' | 'issues' | 'releases' | 'sponsor' | 'kofi' | 'openrgb' | 'openrgb-source'
export const openLink = (id: LinkId) => invoke<void>('open_link', { id })
/** The tray asked for the About sheet while this window was closed (true once). */
export const takeAboutRequest = () => invoke<boolean>('take_about_request')
export const onAbout = (f: () => void) => listen<null>('about', () => f())
