import { test } from 'node:test'
import assert from 'node:assert/strict'
import { bestLayout, crowds, defaultShape, deskLayout, freeSpot, KEY_COUNT, layout, ledSpots, placementOf, SHAPES, slotLed, spotOf, template, turned, type Turn } from './rig.ts'
import { onDesk } from './preview.ts'

test('every template keeps its slots within 0..slots-1 and its cells inside its box', () => {
  for (const { id } of SHAPES) {
    for (const n of [1, 6, 8, 48, 104, 300]) {
      const t = template(id, n)
      for (const c of t.cells) {
        assert.ok(c.x >= 0 && c.x < t.w && c.y >= 0 && c.y < t.h, `${id}/${n} cell ${c.x},${c.y} in ${t.w}x${t.h}`)
        if (c.t === 'l') assert.ok(c.k >= 0 && c.k < t.slots, `${id}/${n} slot ${c.k} < ${t.slots}`)
      }
    }
  }
})

test('the keyboard is a full-size 104-key layout with no two keys on one dot', () => {
  assert.equal(KEY_COUNT, 104)
  const t = template('keys', 104)
  const seen = new Map<string, number>()
  for (const c of t.cells.filter((c) => c.t === 'l')) {
    const at = `${c.x},${c.y}`
    assert.ok(!seen.has(at) || seen.get(at) === c.k, `dot ${at} shared by keys ${seen.get(at)} and ${c.k}`)
    seen.set(at, c.k)
  }
  assert.equal(new Set(t.cells.filter((c) => c.t === 'l').map((c) => c.k)).size, 104)
})

test('long strips and sticks are sampled down instead of growing without end', () => {
  assert.equal(template('strip', 300).slots, 120)
  assert.equal(template('stick', 100).slots, 32)
  assert.equal(template('fans', 48).slots, 48)
})

test('a device’s LEDs are spread evenly over the slots, first to last', () => {
  assert.equal(slotLed(0, 104, 6), 0)
  assert.equal(slotLed(103, 104, 6), 5)
  assert.equal(slotLed(119, 120, 300), 297)
  assert.equal(slotLed(3, 8, 8), 3)
  assert.equal(slotLed(0, 8, 0), -1)
})

test('default shapes follow the device type', () => {
  assert.equal(defaultShape(0), 'board')
  assert.equal(defaultShape(1), 'stick')
  assert.equal(defaultShape(5), 'keys')
  assert.equal(defaultShape(3), 'fans')
  assert.equal(defaultShape(4), 'strip')
  assert.equal(defaultShape(99), 'strip')
})

test('layout never overlaps two devices and keeps them inside the matrix', () => {
  const ts = [template('stick', 8), template('stick', 8), template('board', 6), template('keys', 104), template('fans', 48)]
  for (const rows of [1, 2, 3]) {
    const l = layout(ts, rows)
    l.placed.forEach((a, i) => {
      assert.ok(a.x0 >= 0 && a.y0 >= 0 && a.x0 + a.w <= l.W && a.y0 + a.h <= l.H, `device ${i} inside`)
      l.placed.forEach((b, j) => {
        if (i >= j) return
        const apart = a.x0 + a.w <= b.x0 || b.x0 + b.w <= a.x0 || a.y0 + a.h <= b.y0 || b.y0 + b.h <= a.y0
        assert.ok(apart, `rows ${rows}: devices ${i} and ${j} overlap`)
      })
    })
  }
})

test('a wide stage keeps one row, a squarer one wraps into more', () => {
  const ts = [template('stick', 8), template('board', 6), template('keys', 104), template('fans', 48)]
  const tallest = Math.max(...ts.map((t) => t.h))
  assert.equal(bestLayout(ts, 3000, 200).H, tallest, 'one row is as tall as its tallest device')
  assert.ok(bestLayout(ts, 700, 400).H > tallest)
})

