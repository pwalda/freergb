// The stage is an LED dot matrix. Each device is drawn from a dot template on whole matrix
// cells; the user can pick which template a device uses (a motherboard drawn as fans, RAM as a
// keyboard). Templates have light "slots"; a device's LEDs are spread over them in order.

export type Shape = 'strip' | 'stick' | 'board' | 'keys' | 'fans' | 'ring'

export const SHAPES: { id: Shape; label: string }[] = [
  { id: 'strip', label: 'Strip' },
  { id: 'stick', label: 'Memory stick' },
  { id: 'board', label: 'Motherboard' },
  { id: 'keys', label: 'Keyboard' },
  { id: 'fans', label: 'Fans' },
  { id: 'ring', label: 'Ring' },
]

export const isShape = (v: unknown): v is Shape => SHAPES.some((s) => s.id === v)

/** A template cell: an outline dot ('o') or a light dot ('l') showing slot `k`. */
export interface Cell {
  x: number
  y: number
  t: 'o' | 'l'
  k: number
}

export interface Template {
  w: number
  h: number
  cells: Cell[]
  /** How many light slots the template has (slot ids run 0..slots-1). */
  slots: number
}

// OpenRGB device type ids (crates/freergb-protocol/src/ids.rs) to the template drawn by default.
const DEFAULT: Record<number, Shape> = { 0: 'board', 1: 'stick', 3: 'fans', 5: 'keys', 6: 'ring', 8: 'ring', 18: 'keys' }

export const defaultShape = (deviceType: number): Shape => DEFAULT[deviceType] ?? 'strip'

/** Which LED a slot shows: LEDs are spread evenly over the slots, in order. */
export const slotLed = (k: number, slots: number, leds: number) =>
  leds <= 0 ? -1 : Math.min(leds - 1, Math.floor((k * leds) / Math.max(slots, 1)))

// A full-size 104-key layout in key units: [x, row, width, tall]. Row 0 is the function row.
const KB: [number, number, number, boolean][] = []
{
  const a = (x: number, r: number, w = 1, tall = false) => KB.push([x, r, w, tall])
  a(0, 0)
  for (let i = 0; i < 4; i++) {
    a(2 + i, 0)
    a(6.5 + i, 0)
    a(11 + i, 0)
  }
  a(15.25, 0)
  a(16.25, 0)
  a(17.25, 0)
  for (let i = 0; i < 13; i++) a(i, 1)
  a(13, 1, 2)
  for (let i = 0; i < 3; i++) a(15.25 + i, 1)
  for (let i = 0; i < 4; i++) a(18.5 + i, 1)
  a(0, 2, 1.5)
  for (let i = 0; i < 12; i++) a(1.5 + i, 2)
  a(13.5, 2, 1.5)
  for (let i = 0; i < 3; i++) {
    a(15.25 + i, 2)
    a(18.5 + i, 2)
  }
  a(21.5, 2, 1, true)
  a(0, 3, 1.75)
  for (let i = 0; i < 11; i++) a(1.75 + i, 3)
  a(12.75, 3, 2.25)
  for (let i = 0; i < 3; i++) a(18.5 + i, 3)
  a(0, 4, 2.25)
  for (let i = 0; i < 10; i++) a(2.25 + i, 4)
  a(12.25, 4, 2.75)
  a(16.25, 4)
  for (let i = 0; i < 3; i++) a(18.5 + i, 4)
  a(21.5, 4, 1, true)
  for (const x of [0, 1.25, 2.5]) a(x, 5, 1.25)
  a(3.75, 5, 6.25)
  for (const x of [10, 11.25, 12.5, 13.75]) a(x, 5, 1.25)
  for (let i = 0; i < 3; i++) a(15.25 + i, 5)
  a(18.5, 5, 2)
  a(20.5, 5)
}
export const KEY_COUNT = KB.length

function ringCells(r: number): [number, number][] {
  const c: [number, number][] = []
  for (let dy = -r - 1; dy <= r + 1; dy++)
    for (let dx = -r - 1; dx <= r + 1; dx++) if (Math.abs(Math.hypot(dx, dy) - r) < 0.5) c.push([dx, dy])
  return c.sort((a, b) => Math.atan2(a[1], a[0]) - Math.atan2(b[1], b[0]))
}

/** The template for `shape` showing a device with `n` LEDs. A strip given a `length` (in dots)
 *  is one straight row of that many lights instead of rows of up to 30. */
