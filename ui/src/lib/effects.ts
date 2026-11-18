// Webview mirror of crates/freergb-effects/src/render.rs. The formulas must stay identical;
// effects.test.ts checks every sample in crates/freergb-effects/tests/vectors.json.

export type Direction = 'forward' | 'reverse'
export type EffectKind =
  | 'static'
  | 'gradient'
  | 'spectrum_cycle'
  | 'rainbow'
  | 'breathing'
  | 'wave'
  | 'comet'
  | 'marquee'
  | 'bouncing_beam'
  | 'starfield'
  | 'fire'
  | 'rain'
  | 'painted'
  | 'device_mode'

/** Directions a device's built-in mode can offer (mirrors `ModeDirection` in freergb-effects). */
export type ModeDirection =
  | 'left'
  | 'right'
  | 'up'
  | 'down'
  | 'horizontal'
  | 'vertical'
  | 'up_left'
  | 'up_right'
  | 'down_left'
  | 'down_right'

export interface EffectSpec {
  type: EffectKind
  colors?: string[]
  /** Cycles per second. */
  speed?: number
  direction?: Direction
  /** 0..1 */
  brightness?: number
  /** Spatial cycles across the canvas. */
  density?: number
  /** `device_mode` only: the device's own mode. Speed and brightness are then positions 0..1. */
  mode?: string
  mode_direction?: ModeDirection
  /** Which way a software effect moves across the desk map, degrees 0..360 (0 = right, 90 =
   *  down). Unset: the effect's own default, see `angleOf`. */
  angle?: number
  /** `device_mode` only: zones running a mode of their own; the rest follow the device's. */
  zones?: ZoneLook[]
  /** Software effects only: one setting follows a reading (see react.ts). */
  react?: React
}

/** What a look can follow (mirrors `react::SourceId`). */
export type SourceId = 'cpu_temp' | 'gpu_temp' | 'cpu_load' | 'gpu_load' | 'sound' | 'beat' | 'spectrum' | 'screen'
/** Which setting a reading moves (mirrors `react::Drive`). */
export type Drive = 'brightness' | 'color' | 'amount'
/** A look's link to a reading (mirrors `react::React`). */
export interface React {
  source: SourceId
  drives: Drive
  /** The reading at the setting's low and high end; unset takes the source's own range. */
  low?: number
  high?: number
  /** 0..1, how readily the setting moves; unset is 0.5, the reading as it is. */
  sensitivity?: number
  /** 0..1, how far the setting swings; unset is 1, all the way. */
  strength?: number
}

/** One zone's own built-in mode inside a `device_mode` look (mirrors `ZoneLook`). */
export interface ZoneLook {
  /** Index in the device's zone list. */
  zone: number
  mode: string
  colors: string[]
  speed: number
  brightness: number
  mode_direction?: ModeDirection
}

/** Mirrors `EffectSpec::angle`: rain falls, fire rises, everything else runs left to right. */
export const angleOf = (spec: EffectSpec): number =>
  spec.angle ?? (spec.type === 'rain' ? 90 : spec.type === 'fire' ? 270 : 0)

type F3 = [number, number, number]

const frac = (v: number) => v - Math.floor(v)
const lerp = (a: F3, b: F3, u: number): F3 => [
  a[0] + (b[0] - a[0]) * u,
  a[1] + (b[1] - a[1]) * u,
  a[2] + (b[2] - a[2]) * u,
]

export function parseHex(hex: string): F3 {
  const h = hex.replace(/^#/, '')
  return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)]
}

function sample(palette: F3[], u: number): F3 {
  if (palette.length === 0) return [0, 0, 0]
  if (palette.length === 1) return palette[0]
  const pos = Math.min(Math.max(u, 0), 1) * (palette.length - 1)
  const i = Math.min(Math.floor(pos), palette.length - 2)
  return lerp(palette[i], palette[i + 1], pos - i)
}

function hue(h: number): F3 {
  const h6 = frac(h) * 6
  const f = h6 - Math.floor(h6)
  const [r, g, b] = (
    [
      [1, f, 0],
      [1 - f, 1, 0],
      [0, 1, f],
      [0, 1 - f, 1],
      [f, 0, 1],
      [1, 0, 1 - f],
    ] as F3[]
  )[Math.min(Math.floor(h6), 5)]
  return [r * 255, g * 255, b * 255]
}

