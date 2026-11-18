<script lang="ts">
  import { onMount, type Snippet } from 'svelte'
  import type { Device } from './api'
  import { aim } from './aim.svelte'
  import { onFrame, reducedMotion } from './frames'
  import { ledLit, lookName, type Look, type Lit } from './preview'
  import { bestLayout, deskLayout, slotLed, type Layout, type Spot, type Template } from './rig'
  import { Tweens } from './tween'
  import { fit } from './vial'

  let {
    devices,
    parts,
    names,
    looks,
    templates,
    spots,
    selected,
    focus = null,
    onselect,
    notice,
    arrange = null,
  }: {
    devices: Device[]
    /** What the stage draws: each device, or each of its pieces when it is split into
     *  segments (`dev` indexes `devices`, `leds` its LEDs). `names`, `templates` and `spots`
     *  follow this list; `looks`, `selected` and `onselect` go by device. */
    parts: { dev: number; leds: number[] }[]
    names: string[]
    looks: Look[]
    /** How each piece is drawn (already sized; turned by its spot). */
    templates: Template[]
    /** Where each piece sits on the desk map, or null; with no spot at all the rig is packed. */
    spots: (Spot | null)[]
    /** Index of the device in view, -1 for all devices. */
    selected: number
    /** In a group's view (selected -1, `devices` only its members): the caption names it. */
    focus?: { name: string } | null
    onselect: (i: number) => void
    /** Shown instead of the rig while there is nothing to light (not connected, no devices). */
    notice?: Snippet
    /** Arranging the desk: pieces are dragged (snapping to cells), `sel` is the one picked.
     *  Spots arrive back through `onmove`; a piece without one is drawn right of the rest. */
    arrange?: {
      sel: number
      onpick: (i: number) => void
      onmove: (i: number, s: Spot) => void
      onturn: (i: number) => void
    } | null
  } = $props()

  /** Every piece's spot as drawn now (placed or not), to seed or finish an arrangement. */
  export function drawnSpots(): Spot[] {
    if (!lay) return []
    return lay.placed.map((g, i) => ({ x0: g.x0 + lay!.ox, y0: g.y0 + lay!.oy, rot: spots[i]?.rot ?? 0 }))
  }

  let canvas = $state<HTMLCanvasElement>()
  const tw = new Tweens()
  const live = $derived(!notice && devices.length > 0)
  const desk = $derived(spots.some((s) => s))
  const rigKey = $derived(
    templates.map((t, i) => `${t.w}x${t.h}:${t.slots}@${spots[i] ? `${spots[i].x0},${spots[i].y0},${spots[i].rot}` : '-'}`).join('|'),
  )
  /** A drag in progress: the device, the cell it was grabbed at and its spot then. */
  let drag = $state.raw<{ i: number; from: [number, number]; spot: Spot } | null>(null)

  let lay: Layout | null = null
  let layKey = ''
  let SW = 0
  let SH = 0
  let hover = $state(-1)
  /** The pointer over the matrix, in canvas pixels; the dots around it glow. */
  let ptr: { x: number; y: number } | null = null
  let glowAt = { x: 0, y: 0 }

  // Pixels kept free below the whole rig for the names under its bottom row, and at the sides,
  // where the matrix fades into the page.
  const NAMES = 26
  const SIDES = 40

  // Camera: dot pitch p and the desk cell at the centre (a desk layout may start anywhere, so
  // the camera works in desk cells and a device dragged past the edge does not shift the view).
  // Zooming into a device grows the dots. Arranging leaves room around the desk to drag into.
  function camTarget() {
    if (!lay || !SW) return { p: 8, cx: 0, cy: 0 }
    const mine = lay.placed.filter((_, i) => parts[i]?.dev === selected)
    const g = mine.length
      ? (() => {
          const x0 = Math.min(...mine.map((b) => b.x0))
          const y0 = Math.min(...mine.map((b) => b.y0))
          return { x0, y0, w: Math.max(...mine.map((b) => b.x0 + b.w)) - x0, h: Math.max(...mine.map((b) => b.y0 + b.h)) - y0 }
        })()
      : null
    if (!g) {
      const m = arrange ? 12 : 3
      const p = Math.min((SW - SIDES) / (lay.W + m), (SH - NAMES) / (lay.H + m), 12)
      return { p, cx: lay.ox + (lay.W - 1) / 2, cy: lay.oy + (lay.H - 1) / 2 + NAMES / 2 / p }
    }
    return { p: Math.min(SW / (g.w + 6), SH / (g.h + 4), 28), cx: lay.ox + g.x0 + (g.w - 1) / 2, cy: lay.oy + g.y0 + (g.h - 1) / 2 }
  }
  function camTo(ms: number) {
    const c = camTarget()
    tw.to('p', c.p, ms)
    tw.to('cx', c.cx, ms)
    tw.to('cy', c.cy, ms)
  }
  function camSnap() {
    const c = camTarget()
    tw.set('p', c.p)
    tw.set('cx', c.cx)
    tw.set('cy', c.cy)
  }
  tw.set('master', 0)
  tw.set('lab', 1)
  tw.set('glow', 0)

  // Selecting a device flies the camera to it and fades the others out.
  const alphaOf = (sel: number, i: number) => (sel < 0 || sel === i ? 1 : 0)
  let shown: number | null = null
  $effect(() => {
    const sel = selected
    const n = devices.length
    if (shown === null) {
      for (let i = 0; i < n; i++) tw.set(`al${i}`, alphaOf(sel, i))
      tw.set('lab', sel < 0 ? 1 : 0)
    } else if (sel !== shown) {
      camTo(760)
      for (let i = 0; i < n; i++) tw.to(`al${i}`, alphaOf(sel, i), sel < 0 ? 700 : 420)
      tw.to('lab', sel < 0 ? 1 : 0, 400)
    } else {
      for (let i = 0; i < n; i++) if (!tw.has(`al${i}`)) tw.set(`al${i}`, alphaOf(sel, i))
    }
    shown = sel
  })
  // Arranging pulls the camera back to leave room around the desk to drag into.
  let arranging: boolean | null = null
  $effect(() => {
    const on = !!arrange
    if (arranging !== null && on !== arranging && lay) camTo(600)
    arranging = on
  })
  // The lights warm up when there is something to show.
  $effect(() => tw.to('master', live ? 1 : 0, 1200))

  // The signature: a new look wipes across the matrix left to right over the old one.
  let prev: Look[] = []
  let wipe: { t0: number; from: Look[] } | null = null
  $effect(() => {
    const next = looks
    if (!reducedMotion() && prev.length === next.length && next.some((l, i) => lookName(l) !== lookName(prev[i]))) wipe = { t0: performance.now(), from: prev }
    prev = next
  })

  let pal = { off: [34, 32, 29], line: [84, 80, 73], glow: [239, 230, 207], lamp: [239, 230, 207], ground: '#121110', text: '#e9e5dc', muted: '#958f83', dark: true }
  const rgbOf = (hex: string, fb: number[]) => {
    const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim())
    return m ? [0, 2, 4].map((i) => parseInt(m[1].slice(i, i + 2), 16)) : fb
  }
  // Read on the first frame of each theme, not from an effect: an effect can run before the
  // theme attribute flips and would keep the old theme's dots.
  let palFor: string | undefined
  function palette() {
    const theme = document.documentElement.dataset.theme
    if (theme === palFor) return
    palFor = theme
    const cs = getComputedStyle(document.documentElement)
    const v = (n: string) => cs.getPropertyValue(n).trim()
    pal = { off: rgbOf(v('--dot-off'), pal.off), line: rgbOf(v('--dot-line'), pal.line), glow: rgbOf(v('--dot-glow'), pal.glow), lamp: rgbOf(v('--lamp'), pal.lamp), ground: v('--ground'), text: v('--text'), muted: v('--muted'), dark: theme !== 'light' }
    bg = ''
  }

  const ease = (x: number) => 1 - Math.pow(1 - x, 4)
  const mix = (a: number[], b: number[], k: number) => [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k, a[2] + (b[2] - a[2]) * k]

  function lit(dev: number, led: number, sx: number, t: number): Lit {
    const c = ledLit(looks[dev], led, t)
    if (!wipe) return c
    const p = Math.min(1, (performance.now() - wipe.t0) / 650)
    if (p >= 1) {
      wipe = null
      return c
    }
    const from = wipe.from[dev]
    if (!from) return c
    const front = ease(p) * 1.3 - 0.15
    const k = Math.min(1, Math.max(0, (front - sx) / 0.1))
    const o = ledLit(from, led, t)
    const m = mix(o, c, k)
    return [m[0], m[1], m[2], o[3] + (c[3] - o[3]) * k]
  }

  /** Whether piece `i` is lit up as hovered: the piece itself while arranging (pieces move
   *  alone), otherwise every piece of the hovered device (a click opens the device). */
  const hovered = (i: number) => hover >= 0 && (arrange ? hover === i : parts[hover]?.dev === parts[i]?.dev)

  /** The desk cell under a canvas point. */
  function cellAt(x: number, y: number): [number, number] {
    const p = tw.get('p', 8)
    return [Math.round(tw.get('cx') + (x - SW / 2) / p), Math.round(tw.get('cy') + (y - SH / 2) / p)]
  }
  function devAt(x: number, y: number) {
    if (!lay || !live) return -1
    const L = lay
    const [i, j] = cellAt(x, y)
    const hit = (g: (typeof L.placed)[number]) => i - L.ox >= g.x0 - 1 && i - L.ox <= g.x0 + g.w && j - L.oy >= g.y0 - 1 && j - L.oy <= g.y0 + g.h
    // The picked device first, so it can be grabbed where it overlaps another.
    if (arrange && L.placed[arrange.sel] && hit(L.placed[arrange.sel])) return arrange.sel
    for (let d = L.placed.length - 1; d >= 0; d--) if (hit(L.placed[d])) return d
    return -1
  }

  // Unlit dots everywhere, cached: they only change when the camera, size or theme does.
  let bgCanvas: HTMLCanvasElement | null = null
  let bg = ''
  function background(W: number, H: number, p: number, cx: number, cy: number, r0: number) {
    const dpr = window.devicePixelRatio || 1
    const key = `${W}x${H}@${dpr}:${p.toFixed(3)}:${cx.toFixed(3)}:${cy.toFixed(3)}`
    bgCanvas ??= document.createElement('canvas')
    if (key === bg) return bgCanvas
    bg = key
    bgCanvas.width = Math.round(W * dpr)
    bgCanvas.height = Math.round(H * dpr)
    const g = bgCanvas.getContext('2d')
    if (!g) return bgCanvas
    g.setTransform(dpr, 0, 0, dpr, 0, 0)
    g.fillStyle = `rgb(${pal.off[0]},${pal.off[1]},${pal.off[2]})`
    const i0 = Math.floor(cx - W / 2 / p) - 1
    const i1 = Math.ceil(cx + W / 2 / p) + 1
    const j0 = Math.floor(cy - H / 2 / p) - 1
    const j1 = Math.ceil(cy + H / 2 / p) + 1
    g.beginPath()
    for (let j = j0; j <= j1; j++)
      for (let i = i0; i <= i1; i++) {
        const X = W / 2 + (i - cx) * p
        const Y = H / 2 + (j - cy) * p
        g.moveTo(X + r0, Y)
        g.arc(X, Y, r0, 0, 7)
      }
    g.fill()
    return bgCanvas
  }

  function draw(seconds: number) {
    const box = canvas && fit(canvas)
    if (!box) return
    const [g, W, H] = box
    const t = reducedMotion() ? 0 : seconds
    palette()
    if (W !== SW || H !== SH || rigKey !== layKey) {
      const resized = W !== SW || H !== SH
      const first = !lay
      SW = W
      SH = H
      layKey = rigKey
      lay = desk ? deskLayout(templates, spots) : bestLayout(templates, W - SIDES, H - NAMES)
      if (resized || first) camSnap()
      else if (!drag) camTo(600)
    }
    g.clearRect(0, 0, W, H)
    const p = tw.get('p', 8)
    const cx = tw.get('cx')
    const cy = tw.get('cy')
    const r0 = Math.max(1.6, p * 0.27)
    g.drawImage(background(W, H, p, cx, cy, r0), 0, 0, W, H)

    // The pointer glow: dots near the pointer light up faintly and grow, fading with distance.
    // The centre trails the pointer a little so the glow moves softly.
    const gl = tw.get('glow')
    if (ptr) {
      const k = reducedMotion() || gl < 0.01 ? 1 : 0.3
      glowAt = { x: glowAt.x + (ptr.x - glowAt.x) * k, y: glowAt.y + (ptr.y - glowAt.y) * k }
    }
    const R = Math.max(72, p * 6)
    const glow = (X: number, Y: number) => {
      if (gl < 0.01) return 0
      const d = Math.hypot(X - glowAt.x, Y - glowAt.y)
      return d >= R ? 0 : gl * (1 - (d / R) ** 2) ** 2
    }
    if (gl >= 0.01) {
      const i0 = Math.floor(cx + (glowAt.x - R - W / 2) / p)
      const i1 = Math.ceil(cx + (glowAt.x + R - W / 2) / p)
      const j0 = Math.floor(cy + (glowAt.y - R - H / 2) / p)
      const j1 = Math.ceil(cy + (glowAt.y + R - H / 2) / p)
      for (let j = j0; j <= j1; j++)
        for (let i = i0; i <= i1; i++) {
          if (live && lay?.map.has(`${i - lay.ox},${j - lay.oy}`)) continue // drawn with its device below
          const X = W / 2 + (i - cx) * p
          const Y = H / 2 + (j - cy) * p
          const f = glow(X, Y)
          if (f <= 0) continue
          const col = mix(pal.off, pal.glow, 0.55 * f)
          g.fillStyle = `rgb(${col[0] | 0},${col[1] | 0},${col[2] | 0})`
          g.beginPath()
          g.arc(X, Y, r0 * (1 + 0.45 * f), 0, 7)
          g.fill()
        }
    }
    if (!live || !lay) return
    const master = tw.get('master')
    const dark = pal.dark
    const picked = arrange ? arrange.sel : -1
    for (const [at, m] of lay.map) {
      const [i, j] = at.split(',').map(Number)
      const X = W / 2 + (i + lay.ox - cx) * p
      const Y = H / 2 + (j + lay.oy - cy) * p
      if (X < -p || X > W + p || Y < -p || Y > H + p) continue
      const part = parts[m.dev]
      const al = tw.get(`al${part?.dev ?? m.dev}`, 1)
      let col = pal.off
      let r = r0
      if (m.cell.t === 'o' && m.dev === picked) {
        // The device being arranged: its outline in the lamp, so it reads as "this one".
        col = mix(pal.line, pal.lamp, 0.8)
      } else if (m.cell.t === 'o') col = mix(pal.off, pal.line, Math.min(1, al * (hovered(m.dev) && selected < 0 ? 1.6 : 1)))
      else {
        const tp = templates[m.dev]
        const leds = part?.leds ?? []
        const k = slotLed(m.cell.k, tp.slots, leds.length)
        const c = lit(part?.dev ?? m.dev, k < 0 ? -1 : leds[k], X / W, t)
        const lv = Math.min(1, c[3] * master * al)
        col = mix(pal.off, c, lv)
        r = r0 + p * 0.1 * lv
        if (lv > 0.35) {
          g.fillStyle = `rgba(${c[0] | 0},${c[1] | 0},${c[2] | 0},${(dark ? 0.16 : 0.14) * lv})`
          if (dark) g.globalCompositeOperation = 'lighter'
          g.beginPath()
          g.arc(X, Y, r * 1.75, 0, 7)
          g.fill()
          g.globalCompositeOperation = 'source-over'
        }
      }
      const f = glow(X, Y)
      if (f > 0) {
        col = mix(col, pal.glow, 0.3 * f)
        r *= 1 + 0.45 * f
      }
      // Cover the cached unlit dot, then draw this one.
      g.fillStyle = `rgb(${col[0] | 0},${col[1] | 0},${col[2] | 0})`
      g.beginPath()
      g.arc(X, Y, r, 0, 7)
      g.fill()
    }
    arrow(g, W, H, p, cx, cy)
    // Names under the devices, drawn last so no dot covers them. A name may run wider than its
    // device; where it would meet a neighbour's, both are shortened in the middle (the end often
    // tells two alike devices apart). The hovered device's name is shown whole. Each name sits in
    // a clearing of the matrix: the dots fade out towards it, like a vignette.
    g.font = '11px "Geist Mono Variable", ui-monospace, monospace'
    g.textAlign = 'center'
    const shown: { name: string; X: number; Y: number; a: number; i: number }[] = []
    const lab = tw.get('lab', 1)
    const L = lay
    const labels = L.placed.map((gp, i) => {
      const name = arrange && !spots[i] ? `${names[i] ?? ''} · not placed yet` : (names[i] ?? '')
      return { gp, name, x: W / 2 + (gp.x0 + L.ox + (gp.w - 1) / 2 - cx) * p, half: g.measureText(name).width / 2 }
    })
    labels.forEach((l, i) => {
      const a = lab * tw.get(`al${parts[i]?.dev ?? i}`, 1)
      if (a < 0.01 || !l.name) return
      let room = Math.min(l.x - 4, W - 4 - l.x)
      if (hover !== i)
        for (const o of labels) {
          // Only the nearest devices beside this one, on the same row, compete for room.
          if (o === l || !o.name || o.gp.y0 > l.gp.y0 + l.gp.h || l.gp.y0 > o.gp.y0 + o.gp.h) continue
          const gap = Math.abs(o.x - l.x)
          if (l.half + o.half + 10 > gap) room = Math.min(room, (gap * l.half) / (l.half + o.half) - 5)
        }
      const name = shorten(g, l.name, room * 2)
      if (!name) return
      shown.push({ name, X: l.x, Y: H / 2 + (l.gp.y0 + L.oy + l.gp.h + 0.9 - cy) * p, a, i })
    })
    // Clearings before names, so one never covers a neighbour's name; except the hovered name,
    // shown whole, which comes last and clears whatever it runs over. Only the blurred shadow of a
    // rectangle drawn far off-canvas lands here: soft edges without canvas filters (not in WebKitGTK).
    const FAR = 10000
    // Shadow offset and blur are in device pixels; the context is scaled for HiDPI.
    const dpr = g.getTransform().a
    const clear = (list: typeof shown) => {
      g.shadowBlur = 14 * dpr
      g.shadowOffsetX = FAR * dpr
      g.shadowColor = pal.ground
      g.fillStyle = pal.ground
      for (const { name, X, Y, a } of list) {
        const w = g.measureText(name).width + 16
        g.globalAlpha = a
        g.fillRect(X - w / 2 - FAR, Y - 13, w, 19)
      }
      g.shadowBlur = 0
      g.shadowOffsetX = 0
      g.shadowColor = 'transparent'
      for (const { name, X, Y, a, i } of list) {
        g.globalAlpha = a
        g.fillStyle = hovered(i) || picked === i ? pal.text : pal.muted
        g.fillText(name, X, Y)
      }
    }
    clear(shown.filter((l) => l.i !== hover))
    clear(shown.filter((l) => l.i === hover))
    g.globalAlpha = 1
  }

  /** After the direction dial moves: a short arrow across the middle of the stage pointing the
   *  new way, drawn freely over the matrix (not on its dots, so it reads as a pointer, not as
   *  light), fading in from the tail, with a pulse running to its head; gone in about a second. */
  const AIM_MS = 1100
  function arrow(g: CanvasRenderingContext2D, W: number, H: number, p: number, cx: number, cy: number) {
    const age = performance.now() - aim.at
    if (aim.at === 0 || age >= AIM_MS) return
    const fade = 1 - (age / AIM_MS) ** 2
    const a = (aim.angle * Math.PI) / 180
    const [dx, dy] = [Math.cos(a), Math.sin(a)]
    // Centred on the camera's middle cell, so it sits over the rig.
    const X0 = W / 2 + (Math.round(cx) - cx) * p
    const Y0 = H / 2 + (Math.round(cy) - cy) * p
    const R = Math.min(W, H) * 0.3
    const tail = [X0 - dx * R, Y0 - dy * R]
    const tip = [X0 + dx * R, Y0 + dy * R]
    const [r, gr, b] = pal.glow
    const c = (al: number) => `rgba(${r},${gr},${b},${al * fade})`
    const w = Math.max(2, p * 0.32)
    g.save()
    g.lineCap = 'round'
    g.lineJoin = 'round'
    g.lineWidth = w
    const shaft = g.createLinearGradient(tail[0], tail[1], tip[0], tip[1])
    shaft.addColorStop(0, c(0))
    shaft.addColorStop(1, c(0.9))
    g.strokeStyle = shaft
    g.beginPath()
    g.moveTo(tail[0], tail[1])
    g.lineTo(tip[0], tip[1])
    g.stroke()
    const head = Math.max(10, R * 0.22)
    g.strokeStyle = c(0.9)
    g.beginPath()
    g.moveTo(tip[0] - head * Math.cos(a - 0.55), tip[1] - head * Math.sin(a - 0.55))
    g.lineTo(tip[0], tip[1])
    g.lineTo(tip[0] - head * Math.cos(a + 0.55), tip[1] - head * Math.sin(a + 0.55))
    g.stroke()
    if (!reducedMotion()) {
      // The pulse: a soft bright bead running from tail to head in the first half second.
      const u = Math.min(1, age / 500)
      const px = tail[0] + (tip[0] - tail[0]) * u
      const py = tail[1] + (tip[1] - tail[1]) * u
      const glow = g.createRadialGradient(px, py, 0, px, py, w * 3)
      glow.addColorStop(0, c(0.95 * (1 - u * 0.5)))
      glow.addColorStop(1, c(0))
      g.fillStyle = glow
      g.beginPath()
      g.arc(px, py, w * 3, 0, 7)
      g.fill()
    }
    g.restore()
  }

  /** `name` cut in the middle to fit `max` pixels, or '' when too little of it would be left. */
  function shorten(g: CanvasRenderingContext2D, name: string, max: number) {
    if (g.measureText(name).width <= max) return name
    for (let n = name.length - 1; n >= 4; n--) {
      const head = Math.ceil(n * 0.6)
      const s = `${name.slice(0, head).trimEnd()}…${name.slice(name.length - (n - head)).trimStart()}`
      if (g.measureText(s).width <= max) return s
    }
    return ''
  }

  onMount(() => onFrame(draw))

  const total = $derived(devices.reduce((a, d) => a + d.colors.length, 0))