export function template(shape: Shape, n: number, length?: number): Template {
  const cells: Cell[] = []
  const o = (x: number, y: number) => cells.push({ x, y, t: 'o', k: -1 })
  const l = (x: number, y: number, k: number) => cells.push({ x, y, t: 'l', k })
  // A frame of outline dots; `round` leaves the corners out.
  const box = (x0: number, y0: number, w: number, h: number, round: boolean) => {
    for (let x = x0; x < x0 + w; x++) {
      if (round && (x === x0 || x === x0 + w - 1)) continue
      o(x, y0)
      o(x, y0 + h - 1)
    }
    for (let y = y0 + 1; y < y0 + h - 1; y++) {
      o(x0, y)
      o(x0 + w - 1, y)
    }
  }
  const count = Math.max(n, 1)
  switch (shape) {
    case 'strip': {
      if (length) {
        const m = Math.max(1, Math.round(length))
        box(0, 0, m * 2 + 1, 3, true)
        for (let k = 0; k < m; k++) l(1 + 2 * k, 1, k)
        return { w: m * 2 + 1, h: 3, cells, slots: m }
      }
      // Rows of up to 30, running back and forth; a long strip is sampled down to 4 rows.
      const m = Math.min(count, 120)
      const cols = Math.min(m, 30)
      const rows = Math.ceil(m / 30)
      const w = cols * 2 + 1
      const h = rows * 2 + 1
      box(0, 0, w, h, true)
      for (let k = 0; k < m; k++) {
        const r = Math.floor(k / 30)
        const c = r % 2 ? cols - 1 - (k % 30) : k % 30
        l(1 + 2 * c, 1 + 2 * r, k)
      }
      return { w, h, cells, slots: m }
    }
    case 'stick': {
      const m = Math.min(count, 32)
      const cols = m > 16 ? 2 : 1
      const per = Math.ceil(m / cols)
      const w = cols * 2 + 1
      const h = per * 2 + 1
      box(0, 0, w, h, true)
      for (let k = 0; k < m; k++) l(1 + 2 * Math.floor(k / per), 1 + 2 * (k % per), k)
      return { w, h, cells, slots: m }
    }
    case 'board': {
      // Board outline, CPU socket, two slot lines; the LEDs run down the right edge, then the bottom.
      box(0, 0, 15, 17, true)
      box(2, 2, 5, 5, false)
      for (let x = 2; x <= 9; x++) {
        o(x, 10)
        o(x, 13)
      }
      o(9, 3)
      o(9, 5)
      const spots: [number, number][] = []
      for (let r = 2; r <= 14; r += 2) spots.push([12, r])
      for (let c = 10; c >= 2; c -= 2) spots.push([c, 15])
      const m = Math.min(count, spots.length)
      spots.slice(0, m).forEach(([x, y], k) => l(x, y, k))
      return { w: 15, h: 17, cells, slots: m }
    }
    case 'keys': {
      // 1 key unit = 2 cells; wide keys light a run of dots, tall keys two rows.
      box(0, 0, 47, 16, true)
      KB.forEach(([x, r, w, tall], k) => {
        const y = r === 0 ? 0 : 1 + r * 2
        const c0 = Math.round(x * 2)
        const c1 = Math.max(c0, Math.round((x + w) * 2) - 2)
        for (let cx = c0; cx <= c1; cx++) for (let cy = y; cy <= y + (tall ? 2 : 0); cy++) l(cx + 2, cy + 2, k)
      })
      return { w: 47, h: 16, cells, slots: KB.length }
    }
    case 'fans': {
      // One 120 mm frame per 16 LEDs (up to 6), each with a ring of light and a hub.
      const f = Math.max(1, Math.min(6, Math.ceil(count / 16)))
      const ring = ringCells(4)
      for (let i = 0; i < f; i++) {
        const x0 = i * 12
        box(x0, 0, 11, 11, true)
        o(x0 + 5, 5)
        ring.forEach(([dx, dy], j) => l(x0 + 5 + dx, 5 + dy, i * 16 + Math.floor((j * 16) / ring.length)))
      }
      return { w: f * 12 - 1, h: 11, cells, slots: f * 16 }
    }
    case 'ring': {
      const r = Math.max(3, Math.min(8, Math.round(count / 5.5)))
      const ring = ringCells(r)
      const size = 2 * r + 3
      o(r + 1, r + 1)
      ring.forEach(([dx, dy], j) => l(r + 1 + dx, r + 1 + dy, j))
      return { w: size, h: size, cells, slots: ring.length }
    }
  }
}

