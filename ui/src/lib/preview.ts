// What each device shows on the stage, worked out the way the engine does it
// (crates/freergb-effects/src/engine.rs `plan` and layout.rs), so the preview matches the room.

import type { Device, Piece, Placed, Placement, SegmentKey } from './api.ts'
import { angleOf, parseHex, renderAt, type EffectSpec, type Scene } from './effects.ts'
import { renderLed, type Readings } from './react.ts'
import { sameKey } from './scenes.ts'

export type RGB = [number, number, number]
/** A colour at full intensity plus its level 0..1, so a dot can be drawn as "this hue, this lit". */
export type Lit = [number, number, number, number]

const linear = (i: number, n: number) => (n <= 1 ? 0.5 : i / (n - 1))

export type Point = [number, number]

// Layout in three steps, as in layout.rs (its module docs say why); layout_vectors.json pins
// `arrange` + `canvas` for both sides.

/** Each LED's spot in the device's unit square: controller order at mid-height, except matrix
 *  zones use their cell. Mirrors `layout::local_points`. */
export function localPoints(d: Device): Point[] {
  const n = d.colors.length
  const out: Point[] = Array.from({ length: n }, (_, i) => [linear(i, n), 0.5])
  for (const z of d.zones) {
    const m = z.matrix
    if (!m || m.width <= 0) continue
    const rows = Math.ceil(m.cells.length / m.width)
    m.cells.forEach((led, cell) => {
      if (led !== null && led < n) out[led] = [linear(cell % m.width, m.width), linear(Math.floor(cell / m.width), rows)]
    })
  }
  return out
}

/** A device's pieces on the map (`Device.pieces`; a device without them is one piece). */
export const piecesOf = (d: Device): Piece[] =>
  d.pieces?.length ? d.pieces : [{ segment: null, name: '', leds: d.colors.map((_, i) => i) }]

export const sameSegment = (a: SegmentKey | null | undefined, b: SegmentKey | null | undefined) =>
  (a ?? null) === null ? (b ?? null) === null : !!b && a!.zone === b.zone && a!.start === b.start

/** The map entry for one piece of `d`. */
export const entryOfPiece = (map: Placed[], d: Device, segment: SegmentKey | null) =>
  map.find((p) => sameKey(p.device, d.key) && sameSegment(p.segment, segment))

/** Each piece of `d` with its LED spots and placement: drawn spots while they fit, else the
 *  device's own spots for the rest and a straight run for a segment. Mirrors
 *  `layout::placed_pieces`. */
export function placedPieces(d: Device, map: Placed[]): { leds: number[]; local: Point[]; at: Placement | null }[] {
  const all = localPoints(d)
  return piecesOf(d).map((p) => {
    const e = entryOfPiece(map, d, p.segment)
    const k = p.leds.length
    const local: Point[] =
      e?.points && e.points.length === k ? e.points : p.segment ? p.leds.map((_, i) => [linear(i, k), 0.5]) : p.leds.map((i) => all[i])
    return { leds: p.leds, local, at: e?.at ?? null }
  })
}

const rad = (deg: number) => (deg * Math.PI) / 180

/** `local` placed on the desk. Mirrors `layout::on_desk`. */
export function onDesk(local: Point[], at: Placement): Point[] {
  const s = Math.sin(rad(at.rot))
  const c = Math.cos(rad(at.rot))
  return local.map(([lx, ly]) => {
    const dx = (lx - 0.5) * at.w
    const dy = (ly - 0.5) * at.h
    return [at.x + dx * c - dy * s, at.y + dx * s + dy * c]
  })
}

/** Desk points per device; unplaced devices take square slots right of the placed ones.
 *  Mirrors `layout::arrange`. */
export function arrange(pieces: { local: Point[]; at: Placement | null }[]): Point[][] {
  const placed = pieces.map((p) => (p.at ? onDesk(p.local, p.at) : null))
  const min = [Infinity, Infinity]
  const max = [-Infinity, -Infinity]
  for (const pts of placed) for (const p of pts ?? []) for (const k of [0, 1]) {
    min[k] = Math.min(min[k], p[k])
    max[k] = Math.max(max[k], p[k])
  }
  let x0 = 0, y0 = 0, size = 1
  if (Number.isFinite(min[0])) {
    const h = max[1] - min[1]
    x0 = max[0]
    y0 = min[1]
    size = h > 0 ? h : 1
  }
  let next = 0
  return pieces.map((p, i) => {
    const desk = placed[i]
    if (desk) return desk
    const at = { x: x0 + size * (next + 0.5), y: y0 + size * 0.5, w: size, h: size, rot: 0 }
    next++
    return onDesk(p.local, at)
  })
}

const dist = (a: Point, b: Point) => Math.sqrt((a[0] - b[0]) ** 2 + (a[1] - b[1]) ** 2)

function normalised(vals: number[]): number[] {
  let lo = Infinity, hi = -Infinity
  for (const v of vals) {
    lo = Math.min(lo, v)
    hi = Math.max(hi, v)
  }
  const span = hi - lo
  return vals.map((v) => (span > 0 ? Math.min(Math.max((v - lo) / span, 0), 1) : 0))
}