</script>

<div class="stage">
  <!-- Clicking a device on the matrix opens it, a pointer shortcut: the rail's target list does the
       same for the keyboard and screen readers. -->
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <canvas
    bind:this={canvas}
    aria-hidden={arrange ? undefined : 'true'}
    tabindex={arrange ? 0 : undefined}
    role={arrange ? 'application' : undefined}
    aria-label={arrange ? 'Desk map. Arrow keys move the picked device, Shift moves it further, R turns it, Tab picks the next device.' : undefined}
    class:pointer={hover >= 0 && selected < 0}
    class:grab={arrange && hover >= 0}
    class:dragging={drag}
    onpointerdown={(e) => {
      if (!arrange) return
      const d = devAt(e.offsetX, e.offsetY)
      if (d < 0) return
      arrange.onpick(d)
      const spot = drawnSpots()[d]
      if (!spot) return
      drag = { i: d, from: cellAt(e.offsetX, e.offsetY), spot }
      ;(e.currentTarget as HTMLCanvasElement).setPointerCapture(e.pointerId)
    }}
    onpointerup={() => {
      if (!drag) return
      drag = null
      camTo(600)
    }}
    onpointercancel={() => {
      drag = null
      camTo(600)
    }}
    onpointermove={(e) => {
      if (drag && arrange) {
        const [i, j] = cellAt(e.offsetX, e.offsetY)
        const s = { ...drag.spot, x0: drag.spot.x0 + i - drag.from[0], y0: drag.spot.y0 + j - drag.from[1] }
        const cur = spots[drag.i]
        if (!cur || cur.x0 !== s.x0 || cur.y0 !== s.y0) arrange.onmove(drag.i, s)
      }
      hover = selected < 0 ? devAt(e.offsetX, e.offsetY) : -1
      if (!ptr) glowAt = { x: e.offsetX, y: e.offsetY }
      ptr = { x: e.offsetX, y: e.offsetY }
      tw.to('glow', 1, 300)
    }}
    onpointerleave={() => {
      hover = -1
      ptr = null
      tw.to('glow', 0, 520)
    }}
    onclick={(e) => {
      if (arrange) return
      const d = selected < 0 ? devAt(e.offsetX, e.offsetY) : -1
      if (d >= 0) onselect(parts[d]?.dev ?? d)
    }}
    onkeydown={(e) => {
      if (!arrange || e.altKey || e.ctrlKey || e.metaKey) return
      const n = parts.length
      if (e.key === 'Tab') {
        const next = arrange.sel + (e.shiftKey ? -1 : 1)
        // Past either end, Tab leaves the map as usual.
        if (next < 0 || next >= n) return
        e.preventDefault()
        arrange.onpick(next)
        return
      }
      const i = arrange.sel
      if (i < 0 || i >= n) {
        if (e.key.startsWith('Arrow')) {
          e.preventDefault()
          arrange.onpick(0)
        }
        return
      }
      if (e.key === 'r' || e.key === 'R') {
        e.preventDefault()
        arrange.onturn(i)
        return
      }
      const step = e.shiftKey ? 4 : 1
      const d = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] }[e.key]
      if (!d) return
      e.preventDefault()
      const at = drawnSpots()[i]
      if (at) arrange.onmove(i, { ...at, x0: at.x0 + d[0], y0: at.y0 + d[1] })
    }}
  ></canvas>
  {#if notice}
    <div class="notice">{@render notice()}</div>
  {:else if devices.length > 0}
    <div class="cap num">
      <span>{selected >= 0 ? names[selected] : (focus?.name ?? 'All devices')}</span>
      <span
        >{selected >= 0
          ? devices[selected]?.colors.length
          : total} LEDs</span
      >
    </div>
  {/if}
</div>

<style>
  .stage {
    grid-area: stage;
    position: relative;
    height: clamp(210px, 35vw, 480px);
    transition: height var(--slow) var(--ease);
  }
  /* The matrix fades into the page at its edges; the caption sits in its own strip below. */
  canvas {
    position: absolute;
    left: 0;
    top: 0;
    width: 100%;
    height: calc(100% - 22px);
    -webkit-mask:
      linear-gradient(90deg, transparent, #000 28px, #000 calc(100% - 28px), transparent),
      linear-gradient(transparent, #000 18px, #000 calc(100% - 18px), transparent);
    -webkit-mask-composite: source-in;
    mask:
      linear-gradient(90deg, transparent, #000 28px, #000 calc(100% - 28px), transparent),
      linear-gradient(transparent, #000 18px, #000 calc(100% - 18px), transparent);
    mask-composite: intersect;
  }
  canvas.pointer {
    cursor: pointer;
  }
  canvas.grab {
    cursor: grab;
  }
  canvas.dragging {
    cursor: grabbing;
  }
  canvas:focus-visible {
    outline-offset: -2px;
  }
  .cap {
    position: absolute;
    left: 28px;
    right: 28px;
    bottom: 0;
    display: flex;
    justify-content: space-between;
    color: var(--muted);
    font-size: 11px;
    pointer-events: none;
  }
  .notice {
    position: absolute;
    inset: 0;
    display: grid;
    place-content: center;
    justify-items: center;
    text-align: center;
    gap: 12px;
    color: var(--muted);
    padding: 20px;
    animation: in var(--slow) var(--ease);
  }
  .notice :global(p) {
    max-width: 46ch;
    color: var(--text);
  }
  @media (max-width: 700px) {
    .stage {
      height: clamp(180px, 52vw, 280px);
    }
  }
</style>
