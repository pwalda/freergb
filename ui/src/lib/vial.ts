// A look drawn as a glass vial of light. The vial nearest the pointer leans a little the way the
// pointer moves; the liquid keeps level in the room, lags, sloshes and can splash over the rim.
// The playing look brims over: a dome, two runs down the glass, drops that land and ripple.

import type { Lit } from './preview.ts'

type RGB = readonly [number, number, number] | Lit

/** Pointer speed along x in px/ms, smoothed; decays every frame (see `tickPointer`). */
export const pointer = { x: -1e4, y: -1e4, vx: 0, t: 0 }
let listening = false
export function trackPointer() {
  if (listening || typeof document === 'undefined') return
  listening = true
  document.addEventListener('pointermove', (e) => {
    const now = performance.now()
    const dt = Math.max(8, now - pointer.t)
    if (pointer.t) pointer.vx = pointer.vx * 0.5 + ((e.clientX - pointer.x) / dt) * 0.5
    pointer.x = e.clientX
    pointer.y = e.clientY
    pointer.t = now
  })
}
export const tickPointer = (dt: number) => (pointer.vx *= Math.exp(-dt * 7))

interface Drop {
  x: number
  y: number
  vx: number
  vy: number
  r: number
  c: RGB
}
interface Ripple {
  x: number
  rx: number
  a: number
  c: RGB
}
interface Drip {
  g: number
  falling: boolean
  x: number
  y: number
  v: number
}

export interface VialState {
  /** Glass angle and angular speed. */
  th: number
  tv: number
  /** Liquid surface angle relative to the glass, and its speed. */
  s: number
  sv: number
  drops: Drop[]
  ripples: Ripple[]
  drips: [Drip, Drip]
}

export const newVial = (): VialState => ({
  th: 0,
  tv: 0,
  s: 0,
  sv: 0,
  drops: [],
  ripples: [],
  drips: [
    { g: 0, falling: false, x: 0, y: 0, v: 0 },
    { g: 0.5, falling: false, x: 0, y: 0, v: 0 },
  ],
})

const rgba = (c: RGB, a: number) => `rgba(${c[0] | 0},${c[1] | 0},${c[2] | 0},${a})`
const mix = (a: RGB, b: RGB, k: number): RGB => [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k, a[2] + (b[2] - a[2]) * k]
const WHITE: RGB = [255, 255, 255]

export interface VialFrame {
  /** Seconds since start, 0 under reduced motion. */
  t: number
  /** Seconds since the last frame. */
  dt: number
  /** Fill level: 0.6 resting, 0.8 hovered, above 1 brims over. */
  level: number
  /** This is the vial nearest the pointer: only it leans. */
  near: boolean
  dark: boolean
  calm: boolean
  label: string
  labelColor: string
  /** The look's colour at position u (0..1) along the liquid. */
  light: (u: number, t: number) => Lit
}

/** Draws one vial into a canvas `W` x `H` CSS px wide. The canvas is wider than the vial's cell
 *  (see Vials.svelte) so splashes and the table glow never meet an edge. */