/** `(u, v)` in 0..1 per desk point for an effect moving at `angle` degrees: u along it, v
 *  across; points on one line use their place along it. Mirrors `layout::canvas`. */
export function canvas(points: Point[], angle: number): Point[] {
  const n = points.length
  if (n === 0) return []
  const s = Math.sin(rad(angle))
  const c = Math.cos(rad(angle))
  const along: Point = [c, s]
  const across: Point = [-s, c]
  const dot = (p: Point, d: Point) => p[0] * d[0] + p[1] * d[1]
  const p0 = points[0]
  let far = p0
  for (const p of points) if (dist(p, p0) > dist(far, p0)) far = p
  const len = dist(far, p0)
  if (len <= 0) return points.map(() => [0.5, 0])
  const line: Point = [(far[0] - p0[0]) / len, (far[1] - p0[1]) / len]
  const normal: Point = [-line[1], line[0]]
  if (points.every((p) => Math.abs(dot([p[0] - p0[0], p[1] - p0[1]], normal)) <= 1e-9 * len)) {
    const flip = dot(line, along) < -1e-9
    return normalised(points.map((p) => dot(p, line))).map((u) => [flip ? 1 - u : u, 0])
  }
  const u = normalised(points.map((p) => dot(p, along)))
  const v = normalised(points.map((p) => dot(p, across)))
  return u.map((x, i) => [x, v[i]])
}

/** The latest readings for looks that follow one; every preview below draws with them. */
let current: Readings = {}
export const setReadings = (r: Readings) => (current = r)

export type Look =
  /** A software effect evaluated per LED at canvas position `pos[led]` (u, v); `desk[led]` is
   *  its place on the scene's desk area, where a look that follows the screen samples it. */
  | { kind: 'effect'; spec: EffectSpec; pos: Point[]; desk: Point[] }
  /** Fixed colours per LED: a painting, a built-in mode or the device's own hardware state as
   *  OpenRGB reports it. `level` dims colours the app is not driving. */
  | { kind: 'colors'; colors: RGB[]; level: number }

/** The id a group's live edits go by (devices use their `keyId`). */
export const groupId = (name: string) => `group:${name}`

/** Live edits that have not been sent yet (a slider still moving), shown ahead of the scene. */
export interface Live {
  all?: EffectSpec
  dev?: { id: string; spec: EffectSpec }
}

/** The look device `i` shows: the last scene that targets it wins, as in the engine. */
export function lookOf(devices: Device[], i: number, scenes: Scene[], live: Live = {}, id = '', map: Placed[] = []): Look {
  const d = devices[i]
  let members: Device[] = []
  let scene: Scene | undefined
  for (const s of scenes) {
    if (s.targets === 'all') {
      scene = s
      members = devices
      continue
    }
    const present = s.targets.devices.flatMap((k) => devices.filter((x) => sameKey(x.key, k)).slice(0, 1))
    if (present.includes(d)) {
      scene = s
      members = present
    }
  }
  const reported = (level: number): Look => ({ kind: 'colors', colors: d.colors.map(parseHex), level })
  if (!scene) return reported(0.4)
  let spec = scene.effect
  if (scene.targets === 'all' && live.all) spec = live.all
  else if (scene.targets !== 'all' && live.dev && live.dev.id === (scene.group ? groupId(scene.group) : id) && live.dev.spec.type === spec.type)
    spec = live.dev.spec
  if (spec.type === 'device_mode') return reported(1)
  if (spec.type === 'painted') {
    const b = spec.brightness ?? 1
    return { kind: 'colors', colors: (spec.colors ?? []).map(parseHex), level: b }
  }
  return { kind: 'effect', spec, ...canvasOf(d, members, scene.canvas === 'per_device', spec, map) }
}

/** Where `d`'s LEDs sit on the canvas of `members` (in target order; its own canvas when
 *  `perDevice`) and on their desk area, each device's pieces placed by their own entries as in
 *  the engine's `plan`. */
function canvasOf(d: Device, members: Device[], perDevice: boolean, spec: EffectSpec, map: Placed[]): { pos: Point[]; desk: Point[] } {
  const pieces = members.flatMap((m) => placedPieces(m, map).map((p) => ({ ...p, mine: m === d })))
  const desk = arrange(pieces)
  const area = deskArea(desk)
  const mine = pieces.flatMap((p, k) => (p.mine ? [k] : []))
  const uv = perDevice ? canvas(mine.flatMap((k) => desk[k]), angleOf(spec)) : canvas(desk.flat(), angleOf(spec))
  const pos: Point[] = d.colors.map(() => [0, 0])
  const deskPos: Point[] = d.colors.map(() => [0, 0])
  let at = 0
  pieces.forEach((p, k) => {
    if (p.mine) {
      p.leds.forEach((led, j) => {
        pos[led] = uv[at + j]
        deskPos[led] = area(desk[k][j])
      })
      if (perDevice) at += desk[k].length
    }
    if (!perDevice) at += desk[k].length
  })
  return { pos, desk: deskPos }
}