test('a strip with a length is one straight row of that many lights', () => {
  const t = template('strip', 60, 12)
  assert.equal(t.slots, 12)
  assert.equal(t.h, 3)
  assert.ok(t.cells.filter((c) => c.t === 'l').every((c) => c.y === 1))
})

test('a quarter turn swaps the box and keeps every cell inside it', () => {
  const t = template('board', 12)
  for (const rot of [90, 180, 270] as Turn[]) {
    const r = turned(t, rot)
    assert.deepEqual([r.w, r.h], rot === 180 ? [t.w, t.h] : [t.h, t.w])
    for (const c of r.cells) assert.ok(c.x >= 0 && c.x < r.w && c.y >= 0 && c.y < r.h)
  }
  assert.deepEqual(turned(turned(t, 90), 270).cells, t.cells)
})

test('LED spots follow the drawing: a fan lights round its ring', () => {
  const fan = template('fans', 16)
  const pts = ledSpots(fan, 16)
  assert.equal(pts.length, 16)
  const r = pts.map(([x, y]) => Math.hypot(x - 0.5, y - 0.5))
  assert.ok(Math.max(...r) - Math.min(...r) < 0.1, 'all on one ring')
  // More LEDs than lights: each LED still gets a spot inside the square.
  const many = ledSpots(template('strip', 200, 10), 200)
  assert.equal(many.length, 200)
  assert.ok(many.every(([x, y]) => x >= 0 && x <= 1 && y >= 0 && y <= 1))
})

test('a spot and its placement put every light on the same desk cell as the engine', () => {
  const t = template('board', 12)
  for (const rot of [0, 90, 180, 270] as Turn[]) {
    const spot = { x0: 7, y0: -3, rot }
    const at = placementOf(t, spot)
    assert.deepEqual(spotOf(t, at), spot)
    const tt = turned(t, rot)
    const lights = t.cells.filter((c) => c.t === 'l')
    const local = lights.map((c) => [c.x / (t.w - 1), c.y / (t.h - 1)] as [number, number])
    const desk = onDesk(local, at)
    const want = tt.cells.filter((c) => c.t === 'l').map((c) => [c.x + spot.x0, c.y + spot.y0])
    desk.forEach(([x, y], i) => {
      assert.ok(Math.abs(x - want[i][0]) < 1e-9 && Math.abs(y - want[i][1]) < 1e-9, `rot ${rot} light ${i}: ${x},${y} vs ${want[i]}`)
    })
  }
})

test('the desk layout draws placed devices where they sit and the rest to their right', () => {
  const a = template('stick', 8), b = template('stick', 8), c = template('ring', 12)
  const l = deskLayout([a, b, c], [{ x0: -5, y0: 2, rot: 0 }, { x0: 10, y0: 0, rot: 90 }, null])
  assert.deepEqual([l.ox, l.oy], [-5, 0])
  assert.deepEqual(l.placed[0], { x0: 0, y0: 2, w: a.w, h: a.h })
  assert.deepEqual(l.placed[1], { x0: 15, y0: 0, w: b.h, h: b.w })
  assert.equal(l.placed[2].x0, 15 + b.h + 4)
})

test('a device never lands on another: it takes the nearest spot with a free dot between', () => {
  const other = { x0: 10, y0: 0, w: 5, h: 5 }
  assert.deepEqual(freeSpot([other], 3, 3, { x0: 0, y0: 0 }), { x0: 0, y0: 0 })
  const at = freeSpot([other], 3, 3, { x0: 9, y0: 1 })
  assert.ok(!crowds({ ...at, w: 3, h: 3 }, other), `free at ${JSON.stringify(at)}`)
  // Just touching is crowding: one empty dot is kept between devices.
  assert.ok(crowds({ x0: 15, y0: 0, w: 2, h: 2 }, other))
  assert.ok(!crowds({ x0: 16, y0: 0, w: 2, h: 2 }, other))
  // Nearest: one step left of the obstacle rather than across it.
  assert.deepEqual(freeSpot([other], 3, 3, { x0: 7, y0: 1 }), { x0: 6, y0: 1 })
})
