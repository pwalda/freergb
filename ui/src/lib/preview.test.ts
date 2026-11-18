import { test } from 'node:test'
import assert from 'node:assert/strict'
import type { Device } from './api.ts'
import { buildEffect, buildScene, DEFAULT_PARAMS, EFFECTS, type DeviceKey } from './effects.ts'
import { lookParams } from './looks.ts'
import { readFileSync } from 'node:fs'
import { arrange, canvas, glow, glowAt, GLOW_STOPS, ledLit, litAt, localPoints, lookOf, placedPieces, type Glow, type Point } from './preview.ts'
import type { Placement } from './api.ts'

const key = (nth: number): DeviceKey => ({ name: 'RAM', location: 'i2c', serial: '', nth })
const dev = (nth: number, n = 5): Device => ({
  id: nth + 1, key: key(nth), name: 'RAM', vendor: 'X', description: '', version: '', serial: '', location: '', device_type: 1, active_mode: 0, modes: [],
  zones: [{ index: 0, name: 'Z', leds: n, min_leds: n, max_leds: n, resizable: false, start: 0, matrix: null, modes: [], active_mode: -1 }],
  pieces: [],
  colors: Array.from({ length: n }, () => '#804020'),
})

const us = (pos: Point[]) => pos.map((p) => p[0])

test('LED spots run along the controller order at mid-height; a matrix zone uses its cells', () => {
  assert.deepEqual(us(localPoints(dev(0, 5))), [0, 0.25, 0.5, 0.75, 1])
  const kb = dev(0, 4)
  kb.zones = [{ index: 0, name: 'K', leds: 4, min_leds: 4, max_leds: 4, resizable: false, start: 0, matrix: { width: 3, height: 2, cells: [0, null, 1, 2, 3, null] }, modes: [], active_mode: -1 }]
  assert.deepEqual(localPoints(kb), [[0, 0], [1, 0], [0, 1], [0.5, 1]])
})

test('a shared all-devices look gives each device its slice of the canvas, like the engine', () => {
  const ds = [dev(0), dev(1)]
  const scenes = [buildScene({ ...DEFAULT_PARAMS, kind: 'rainbow' })]
  const a = lookOf(ds, 0, scenes), b = lookOf(ds, 1, scenes)
  assert.ok(a.kind === 'effect' && b.kind === 'effect')
  assert.deepEqual(us(a.pos), [0, 0.125, 0.25, 0.375, 0.5])
  assert.deepEqual(us(b.pos), [0.5, 0.625, 0.75, 0.875, 1])
  const per = lookOf(ds, 1, [buildScene({ ...DEFAULT_PARAMS, kind: 'rainbow', perDevice: true })])
  assert.ok(per.kind === 'effect' && per.pos[0][0] === 0 && per.pos[4][0] === 1)
})

test('a device’s own scene wins over the all-devices one; live edits show ahead of the scene', () => {
  const ds = [dev(0), dev(1)]
  const scenes = [buildScene({ ...DEFAULT_PARAMS, kind: 'rainbow' }), buildScene({ ...DEFAULT_PARAMS, kind: 'static' }, { devices: [key(1)] })]
  const own = lookOf(ds, 1, scenes)
  assert.ok(own.kind === 'effect' && own.spec.type === 'static' && own.pos[4][0] === 1)
  const live = { all: { ...scenes[0].effect, speed: 2 }, dev: { id: 'x', spec: { ...scenes[1].effect, brightness: 0.2 } } }
  const a = lookOf(ds, 0, scenes, live), b = lookOf(ds, 1, scenes, live, 'x')
  assert.ok(a.kind === 'effect' && a.spec.speed === 2)
  assert.ok(b.kind === 'effect' && b.spec.brightness === 0.2)
})

test('with no scene, a built-in mode or a painting the stage shows colours per LED', () => {
  const ds = [dev(0)]
  const none = lookOf(ds, 0, [])
  assert.ok(none.kind === 'colors' && none.level < 1 && none.colors[0][0] === 128)
  const painted = lookOf(ds, 0, [{ effect: { type: 'painted', colors: ['#ff0000', '#000000'], brightness: 0.5 }, targets: { devices: [key(0)] }, canvas: 'shared' }])
  assert.deepEqual(ledLit(painted, 0, 0), [255, 0, 0, 0.5])
  assert.deepEqual(ledLit(painted, 1, 0), [0, 0, 0, 0])
})

test('an effect colour is split into full-intensity hue and level; black keeps a hue', () => {
  assert.deepEqual(litAt({ type: 'static', colors: ['#804000'], brightness: 1 }, 0, 0), [255, 127.5, 0, 128 / 255])
  assert.deepEqual(litAt({ type: 'static', colors: ['#000000'] }, 0, 0, [1, 2, 3]), [1, 2, 3, 0])
})

