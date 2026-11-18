// Looks that follow a reading, worked out as the engine does (crates/freergb-effects/src/
// react.rs, whose module docs say why it runs in two steps); react_vectors.json pins both.

import { parseHex, renderAt, type Drive, type EffectSpec, type React, type SourceId } from './effects.ts'
import type { Point } from './preview.ts'

/** The screen as a small grid of colours, row-major (mirrors `react::Screen`). */
export interface Screen {
  width: number
  height: number
  colors: string[]
}

/** The latest value of every running source (mirrors `react::Readings`). */
export interface Readings {
  cpu_temp?: number
  gpu_temp?: number
  cpu_load?: number
  gpu_load?: number
  sound?: number
  beat?: number
  spectrum?: number[]
  screen?: Screen
}

type RGB = [number, number, number]

/** The usual span of each reading. Mirrors `SourceId::range`. */
export const RANGE: Record<SourceId, [number, number]> = {
  cpu_temp: [35, 85],
  gpu_temp: [35, 85],
  cpu_load: [0, 100],
  gpu_load: [0, 100],
  sound: [0, 1],
  beat: [0, 1],
  spectrum: [0, 1],
  screen: [0, 1],
}

/** The settings a reading can move. Mirrors `SourceId::drives`. */
export const drivesOf = (s: SourceId, d: Drive) => (s === 'screen' ? d === 'color' : s === 'spectrum' ? d === 'brightness' : true)

export function rangeOf(m: React): [number, number] {
  const [lo, hi] = RANGE[m.source]
  return [m.low ?? lo, m.high ?? hi]
}

/** `v` placed in the range after the sensitivity. Mirrors `React::level`. */
export function levelOf(m: React, v: number): number {
  const [lo, hi] = rangeOf(m)
  if (!(hi > lo)) return 0
  const placed = (v - lo) / (hi - lo)
  const k = m.sensitivity === undefined ? placed : placed * Math.pow(4, 2 * m.sensitivity - 1)
  return Math.min(Math.max(k, 0), 1)
}

/** A level as a factor on brightness or amount. Mirrors `React::swing`. */
export const swingOf = (m: React, level: number) => (m.strength === undefined ? level : 1 - m.strength * (1 - level))
/** A level as a place along the palette. Mirrors `React::travel`. */
const travelOf = (m: React, level: number) => (m.strength === undefined ? level : level * m.strength)

/** A single-number reading, undefined for a per-LED source or a missing value. */
export function valueOf(r: Readings, s: SourceId): number | undefined {
  const v = s === 'spectrum' || s === 'screen' ? undefined : r[s]
  return v !== undefined && Number.isFinite(v) ? v : undefined
}

/** The look for this frame with a reading folded into brightness or amount. Mirrors
 *  `react::frame_spec`. */
export function frameSpec(spec: EffectSpec, r: Readings): EffectSpec {
  const m = spec.react
  if (!m) return spec
  const v = valueOf(r, m.source)
  if (v === undefined) return spec
  const level = levelOf(m, v)
  if (m.drives === 'brightness') return { ...spec, brightness: (spec.brightness ?? 1) * swingOf(m, level) }
  if (m.drives === 'amount') return { ...spec, density: (spec.density ?? 1) * (0.1 + 0.9 * swingOf(m, level)) }
  return spec
}

function bandAt(bands: number[], x: number): number {
  const n = bands.length
  if (n === 1) return bands[0]
  const pos = Math.min(Math.max(x, 0), 1) * (n - 1)
  const i = Math.min(Math.floor(pos), n - 2)
  return bands[i] + (bands[i + 1] - bands[i]) * (pos - i)
}

function screenAt(s: Screen, desk: Point): RGB | undefined {
  const { width: w, height: h } = s
  if (w <= 0 || h <= 0 || s.colors.length < w * h) return undefined
  const cell = (v: number, n: number) => Math.min(Math.floor(Math.min(Math.max(v, 0), 1) * n), n - 1)
  return parseHex(s.colors[cell(desk[1], h) * w + cell(desk[0], w)])
}

function palette(colors: string[], u: number): RGB {
  const p = colors.map(parseHex)
  if (p.length === 0) return [255, 255, 255]
  if (p.length === 1) return p[0]
  const pos = u * (p.length - 1)
  const i = Math.min(Math.floor(pos), p.length - 2)
  const k = pos - i
  return [0, 1, 2].map((n) => p[i][n] + (p[i + 1][n] - p[i][n]) * k) as RGB
}

const ch = (v: number) => Math.min(Math.max(Math.round(v), 0), 255)
const scale = (c: RGB, k: number): RGB => [ch(c[0] * k), ch(c[1] * k), ch(c[2] * k)]
/** `to` at the lit level of `c`, so motion and brightness stay. */
function tint(c: RGB, to: RGB): RGB {
  const lit = Math.max(c[0], c[1], c[2]) / 255
  return [ch(to[0] * lit), ch(to[1] * lit), ch(to[2] * lit)]
}

/** LED `c` after the per-LED part of the link. Mirrors `react::per_led`. */
export function perLed(spec: EffectSpec, r: Readings, c: RGB, x: number, desk: Point): RGB {
  const m = spec.react
  if (!m) return c
  if (m.source === 'spectrum') return r.spectrum?.length ? scale(c, swingOf(m, levelOf(m, bandAt(r.spectrum, x)))) : c
  if (m.source === 'screen') {
    const px = r.screen && screenAt(r.screen, desk)
    return px ? tint(c, px) : c
  }
  if (m.drives === 'color') {
    const v = valueOf(r, m.source)
    return v === undefined ? c : tint(c, palette(spec.colors ?? [], travelOf(m, levelOf(m, v))))
  }
  return c
}

/** One LED, both steps. Mirrors `react::render_led`. */
export const renderLed = (spec: EffectSpec, r: Readings, t: number, x: number, y: number, desk: Point): RGB =>
  perLed(spec, r, renderAt(frameSpec(spec, r), t, x, y), x, desk)