export interface Placed {
  x0: number
  y0: number
  w: number
  h: number
}

export interface Layout {
  placed: Placed[]
  W: number
  H: number
  /** "x,y" -> which device and template cell sits there. */
  map: Map<string, { dev: number; cell: Cell }>
  /** Desk cell of layout cell 0,0 (a desk layout may start anywhere). */
  ox: number
  oy: number
}

function cellMap(templates: Template[], placed: Placed[]) {
  const map = new Map<string, { dev: number; cell: Cell }>()
  templates.forEach((t, dev) => {
    const { x0, y0 } = placed[dev]
    for (const cell of t.cells) map.set(`${cell.x + x0},${cell.y + y0}`, { dev, cell })
  })
  return map
}

const GAP = 4

/** Devices flowed into `rows` centred rows, 4 cells apart. */
export function layout(templates: Template[], rows: number): Layout {
  const total = templates.reduce((a, t) => a + t.w + GAP, 0) - GAP
  const goal = total / rows
  const groups: number[][] = [[]]
  let acc = 0
  templates.forEach((t, i) => {
    if (acc > 0 && acc + t.w / 2 > goal && groups.length < rows) {
      groups.push([])
      acc = 0
    }
    groups[groups.length - 1].push(i)
    acc += t.w + GAP
  })
  const widths = groups.map((g) => g.reduce((a, i) => a + templates[i].w + GAP, 0) - GAP)
  const W = Math.max(0, ...widths)
  const placed: Placed[] = []
  let y = 0
  groups.forEach((g, gi) => {
    const h = Math.max(0, ...g.map((i) => templates[i].h))
    let x = Math.round((W - widths[gi]) / 2)
    for (const i of g) {
      const t = templates[i]
      placed[i] = { x0: x, y0: y + Math.round((h - t.h) / 2), w: t.w, h: t.h }
      x += t.w + GAP
    }
    y += h + GAP
  })
  return { placed, W, H: Math.max(0, y - GAP), map: cellMap(templates, placed), ox: 0, oy: 0 }
}

/** Dot pitch that fits `l` into a `w` x `h` pixel area with a margin of cells. */
export const fitPitch = (l: Layout, w: number, h: number) => Math.min(w / (l.W + 3), h / (l.H + 3))

/** One, two or three rows: whichever gives the biggest dots in a `w` x `h` stage. */
export function bestLayout(templates: Template[], w: number, h: number): Layout {
  let best = layout(templates, 1)
  let bp = fitPitch(best, w, h)
  for (let rows = 2; rows <= Math.min(3, templates.length); rows++) {
    const l = layout(templates, rows)
    const p = fitPitch(l, w, h)
    if (p > bp + 0.01) {
      best = l
      bp = p
    }
  }
  return best
}

// The desk map in dot cells. A device sits at a whole cell (its turned box's top-left) in one of
// four quarter turns, so the matrix stays exact. The engine gets the same thing as a `Placement`
// (centre, size, rotation) plus each LED's spot from the drawing.

export type Turn = 0 | 90 | 180 | 270
export interface Spot {
  x0: number
  y0: number
  rot: Turn
}

/** `t` turned clockwise by `rot` about its box, with the box's top-left kept at 0,0. */
export function turned(t: Template, rot: Turn): Template {
  if (rot === 0) return t
  const at = (x: number, y: number): [number, number] =>
    rot === 90 ? [t.h - 1 - y, x] : rot === 180 ? [t.w - 1 - x, t.h - 1 - y] : [y, t.w - 1 - x]
  const cells = t.cells.map((c) => {
    const [x, y] = at(c.x, c.y)
    return { ...c, x, y }
  })
  return rot === 180 ? { ...t, cells } : { w: t.h, h: t.w, cells, slots: t.slots }
}

/** Each LED's spot in the (unturned) template's unit square: where the stage draws it. With
 *  more LEDs than lights, an LED takes the light nearest its place in order. */
