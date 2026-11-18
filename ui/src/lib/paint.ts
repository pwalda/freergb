// Pure helpers for painting a device by hand. A painting is a `painted` scene: one colour per
// LED of the device, by device-wide index (zones concatenated), black where nothing was painted.

import type { Device, Zone } from './api.ts'
import { buildScene, DEFAULT_PARAMS, type DeviceKey, type Scene } from './effects.ts'

export const OFF = '#000000'

/** What the painter starts from: the device's own painting if it has one, otherwise the
 *  colours it shows right now. Always exactly one colour per LED. */
export function startColors(device: Device, own: Scene | undefined): string[] {
  const from = own?.effect.type === 'painted' ? (own.effect.colors ?? []) : device.colors
  return Array.from({ length: device.colors.length }, (_, i) => from[i] ?? OFF)
}

/** `colors` with LEDs `start .. start + count` set to `color`. */
export function fill(colors: string[], start: number, count: number, color: string): string[] {
  return colors.map((c, i) => (i >= start && i < start + count ? color : c))
}

const rgb = (c: string) => [1, 3, 5].map((i) => parseInt(c.slice(i, i + 2), 16) || 0)

/** Whether two colours look alike on a swatch row (RGB distance below 48 of 441). */
export function near(a: string, b: string): boolean {
  const [x, y] = [rgb(a), rgb(b)]
  return Math.hypot(x[0] - y[0], x[1] - y[1], x[2] - y[2]) < 48
}

/** The colours used in a painting, most used first, so they can be picked again. Colours that
 *  look alike count as one (a frozen gradient would otherwise fill the row with one hue);
 *  off and near-black are left out, the painter offers Off on its own. */
export function paletteOf(colors: string[], max = 8): string[] {
  const exact = new Map<string, number>()
  for (const c of colors) {
    const k = c.toLowerCase()
    if (!near(k, OFF)) exact.set(k, (exact.get(k) ?? 0) + 1)
  }
  const reps: { c: string; n: number }[] = []
  for (const [c, n] of [...exact.entries()].sort((a, b) => b[1] - a[1])) {
    const r = reps.find((r) => near(r.c, c))
    if (r) r.n += n
    else reps.push({ c, n })
  }
  return reps
    .sort((a, b) => b.n - a.n)
    .slice(0, max)
    .map((r) => r.c)
}

/** `list` with `c` added at the end, unless a colour like it is already there. */
export const withColor = (list: string[], c: string) =>
  list.some((x) => near(x, c)) ? list : [...list, c.toLowerCase()]

/** How a zone is drawn: its own grid, or its LEDs in a wrapping row. */
export type ZoneLayout =
  | { kind: 'matrix'; width: number; cells: (number | null)[] }
  | { kind: 'strip'; leds: number[] }

export function layoutOf(zone: Zone): ZoneLayout {
  if (zone.matrix && zone.matrix.width > 0) {
    return { kind: 'matrix', width: zone.matrix.width, cells: zone.matrix.cells }
  }
  return { kind: 'strip', leds: Array.from({ length: zone.leds }, (_, i) => zone.start + i) }
}

export function paintedScene(key: DeviceKey, colors: string[], brightness = 1): Scene {
  return buildScene({ ...DEFAULT_PARAMS, kind: 'painted', colors, brightness }, { devices: [key] })
}

/** The LED an arrow key (or Home/End) moves to from `led`, or `led` at an edge. Grids move by
 *  row and column and skip empty cells; rows of LEDs move by one. */
export function step(layout: ZoneLayout, led: number, key: string): number {
  if (layout.kind === 'strip') {
    const i = layout.leds.indexOf(led)
    const to = { ArrowLeft: i - 1, ArrowUp: i - 1, ArrowRight: i + 1, ArrowDown: i + 1, Home: 0, End: layout.leds.length - 1 }[key]
    return to === undefined ? led : (layout.leds[to] ?? led)
  }
  const { width, cells } = layout
  const lit = cells.filter((c): c is number => c !== null)
  if (key === 'Home') return lit[0] ?? led
  if (key === 'End') return lit.at(-1) ?? led
  const d = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -width, ArrowDown: width }[key]
  if (d === undefined) return led
  const row = (i: number) => Math.floor(i / width)
  let i = cells.indexOf(led)
  const r0 = row(i)
  for (;;) {
    i += d
    if (i < 0 || i >= cells.length || (Math.abs(d) === 1 && row(i) !== r0)) return led
    if (cells[i] !== null) return cells[i] as number
  }
}