// Hashed randomness, bit for bit as in render.rs: 32-bit integer maths via Math.imul and >>>.

function hash(a: number, b: number): number {
  let h = Math.imul(a, 0x9e3779b1) ^ Math.imul(b, 0x85ebca77)
  h ^= h >>> 16
  h = Math.imul(h, 0x7feb352d)
  h ^= h >>> 15
  h = Math.imul(h, 0x846ca68b)
  h ^= h >>> 16
  return h >>> 0
}

const unit = (h: number) => h / 4294967296
/** `floor(v)` wrapped to 32 bits, as Rust's `v.floor() as i64 as u32`. */
const cell = (v: number) => Math.floor(v) >>> 0
const smooth = (f: number) => f * f * (3 - 2 * f)

function noise(a: number, b: number, salt: number): number {
  const ia = cell(a)
  const ib = cell(b)
  const fa = smooth(a - Math.floor(a))
  const fb = smooth(b - Math.floor(b))
  const corner = (i: number, j: number) => unit(hash(hash(i, salt), j))
  const ia1 = (ia + 1) >>> 0
  const ib1 = (ib + 1) >>> 0
  const top = corner(ia, ib) + (corner(ia1, ib) - corner(ia, ib)) * fa
  const bottom = corner(ia, ib1) + (corner(ia1, ib1) - corner(ia, ib1)) * fa
  return top + (bottom - top) * fb
}

const pick = (palette: F3[], h: number): F3 =>
  palette.length === 0 ? [0, 0, 0] : palette[h % palette.length]

function tail(d: number, len: number): number {
  if (d >= 0 && d < len) {
    const k = 1 - d / len
    return k * k
  }
  return 0
}

const COMET_TAIL = 0.4
const MARQUEE_DASHES = 8
const BEAM_WIDTH = 0.15
const STAR_CELLS = 4096
const FIRE_SCALE = 6
const FIRE_RATE = 10
const RAIN_TAIL = 0.12
const RAIN_MAX = 32
const RAIN_LANES = 12

/** Rust's `f64::round` rounds halves away from zero; for the non-negative values here that is
 *  `Math.round`. */
const channel = (v: number, brightness: number) =>
  Math.min(Math.max(Math.round(v * brightness), 0), 255)

/** The colour of one LED at time `t` (seconds) and canvas position `x` along the effect's
 *  direction, `y` across it (both 0..1; a strip has `y = 0`), as [r, g, b]. */