export function ledSpots(t: Template, leds: number): [number, number][] {
  const sum = Array.from({ length: t.slots }, () => [0, 0, 0])
  for (const c of t.cells) {
    if (c.t !== 'l' || c.k < 0 || c.k >= t.slots) continue
    sum[c.k][0] += c.x
    sum[c.k][1] += c.y
    sum[c.k][2]++
  }
  const sx = Math.max(t.w - 1, 1)
  const sy = Math.max(t.h - 1, 1)
  const slot = (k: number): [number, number] => {
    const [x, y, n] = sum[k] ?? [0, 0, 0]
    return n ? [x / n / sx, y / n / sy] : [0.5, 0.5]
  }
  const out: [number, number][] = []
  for (let i = 0; i < leds; i++) {
    const ks: number[] = []
    for (let k = 0; k < t.slots; k++) if (slotLed(k, t.slots, leds) === i) ks.push(k)
    if (ks.length === 0) ks.push(Math.min(t.slots - 1, Math.floor((i * t.slots) / Math.max(leds, 1))))
    const pts = ks.map(slot)
    out.push([pts.reduce((a, p) => a + p[0], 0) / pts.length, pts.reduce((a, p) => a + p[1], 0) / pts.length])
  }
  return out
}

/** The engine's view of a spot: centre, unturned size (cell span) and rotation. */
export function placementOf(t: Template, s: Spot) {
  const tt = turned(t, s.rot)
  return { x: s.x0 + (tt.w - 1) / 2, y: s.y0 + (tt.h - 1) / 2, w: Math.max(t.w - 1, 1), h: Math.max(t.h - 1, 1), rot: s.rot }
}

/** A stored placement back on whole cells (snapped to a quarter turn). */
export function spotOf(t: Template, at: { x: number; y: number; rot: number }): Spot {
  const r = ((((Math.round(at.rot / 90) * 90) % 360) + 360) % 360) as Turn
  const tt = turned(t, r)
  return { x0: Math.round(at.x - (tt.w - 1) / 2), y0: Math.round(at.y - (tt.h - 1) / 2), rot: r }
}

/** Devices drawn where they sit on the desk; those without a spot follow in a row to the right. */
export function deskLayout(templates: Template[], spots: (Spot | null)[]): Layout {
  const shapes = templates.map((t, i) => turned(t, spots[i]?.rot ?? 0))
  const desk: (Placed | null)[] = shapes.map((t, i) => {
    const s = spots[i]
    return s ? { x0: s.x0, y0: s.y0, w: t.w, h: t.h } : null
  })
  const boxes = desk.filter((d): d is Placed => !!d)
  let x = boxes.length ? Math.max(...boxes.map((b) => b.x0 + b.w)) + GAP : 0
  const top = boxes.length ? Math.min(...boxes.map((b) => b.y0)) : 0
  const all = desk.map((d, i) => {
    if (d) return d
    const b = { x0: x, y0: top, w: shapes[i].w, h: shapes[i].h }
    x += b.w + GAP
    return b
  })
  const ox = all.length ? Math.min(...all.map((b) => b.x0)) : 0
  const oy = all.length ? Math.min(...all.map((b) => b.y0)) : 0
  const placed = all.map((b) => ({ ...b, x0: b.x0 - ox, y0: b.y0 - oy }))
  const W = Math.max(0, ...placed.map((b) => b.x0 + b.w))
  const H = Math.max(0, ...placed.map((b) => b.y0 + b.h))
  return { placed, W, H, map: cellMap(shapes, placed), ox, oy }
}

/** A box on the desk, in cells. */
export interface Box {
  x0: number
  y0: number
  w: number
  h: number
}

/** True when `a` and `b` overlap or come closer than `gap` empty cells. */
export const crowds = (a: Box, b: Box, gap = 1) =>
  a.x0 < b.x0 + b.w + gap && b.x0 < a.x0 + a.w + gap && a.y0 < b.y0 + b.h + gap && b.y0 < a.y0 + a.h + gap

/** The free spot nearest to `want` for a `w` x `h` box among `others`, keeping `gap` empty cells
 *  between devices (searched ring by ring outwards; `want` itself when it is free). */
export function freeSpot(others: Box[], w: number, h: number, want: { x0: number; y0: number }, gap = 1): { x0: number; y0: number } {
  const free = (x0: number, y0: number) => !others.some((o) => crowds({ x0, y0, w, h }, o, gap))
  if (free(want.x0, want.y0)) return want
  let best: { x0: number; y0: number } | null = null
  let bd = Infinity
  for (let r = 1; r <= 400 && r < bd; r++) {
    for (let dy = -r; dy <= r; dy++)
      for (let dx = -r; dx <= r; dx++) {
        if (Math.max(Math.abs(dx), Math.abs(dy)) !== r) continue
        const d = Math.hypot(dx, dy)
        if (d < bd && free(want.x0 + dx, want.y0 + dy)) {
          bd = d
          best = { x0: want.x0 + dx, y0: want.y0 + dy }
        }
      }
  }
  return best ?? want
}