export function drawVial(g: CanvasRenderingContext2D, W: number, H: number, V: VialState, f: VialFrame) {
  const { t, dt, level: L, dark, calm } = f
  g.clearRect(0, 0, W, H)
  const over = Math.min(1, Math.max(0, (L - 1) / 0.1))
  const cw = W / 1.6
  const tw = Math.min(cw * 0.3, H * 0.28, 44)
  const cx = W / 2
  const x0 = cx - tw / 2
  const top = H * 0.18
  const bot = H - 24
  const R2 = tw / 2
  const py = H - 12

  if (!calm && dt > 0) {
    const target = f.near ? Math.max(-0.07, Math.min(0.07, pointer.vx * 0.05)) : 0
    V.tv += (110 * (target - V.th) - 17 * V.tv) * dt
    V.th += V.tv * dt
    // The liquid is far looser than the glass: it lags, swings past level and settles slowly.
    V.sv += (34 * (-V.th * 5 - V.s) - 1.1 * V.sv) * dt
    V.s = Math.max(-0.6, Math.min(0.6, V.s + V.sv * dt))
  }
  const th = V.th
  const cs = Math.cos(th)
  const sn = Math.sin(th)
  const E = Math.min(1, Math.abs(V.sv) * 0.2)
  const toWorld = (x: number, y: number): [number, number] => [cx + (x - cx) * cs - (y - bot) * sn, bot + (x - cx) * sn + (y - bot) * cs]
  const c0 = f.light(0, t)
  const c1 = f.light(1, t)
  const cm = f.light(0.5, t)

  // On the table: a shadow, a coloured reflection, and the pool when brimming.
  g.fillStyle = dark ? 'rgba(0,0,0,.55)' : 'rgba(70,52,25,.16)'
  g.beginPath()
  g.ellipse(cx + th * 14, py, tw * 0.75, 2.3, 0, 0, 7)
  g.fill()
  if (dark) g.globalCompositeOperation = 'lighter'
  {
    const r = Math.min(cw * 0.7, tw * (1.2 + 0.6 * over))
    g.save()
    g.translate(cx, py)
    g.scale(1, 0.2)
    const gr = g.createRadialGradient(0, 0, 0, 0, 0, r)
    gr.addColorStop(0, rgba(cm, (dark ? 0.16 : 0.12) + 0.24 * over))
    gr.addColorStop(1, rgba(cm, 0))
    g.fillStyle = gr
    g.fillRect(-r, -r, 2 * r, 2 * r)
    g.restore()
  }
  g.globalCompositeOperation = 'source-over'
  if (over > 0) {
    const pg = g.createRadialGradient(cx, py, 0, cx, py, tw * 1.05 * over)
    pg.addColorStop(0, rgba(mix(c0, WHITE, 0.25), 0.95))
    pg.addColorStop(1, rgba(c1, 0.8))
    g.fillStyle = pg
    g.beginPath()
    g.ellipse(cx, py, Math.min(cw * 0.6, tw * 1.05 * over), 2.4 + over, 0, 0, 7)
    g.fill()
    g.strokeStyle = 'rgba(255,255,255,.45)'
    g.lineWidth = 1
    g.beginPath()
    g.ellipse(cx - tw * 0.2, py - 0.6, tw * 0.35 * over, 0.9, 0, Math.PI, 0)
    g.stroke()
  }
  V.ripples = V.ripples.filter((r) => (r.a -= dt * 1.7) > 0)
  for (const r of V.ripples) {
    r.rx += dt * 28
    g.strokeStyle = rgba(r.c, r.a * 0.8)
    g.lineWidth = 1
    g.beginPath()
    g.ellipse(r.x, py, r.rx, r.rx * 0.22, 0, 0, 7)
    g.stroke()
  }

  g.save()
  g.translate(cx, bot)
  g.rotate(th)
  g.translate(-cx, -bot)
  const tube = (i = 0) => {
    g.beginPath()
    g.moveTo(x0 + i, top)
    g.lineTo(x0 + i, bot - R2)
    g.arc(cx, bot - R2, R2 - i, Math.PI, 0, true)
    g.lineTo(x0 + tw - i, top)
  }
  // The surface stays level in the room, so inside the glass it tilts against the glass; fast
  // slosh climbs the walls.
  const lv = Math.min(L, 1)
  const lt = bot - (bot - top - 2) * lv
  const tilt = Math.tan(V.s)
  const climb = Math.abs(V.sv) * 5
  const surf = (x: number) => {
    const dw = Math.min(x - x0, x0 + tw - x)
    return lt + tilt * (x - cx) + (Math.sin(x * 0.28 + t * 2.2) * (0.3 + E * 1.4) + Math.sin(x * 0.55 - t * 3.1) * E * 0.9) - (1.5 + climb) * Math.exp(-dw / 2.6)
  }
  const lg = g.createLinearGradient(0, lt - 6, 0, bot)
  for (let k = 0; k <= 8; k++) {
    const col = f.light(k / 8, t)
    const a = (dark ? 0.6 : 0.72) + 0.38 * col[3] * (L > 0.9 ? 1 : 0.65)
    lg.addColorStop(k / 8, rgba(mix(col, [0, 0, 0], (k / 8) * 0.2), Math.min(1, a)))
  }
  g.save()
  tube()
  g.closePath()
  g.clip()
  const edge: [number, number][] = []
  for (let x = x0 - 1; x <= x0 + tw + 1; x += 1.5) edge.push([x, Math.max(top - 7, surf(x))])
  g.beginPath()
  g.moveTo(x0 - 2, bot + 2)
  for (const [x, y] of edge) g.lineTo(x, y)
  g.lineTo(x0 + tw + 2, bot + 2)
  g.closePath()
  g.fillStyle = lg
  g.fill()
  const sg = g.createLinearGradient(x0, 0, x0 + tw, 0)
  sg.addColorStop(0, 'rgba(0,0,0,.3)')
  sg.addColorStop(0.28, 'rgba(0,0,0,0)')
  sg.addColorStop(0.6, 'rgba(255,255,255,.1)')
  sg.addColorStop(1, 'rgba(0,0,0,.34)')
  g.fillStyle = sg
  g.fill()
  const ig = g.createRadialGradient(cx, bot - R2 * 0.7, 0, cx, bot - R2 * 0.7, tw * 0.9)
  ig.addColorStop(0, 'rgba(255,255,255,.22)')
  ig.addColorStop(1, 'rgba(255,255,255,0)')
  g.fillStyle = ig
  g.fill()
  g.strokeStyle = rgba(mix(c0, WHITE, 0.5), 0.9)
  g.lineWidth = 1.3
  g.beginPath()
  edge.forEach(([x, y], n) => (n ? g.lineTo(x, y) : g.moveTo(x, y)))
  g.stroke()
  g.restore()

  // Glass: thin body tint, double wall, a long specular band, a rim light, a bright lip and a
  // glint on the curved bottom.
  const bg = g.createLinearGradient(x0, 0, x0 + tw, 0)
  bg.addColorStop(0, dark ? 'rgba(255,255,255,.16)' : 'rgba(255,255,255,.7)')
  bg.addColorStop(0.3, dark ? 'rgba(255,255,255,.03)' : 'rgba(255,255,255,.2)')
  bg.addColorStop(0.75, 'rgba(255,255,255,0)')
  bg.addColorStop(1, dark ? 'rgba(255,255,255,.12)' : 'rgba(255,255,255,.55)')
  g.fillStyle = bg
  tube()
  g.closePath()
  g.fill()
  g.strokeStyle = dark ? 'rgba(255,255,255,.5)' : 'rgba(40,32,20,.42)'
  g.lineWidth = 1.3
  tube()
  g.stroke()
  g.strokeStyle = dark ? 'rgba(255,255,255,.16)' : 'rgba(40,32,20,.14)'
  g.lineWidth = 1
  tube(1.8)
  g.stroke()
  const hg = g.createLinearGradient(0, top + 5, 0, bot - R2)
  hg.addColorStop(0, 'rgba(255,255,255,.85)')
  hg.addColorStop(0.55, 'rgba(255,255,255,.35)')
  hg.addColorStop(1, 'rgba(255,255,255,.08)')
  g.fillStyle = hg
  g.beginPath()
  g.roundRect(x0 + tw * 0.14, top + 5, Math.max(2.2, tw * 0.09), bot - R2 - top - 5, 2)
  g.fill()
  g.fillStyle = 'rgba(255,255,255,.6)'
  g.beginPath()
  g.roundRect(x0 + tw * 0.27, top + 8, 1.2, (bot - R2 - top) * 0.35, 1)
  g.fill()
  g.strokeStyle = 'rgba(255,255,255,.45)'
  g.lineWidth = 1.1
  g.beginPath()
  g.moveTo(x0 + tw * 0.86, top + 12)
  g.lineTo(x0 + tw * 0.86, bot - R2 - 3)
  g.stroke()
  g.strokeStyle = 'rgba(255,255,255,.55)'
  g.lineWidth = 1.4
  g.lineCap = 'round'
  g.beginPath()
  g.arc(cx, bot - R2, R2 - 3.2, Math.PI * 0.62, Math.PI * 0.86)
  g.stroke()
  const lp = g.createLinearGradient(0, top - 4, 0, top + 1)
  lp.addColorStop(0, 'rgba(255,255,255,.75)')
  lp.addColorStop(1, dark ? 'rgba(255,255,255,.12)' : 'rgba(255,255,255,.4)')
  g.fillStyle = lp
  g.beginPath()
  g.roundRect(x0 - 3.5, top - 3.5, tw + 7, 4.5, 2.2)
  g.fill()
  g.strokeStyle = dark ? 'rgba(255,255,255,.55)' : 'rgba(40,32,20,.42)'
  g.lineWidth = 1
  g.stroke()
  g.fillStyle = f.labelColor
  g.font = '10px "Geist Mono Variable", ui-monospace, monospace'
  g.textAlign = 'left'
  g.fillText(f.label, x0 + tw + 7, top + 10)

  // Brimming over: a dome held by surface tension, two runs down the glass, drops that swell,
  // let go and land.
  const falling: [Drip, RGB][] = []
  if (over > 0) {
    g.fillStyle = rgba(c0, 0.95)
    g.beginPath()
    g.moveTo(x0 - 1, top)
    for (let x = x0 - 1; x <= x0 + tw + 1; x += 1) {
      const k = (x - x0) / tw
      g.lineTo(x, top - Math.sin(Math.PI * Math.min(1, Math.max(0, k))) * (4.5 + Math.sin(t * 3) * 0.5 - tilt * (x - cx) * 0.4) * over)
    }
    g.closePath()
    g.fill()
    g.strokeStyle = 'rgba(255,255,255,.6)'
    g.lineWidth = 1
    g.beginPath()
    g.arc(cx - tw * 0.15, top - 1, tw * 0.22, Math.PI * 1.15, Math.PI * 1.5)
    g.stroke()
    const runs: [number, number, RGB, number][] = [
      [x0 - 0.6, -1, c0, 0.9],
      [x0 + tw + 0.6, 1, c1, 0.66],
    ]
    runs.forEach(([sx, dir, col, frac], n) => {
      const len = (bot - top - 6) * over * frac * (1 + dir * th * 1.4)
      const pts: [number, number, number][] = []
      for (let k = 0; k <= 12; k++) {
        const q = k / 12
        pts.push([sx + dir * (Math.sin(q * Math.PI) * 1.5 + q * 1.1), top + q * len, (1 - q) * 1.8 + 0.9])
      }
      g.fillStyle = rgba(col, 0.95)
      g.beginPath()
      pts.forEach(([x, y, w], k) => (k ? g.lineTo(x + dir * w, y) : g.moveTo(x + dir * w, y)))
      for (let k = pts.length - 1; k >= 0; k--) {
        const [x, y, w] = pts[k]
        g.lineTo(x - dir * w * 0.4, y)
      }
      g.closePath()
      g.fill()
      g.strokeStyle = 'rgba(255,255,255,.35)'
      g.lineWidth = 0.8
      g.beginPath()
      g.moveTo(pts[1][0], pts[1][1])
      g.lineTo(pts[6][0], pts[6][1])
      g.stroke()
      const D = V.drips[n]
      const [ex, ey] = pts[12]
      if (!D.falling) {
        D.g += dt * 0.9
        const r = Math.min(1, D.g) * 2.6
        g.beginPath()
        g.ellipse(ex, ey + r * 0.7, r * 0.8, r, 0, 0, 7)
        g.fill()
        if (D.g >= 1) {
          ;[D.x, D.y] = toWorld(ex, ey + 2)
          D.falling = true
          D.v = 0
        }
      } else falling.push([D, col])
    })
  }
  g.restore()
  for (const [D, col] of falling) {
    D.v += dt * 380
    D.y += D.v * dt
    g.fillStyle = rgba(col, 0.95)
    g.beginPath()
    g.ellipse(D.x, D.y, 1.8, 2.9, 0, 0, 7)
    g.fill()
    if (D.y >= py - 1) {
      V.ripples.push({ x: D.x, rx: 2, a: 1, c: col })
      D.falling = false
      D.g = 0
    }
  }
  // Liquid that reaches a rim goes over it.
  if (!calm) {
    const rims: [number, number, RGB][] = [
      [x0 + 0.5, -1, c0],
      [x0 + tw - 0.5, 1, c1],
    ]
    for (const [wx, side, col] of rims) {
      const o = top + 1.5 - surf(wx)
      if (o > 0 && Math.random() < dt * Math.min(70, o * 5 + Math.abs(V.sv) * 6)) {
        const [x, y] = toWorld(wx + side * 1.5, top - 1)
        V.drops.push({ x, y, vx: side * (14 + Math.random() * 30) + V.tv * 15, vy: -(15 + Math.random() * 50 * (0.3 + E)), r: 0.9 + Math.random() * 1.3, c: col })
      }
    }
  }
  V.drops = V.drops.filter((q) => {
    q.vy += 520 * dt
    q.x += q.vx * dt
    q.y += q.vy * dt
    if (q.y >= py) {
      V.ripples.push({ x: q.x, rx: 1.5, a: 0.8, c: q.c })
      return false
    }
    g.fillStyle = rgba(q.c, 0.95)
    g.beginPath()
    g.ellipse(q.x, q.y, q.r, q.r * (1 + Math.min(1.2, Math.abs(q.vy) / 300)), Math.atan2(q.vy, q.vx) - Math.PI / 2, 0, 7)
    g.fill()
    return q.x > 2 && q.x < W - 2
  })
}

/** Sizes a canvas's backing store to its CSS box at the device pixel ratio. */
export function fit(c: HTMLCanvasElement): [CanvasRenderingContext2D, number, number] | null {
  const g = c.getContext('2d')
  if (!g) return null
  const dpr = window.devicePixelRatio || 1
  const w = c.clientWidth
  const h = c.clientHeight
  const bw = Math.round(w * dpr)
  const bh = Math.round(h * dpr)
  if (c.width !== bw || c.height !== bh) {
    c.width = bw
    c.height = bh
  }
  g.setTransform(dpr, 0, 0, dpr, 0, 0)
  return [g, w, h]
}