export function renderAt(spec: EffectSpec, t: number, x: number, y = 0): [number, number, number] {
  const colors = (spec.colors ?? []).map(parseHex)
  const speed = spec.speed ?? 0.2
  const density = spec.density ?? 1
  const brightness = spec.brightness ?? 1
  const sign = (spec.direction ?? 'forward') === 'forward' ? 1 : -1
  let c: F3
  switch (spec.type) {
    case 'static':
      c = sample(colors, 0)
      break
    case 'gradient':
      c = sample(colors, sign > 0 ? x : 1 - x)
      break
    case 'spectrum_cycle':
      c = hue(sign * speed * t)
      break
    case 'rainbow':
      c = hue(density * x - sign * speed * t)
      break
    case 'breathing': {
      const cycles = speed * t
      const n = Math.max(colors.length, 1)
      const idx = Math.floor(Math.max(Math.floor(cycles), 0)) % n
      const level = 0.5 - 0.5 * Math.cos(2 * Math.PI * frac(cycles))
      c = lerp([0, 0, 0], colors[idx] ?? [0, 0, 0], level)
      break
    }
    case 'wave': {
      const phase = density * x - sign * speed * t
      const u = 0.5 - 0.5 * Math.cos(2 * Math.PI * phase)
      c = colors.length === 1 ? lerp([0, 0, 0], colors[0], u) : sample(colors, u)
      break
    }
    case 'comet': {
      const d = frac(speed * t - sign * density * x)
      c = lerp([0, 0, 0], sample(colors, d / COMET_TAIL), tail(d, COMET_TAIL))
      break
    }
    case 'marquee': {
      const q = MARQUEE_DASHES * density * x - sign * speed * t
      if (frac(q) < 0.5) {
        const n = Math.max(colors.length, 1)
        c = pick(colors, ((Math.floor(q) % n) + n) % n)
      } else c = [0, 0, 0]
      break
    }
    case 'bouncing_beam': {
      let at = 0.5 - 0.5 * Math.cos(2 * Math.PI * speed * t)
      if (sign < 0) at = 1 - at
      const off = Math.min(Math.abs(x - at) / (BEAM_WIDTH / density), 1)
      const level = 0.5 + 0.5 * Math.cos(Math.PI * off)
      c = lerp([0, 0, 0], sample(colors, at), level)
      break
    }
    case 'starfield': {
      const k = (cell(x * STAR_CELLS) + Math.imul(cell(y * STAR_CELLS), STAR_CELLS + 1)) >>> 0
      const rate = speed * (0.5 + unit(hash(k, 0x51a2)))
      const cycles = rate * t + unit(hash(k, 0x51a3))
      const h = hash(hash(k, cell(cycles)), 0x51a4)
      if (unit(h) < Math.min(0.25 * density, 1)) {
        const s = Math.sin(Math.PI * frac(cycles))
        c = lerp([0, 0, 0], pick(colors, hash(h, 1)), s * s)
      } else c = [0, 0, 0]
      break
    }
    case 'fire': {
      const u = sign > 0 ? x : 1 - x
      const a = FIRE_SCALE * density * u
      const s = FIRE_RATE * speed * t
      const b = FIRE_SCALE * density * y
      const n = 0.6 * noise(a - s, s + b, 0xf1) + 0.4 * noise(2 * a - 2 * s, 2 * (s + b), 0xf2)
      const heat = Math.min(Math.max((1 - 0.75 * u) * (0.25 + n), 0), 1)
      c = lerp([0, 0, 0], sample(colors, heat), Math.min(1.6 * heat, 1))
      break
    }
    case 'rain': {
      const u = sign > 0 ? x : 1 - x
      const drops = Math.min(Math.max(Math.round(4 * density), 1), RAIN_MAX)
      const lane = Math.min(Math.max(Math.floor(y * RAIN_LANES), 0), RAIN_LANES - 1)
      c = [0, 0, 0]
      for (let i = 0; i < drops; i++) {
        const j = i + lane * RAIN_MAX
        const cycles = speed * t + unit(hash(j, 0xa1))
        const h = hash(hash(j, cell(cycles)), 0xa2)
        const head = frac(cycles) * (1 + unit(h)) * (1 + RAIN_TAIL)
        const level = tail(head - u, RAIN_TAIL)
        if (level > 0) {
          const d = lerp([0, 0, 0], pick(colors, hash(h, 1)), level)
          c = [Math.max(c[0], d[0]), Math.max(c[1], d[1]), Math.max(c[2], d[2])]
        }
      }
      break
    }
    case 'painted':
    case 'device_mode':
      // Neither depends on a canvas position: a painting is by LED index, and the hardware
      // animates its own mode.
      c = [0, 0, 0]
      break
  }
  return [channel(c[0], brightness), channel(c[1], brightness), channel(c[2], brightness)]
}

/** What identifies a device across OpenRGB restarts (mirrors `DeviceKey` in freergb-sdk). */
export interface DeviceKey {
  name: string
  location: string
  serial: string
  nth: number
}

export type Targets = 'all' | { devices: DeviceKey[] }

export interface Scene {
  effect: EffectSpec
  targets: Targets
  canvas: 'shared' | 'per_device'
  /** The look of this named group (see `Group` in api.ts); `targets` are its devices. */
  group?: string
}

/** How many colour pickers each effect uses. Effects with 0 ignore `colors`. */
export const COLOR_COUNT: Record<EffectKind, number> = {
  static: 1,
  gradient: 3,
  spectrum_cycle: 0,
  rainbow: 0,
  breathing: 3,
  wave: 3,
  comet: 2,
  marquee: 3,
  bouncing_beam: 2,
  starfield: 3,
  fire: 3,
  rain: 2,
  painted: 0, // one colour per LED, see paint.ts
  device_mode: 0, // a built-in mode's colours depend on the device, see ModeControls
}