/** Desk points to 0..1 over their bounding box, each axis on its own. Mirrors the engine's
 *  `desk_area`. */
function deskArea(desk: Point[][]): (p: Point) => Point {
  const min = [Infinity, Infinity]
  const max = [-Infinity, -Infinity]
  for (const pts of desk) for (const p of pts) for (const k of [0, 1]) {
    min[k] = Math.min(min[k], p[k])
    max[k] = Math.max(max[k], p[k])
  }
  const f = (p: Point, k: number) => (max[k] - min[k] > 0 ? (p[k] - min[k]) / (max[k] - min[k]) : 0.5)
  return (p) => [f(p, 0), f(p, 1)]
}

/** An effect's colour at `x` split into hue and level. A black phase keeps `fallback`'s hue. */
export function litAt(spec: EffectSpec, t: number, x: number, fallback: RGB = [255, 255, 255], y = 0, desk: Point = [x, 0.5]): Lit {
  const [r, g, b] = spec.react ? renderLed(spec, current, t, x, y, desk) : renderAt(spec, t, x, y)
  const peak = Math.max(r, g, b)
  if (peak === 0) return [...fallback, 0]
  const k = 255 / peak
  return [r * k, g * k, b * k, peak / 255]
}

/** A vial's liquid: the look at nine stops along it, eased over time. Vials draw a hue at full
 *  strength whatever its level, so sampling the look directly makes sparse or dark-gapped
 *  effects (starfield, rain, comet, breathing) snap between colours and flicker. */
export interface Glow {
  hue: RGB[]
  level: number[]
}
export const GLOW_STOPS = 9
const GLOW_SPREAD = 1 / 16
const GLOW_TAPS = 5
const LEVEL_TAU = 0.12
const HUE_TAU = 0.25

/** Moves `prev` (none on the first frame) towards the look at time `t`, `dt` seconds later. */
export function glow(spec: EffectSpec, t: number, dt: number, prev: Glow | undefined, fallback: RGB = [255, 255, 255]): Glow {
  const kl = 1 - Math.exp(-dt / LEVEL_TAU)
  const kh = 1 - Math.exp(-dt / HUE_TAU)
  const next: Glow = { hue: [], level: [] }
  for (let i = 0; i < GLOW_STOPS; i++) {
    // A few taps around the stop, so one twinkling LED does not decide the colour.
    const sum: RGB = [0, 0, 0]
    for (let j = 0; j < GLOW_TAPS; j++) {
      const x = Math.min(Math.max(i / (GLOW_STOPS - 1) + GLOW_SPREAD * (2 * j / (GLOW_TAPS - 1) - 1), 0), 1)
      const c = spec.react ? renderLed(spec, current, t, x, 0, [x, 0.5]) : renderAt(spec, t, x)
      for (let n = 0; n < 3; n++) sum[n] += c[n] / GLOW_TAPS
    }
    const peak = Math.max(...sum)
    const level = peak / 255
    const target: RGB | undefined = peak > 1 ? [(sum[0] * 255) / peak, (sum[1] * 255) / peak, (sum[2] * 255) / peak] : undefined
    const was = prev?.hue[i]
    if (!was) {
      next.hue.push(target ?? fallback)
      next.level.push(level)
      continue
    }
    // In the dark the hue holds; when lit it drifts to the new one rather than jumping.
    const h: RGB = target ? [was[0] + (target[0] - was[0]) * kh, was[1] + (target[1] - was[1]) * kh, was[2] + (target[2] - was[2]) * kh] : was
    const hp = Math.max(...h)
    next.hue.push(hp > 0 ? [(h[0] * 255) / hp, (h[1] * 255) / hp, (h[2] * 255) / hp] : fallback)
    next.level.push(prev.level[i] + (level - prev.level[i]) * kl)
  }
  return next
}

/** The glow at `u` (0..1) along the liquid, at the nearest stop. */
export function glowAt(g: Glow, u: number): Lit {
  const i = Math.min(Math.max(Math.round(u * (GLOW_STOPS - 1)), 0), GLOW_STOPS - 1)
  return [...g.hue[i], g.level[i]]
}

/** What LED `led` of a device with this look shows at time `t`. */
export function ledLit(look: Look, led: number, t: number): Lit {
  if (led < 0) return [0, 0, 0, 0]
  if (look.kind === 'effect') {
    const fb = look.spec.colors?.[0] ? parseHex(look.spec.colors[0]) : undefined
    const [u, v] = look.pos[led] ?? [0.5, 0]
    return litAt(look.spec, t, u, fb, v, look.desk[led])
  }
  const c = look.colors[led]
  if (!c) return [0, 0, 0, 0]
  const peak = Math.max(c[0], c[1], c[2])
  if (peak === 0) return [0, 0, 0, 0]
  const k = 255 / peak
  return [c[0] * k, c[1] * k, c[2] * k, (peak / 255) * look.level]
}

/** A short name for a look, used to spot a change worth the wipe (not a slider retune). */
export function lookName(look: Look): string {
  if (look.kind === 'effect') return look.spec.type
  return `colors:${look.level}`
}