test('a vial glows without snapping: no look jumps colour or level from one frame to the next', () => {
  for (const fps of [60, 30]) {
    for (const e of EFFECTS) {
      const spec = buildEffect({ ...lookParams(e.id, { ...DEFAULT_PARAMS }), brightness: 1 })
      let g: Glow | undefined
      for (let f = 0; f < fps * 20; f++) {
        const next = glow(spec, f / fps, 1 / fps, g, [255, 0, 255])
        if (g)
          for (let i = 0; i < GLOW_STOPS; i++) {
            const a = glowAt(g, i / (GLOW_STOPS - 1))
            const b = glowAt(next, i / (GLOW_STOPS - 1))
            const hue = Math.max(...[0, 1, 2].map((n) => Math.abs(a[n] - b[n])))
            assert.ok(hue <= 60, `${e.id} at ${fps} fps: hue moved ${hue.toFixed(0)} in a frame`)
            assert.ok(Math.abs(a[3] - b[3]) <= 0.25, `${e.id} at ${fps} fps: level moved ${Math.abs(a[3] - b[3]).toFixed(2)}`)
          }
        g = next
      }
    }
  }
})

test('a still look glows as it is: the first frame is the look, and it stays put', () => {
  const spec = buildEffect({ ...DEFAULT_PARAMS, kind: 'static' })
  const first = glow(spec, 0, 1 / 60, undefined)
  const later = glow(spec, 5, 1 / 60, first)
  assert.deepEqual(glowAt(later, 0).map(Math.round), glowAt(first, 0).map(Math.round))
  assert.deepEqual(glowAt(first, 0).slice(0, 3).map(Math.round), litAt(spec, 0, 0).slice(0, 3).map(Math.round))
})

interface LayoutCase {
  pieces: { local: Point[]; at: Placement | null }[]
  angle: number
  uv: Point[]
}

test('the layout reproduces layout.rs on every golden case', () => {
  const { cases }: { cases: LayoutCase[] } = JSON.parse(
    readFileSync(new URL('../../../crates/freergb-effects/tests/layout_vectors.json', import.meta.url), 'utf8'),
  )
  assert.ok(cases.length > 0)
  for (const c of cases) {
    const got = canvas(arrange(c.pieces).flat(), c.angle)
    assert.equal(got.length, c.uv.length)
    got.forEach((p, i) => {
      for (const k of [0, 1]) assert.ok(Math.abs(p[k] - c.uv[i][k]) < 1e-9, `angle ${c.angle} point ${i}: js ${p} vs rust ${c.uv[i]}`)
    })
  }
})

test('a placed device follows the desk map, and the angle turns the canvas round', () => {
  const ds = [dev(0), dev(1)]
  const at = (x: number): Placement => ({ x, y: 0, w: 1, h: 1, rot: 0 })
  const map = [{ device: key(0), at: at(5) }, { device: key(1), at: at(0) }]
  const scenes = [buildScene({ ...DEFAULT_PARAMS, kind: 'rainbow' })]
  const a = lookOf(ds, 0, scenes, {}, '', map), b = lookOf(ds, 1, scenes, {}, '', map)
  assert.ok(a.kind === 'effect' && b.kind === 'effect')
  assert.ok(us(b.pos).every((u) => u <= 0.5) && us(a.pos).every((u) => u >= 0.5))
  const back = [{ ...scenes[0], effect: { ...scenes[0].effect, angle: 180 } }]
  const r = lookOf(ds, 0, back, {}, '', map)
  assert.ok(r.kind === 'effect' && us(r.pos).every((u) => u <= 0.5))
})

test('a segment sits where its own entry puts it, the rest where the device’s does (as in plan)', () => {
  const strip = dev(0, 300)
  const range = (a: number, b: number) => Array.from({ length: b - a }, (_, i) => a + i)
  const shelf = { zone: 0, start: 200 }
  strip.pieces = [
    { segment: null, name: '', leds: range(0, 200) },
    { segment: shelf, name: 'Shelf', leds: range(200, 300) },
  ]
  const at = (x: number): Placement => ({ x, y: 0, w: 1, h: 1, rot: 0 })
  const map = [{ device: key(0), at: at(5) }, { device: key(0), at: at(0), segment: shelf }]
  const look = lookOf([strip], 0, [buildScene({ ...DEFAULT_PARAMS, kind: 'rainbow', perDevice: true })], {}, '', map)
  assert.ok(look.kind === 'effect')
  const u = us(look.pos)
  assert.equal(u.length, 300)
  assert.ok(u.slice(200).every((x) => x <= 0.5) && u.slice(0, 200).every((x) => x >= 0.5))
  assert.equal(u[200], 0)
  assert.equal(u[199], 1)
  // Undrawn, a segment is a straight run; drawn spots are used while they fit its LED count.
  const p = placedPieces(strip, [])
  assert.deepEqual([p[1].local[0], p[1].local[99]], [[0, 0.5], [1, 0.5]])
  const drawn = range(0, 100).map((i): [number, number] => [0.5, i / 99])
  assert.deepEqual(placedPieces(strip, [{ device: key(0), points: drawn, segment: shelf }])[1].local, drawn)
})