export interface SceneParams {
  kind: EffectKind
  colors: string[]
  speed: number
  density: number
  brightness: number
  reverse: boolean
  perDevice: boolean
  /** Which way it moves across the desk, degrees; null or absent is the effect's own default. */
  angle?: number | null
  /** `device_mode` only. */
  mode?: string
  modeDirection?: ModeDirection | null
  zones?: ZoneLook[]
  /** A setting that follows a reading; null or absent follows nothing. */
  react?: React | null
}

/** Whether `kind` can let `drive` follow `source`: the screen gives colours and the spectrum
 *  levels (any look); a single reading moves brightness, the amount of a look that has one, or
 *  the colour of a look that has colours. */
export function canFollow(kind: EffectKind, source: SourceId, drive: Drive): boolean {
  if (kind === 'painted' || kind === 'device_mode') return false
  if (source === 'screen') return drive === 'color'
  if (source === 'spectrum') return drive === 'brightness'
  if (drive === 'amount') return !!EFFECTS.find((e) => e.id === kind)?.spatial
  if (drive === 'color') return COLOR_COUNT[kind] > 0
  return true
}

/** How many colours the look shows: its own count, or at least two (low and high end) while a
 *  reading picks along them. */
export const colorSlots = (p: Pick<SceneParams, 'kind' | 'react'>) =>
  p.react && p.react.drives === 'color' && p.react.source !== 'screen' && canFollow(p.kind, p.react.source, 'color')
    ? Math.max(COLOR_COUNT[p.kind], 2)
    : COLOR_COUNT[p.kind]

/** The effect the panel previews, and (in a scene) what it sends to the backend. */
export function buildEffect(p: SceneParams): EffectSpec {
  if (p.kind === 'painted') return { type: p.kind, colors: p.colors, brightness: p.brightness }
  if (p.kind === 'device_mode') {
    return {
      type: p.kind,
      mode: p.mode,
      colors: p.colors,
      speed: p.speed,
      brightness: p.brightness,
      ...(p.modeDirection ? { mode_direction: p.modeDirection } : {}),
      ...(p.zones?.length ? { zones: p.zones.map((z) => ({ ...z, colors: [...z.colors] })) } : {}),
    }
  }
  return {
    type: p.kind,
    colors: p.colors.slice(0, colorSlots(p)),
    speed: p.speed,
    density: p.density,
    brightness: p.brightness,
    direction: p.reverse ? 'reverse' : 'forward',
    ...(p.angle != null && AIMED.includes(p.kind) ? { angle: p.angle } : {}),
    ...(p.react && canFollow(p.kind, p.react.source, p.react.drives)
      ? {
          react: {
            source: p.react.source,
            drives: p.react.drives,
            ...(p.react.low != null ? { low: p.react.low } : {}),
            ...(p.react.high != null ? { high: p.react.high } : {}),
            ...(p.react.sensitivity != null ? { sensitivity: p.react.sensitivity } : {}),
            ...(p.react.strength != null ? { strength: p.react.strength } : {}),
          },
        }
      : {}),
  }
}

/** Effects that run in a direction across the desk, so the direction dial means something. */
export const AIMED: EffectKind[] = ['gradient', 'rainbow', 'wave', 'comet', 'marquee', 'bouncing_beam', 'fire', 'rain']

/** The exact object sent over IPC to `set_scenes`. The Rust side rejects unknown fields, so
 *  scene-fixtures.json pins this shape for both languages. */
export function buildScene(p: SceneParams, targets: Targets = 'all', group?: string): Scene {
  const scene: Scene = { effect: buildEffect(p), targets, canvas: p.perDevice ? 'per_device' : 'shared' }
  return group === undefined ? scene : { ...scene, group }
}

/** Shelves of the Looks rack, six vials each (number keys 1-6 pick from the open one). `mine`
 *  holds the user's saved effects instead of effect kinds. */
export const SHELVES = [
  { id: 'essentials', label: 'Essentials' },
  { id: 'scenes', label: 'Scenes' },
  { id: 'mine', label: 'My effects' },
] as const
export type ShelfId = (typeof SHELVES)[number]['id']

/** The effects the UI offers; `moves`/`spatial` say which controls make sense for each, and
 *  `amount` names what the density slider means for that effect. */
export const EFFECTS: {
  id: EffectKind
  label: string
  meta: string
  shelf: ShelfId
  moves: boolean
  spatial: boolean
  amount?: string
}[] = [
  { id: 'static', label: 'Static colour', meta: 'One colour', shelf: 'essentials', moves: false, spatial: false },
  { id: 'gradient', label: 'Gradient', meta: 'Blends across', shelf: 'essentials', moves: false, spatial: false },
  { id: 'spectrum_cycle', label: 'Spectrum cycle', meta: 'Hue over time', shelf: 'essentials', moves: true, spatial: false },
  { id: 'rainbow', label: 'Rainbow', meta: 'Hue across LEDs', shelf: 'essentials', moves: true, spatial: true },
  { id: 'breathing', label: 'Breathing', meta: 'Fades in and out', shelf: 'essentials', moves: true, spatial: false },
  { id: 'wave', label: 'Wave', meta: 'Travels along', shelf: 'essentials', moves: true, spatial: true },
  { id: 'comet', label: 'Comet', meta: 'Head and tail', shelf: 'scenes', moves: true, spatial: true, amount: 'Comets' },
  { id: 'marquee', label: 'Marquee', meta: 'Moving dashes', shelf: 'scenes', moves: true, spatial: true, amount: 'Dashes' },
  { id: 'bouncing_beam', label: 'Bouncing beam', meta: 'Sweeps to and fro', shelf: 'scenes', moves: true, spatial: true, amount: 'Focus' },
  { id: 'starfield', label: 'Starfield', meta: 'Random twinkles', shelf: 'scenes', moves: true, spatial: true, amount: 'Stars' },
  { id: 'fire', label: 'Fire', meta: 'Flickering flames', shelf: 'scenes', moves: true, spatial: true, amount: 'Detail' },
  { id: 'rain', label: 'Rain', meta: 'Falling drops', shelf: 'scenes', moves: true, spatial: true, amount: 'Drops' },
]

export const shelfEffects = (shelf: ShelfId) => EFFECTS.filter((e) => e.shelf === shelf)
export const shelfOf = (k: EffectKind): ShelfId | undefined => EFFECTS.find((e) => e.id === k)?.shelf

/** Colours a look starts from instead of carrying over the running look's: fire only reads as
 *  fire from embers to yellow. */
export const OWN_PALETTE: Partial<Record<EffectKind, string[]>> = {
  fire: ['#c81e00', '#ff7a00', '#ffd76a'],
}

export const effectLabel = (k: EffectKind) => EFFECTS.find((e) => e.id === k)?.label ?? k

export const DEFAULT_PARAMS: SceneParams = {
  kind: 'rainbow',
  colors: ['#ff3b30', '#0a84ff', '#30d158'],
  speed: 0.25,
  density: 1,
  brightness: 1,
  reverse: false,
  perDevice: false,
}

/** The editor settings that produce `scene`: the inverse of `buildScene`. Colour slots the
 *  effect does not use are filled (from its own colours, or the defaults when it has fewer than
 *  two), so switching effect kind later has colours to show. */
export function paramsOf(scene: Scene): SceneParams {
  const e = scene.effect
  if (e.type === 'device_mode') {
    return {
      ...DEFAULT_PARAMS,
      kind: e.type,
      colors: [...(e.colors ?? [])],
      speed: e.speed ?? 0.5,
      brightness: e.brightness ?? 1,
      mode: e.mode ?? '',
      modeDirection: e.mode_direction ?? null,
      ...(e.zones?.length ? { zones: e.zones.map((z) => ({ ...z, colors: [...z.colors] })) } : {}),
    }
  }
  if (e.type === 'painted') {
    return { ...DEFAULT_PARAMS, kind: e.type, colors: [...(e.colors ?? [])], brightness: e.brightness ?? 1 }
  }
  // Slots the effect does not use repeat its own colours, so switching to an effect with more
  // slots never brings in a default colour the user did not choose.
  const own = e.colors ?? []
  const colors = DEFAULT_PARAMS.colors.map((d, i) => own[i] ?? (own.length >= 2 ? own[i % own.length] : d))
  return {
    kind: e.type,
    colors,
    speed: e.speed ?? DEFAULT_PARAMS.speed,
    density: e.density ?? DEFAULT_PARAMS.density,
    brightness: e.brightness ?? DEFAULT_PARAMS.brightness,
    reverse: e.direction === 'reverse',
    perDevice: scene.canvas === 'per_device',
    angle: e.angle ?? null,
    react: e.react ? { ...e.react } : null,
  }
}
