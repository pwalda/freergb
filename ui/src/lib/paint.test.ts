import { test } from 'node:test'
import assert from 'node:assert/strict'
import type { Device, Zone } from './api.ts'
import { fill, layoutOf, near, OFF, paintedScene, paletteOf, startColors, step, withColor } from './paint.ts'
import { buildScene, DEFAULT_PARAMS } from './effects.ts'

const key = { name: 'K70', location: 'HID', serial: '', nth: 0 }
const device = (colors: string[]) => ({ colors }) as Device

test('painting starts from what the device shows, or from its own painting cut or padded to its LEDs', () => {
  assert.deepEqual(startColors(device(['#111111', '#222222']), undefined), ['#111111', '#222222'])
  const wave = buildScene({ ...DEFAULT_PARAMS, kind: 'wave' }, { devices: [key] })
  assert.deepEqual(startColors(device(['#111111']), wave), ['#111111'], 'another effect: the live colours')
  assert.deepEqual(startColors(device(['#1', '#2', '#3']), paintedScene(key, ['#ff0000'])), ['#ff0000', OFF, OFF])
  assert.deepEqual(startColors(device(['#1']), paintedScene(key, ['#ff0000', '#00ff00'])), ['#ff0000'])
})

test('fill sets one zone and leaves the rest', () => {
  assert.deepEqual(fill(['a', 'b', 'c', 'd'], 1, 2, 'x'), ['a', 'x', 'x', 'd'])
})

test('the palette lists used colours, most used first, case-insensitively, without off', () => {
  assert.deepEqual(paletteOf(['#FF0000', '#00ff00', '#ff0000', '#0000ff', '#00ff00', '#ff0000', OFF, OFF, OFF, OFF], 2), ['#ff0000', '#00ff00'])
})

test('colours that look alike share one swatch, so a frozen gradient does not fill the row', () => {
  const hex = (r: number, b: number) => `#${r.toString(16).padStart(2, '0')}00${b.toString(16).padStart(2, '0')}`
  const gradient = Array.from({ length: 64 }, (_, i) => hex(i * 4, 255 - i * 4))
  const p = paletteOf(gradient)
  assert.ok(p.length >= 3 && p.length <= 8, `${p.length}`)
  for (const a of p) for (const b of p) assert.ok(a === b || !near(a, b), `${a} ~ ${b}`)
  assert.deepEqual(withColor(['#ff0000'], '#fe0101'), ['#ff0000'])
  assert.deepEqual(withColor(['#ff0000'], '#00FF00'), ['#ff0000', '#00ff00'])
})

test('a zone with a grid is drawn as its grid, any other as a row starting at its first LED', () => {
  const z = { start: 4, leds: 3, matrix: null } as Zone
  assert.deepEqual(layoutOf(z), { kind: 'strip', leds: [4, 5, 6] })
  const m = { ...z, matrix: { width: 2, height: 1, cells: [4, null] } }
  assert.deepEqual(layoutOf(m), { kind: 'matrix', width: 2, cells: [4, null] })
})

test('a painting is a single-device scene of every LED colour', () => {
  assert.deepEqual(paintedScene(key, ['#ff0000', OFF], 0.5), {
    effect: { type: 'painted', colors: ['#ff0000', OFF], brightness: 0.5 },
    targets: { devices: [key] },
    canvas: 'shared',
  })
})

test('arrow keys move by row and column in a grid, skipping gaps, and by one in a row of LEDs', () => {
  // 3x2 grid: [0, null, 1] / [2, 3, null]
  const g = { kind: 'matrix' as const, width: 3, cells: [0, null, 1, 2, 3, null] }
  assert.equal(step(g, 0, 'ArrowRight'), 1, 'skips the gap')
  assert.equal(step(g, 1, 'ArrowRight'), 1, 'stops at the row end')
  assert.equal(step(g, 0, 'ArrowDown'), 2)
  assert.equal(step(g, 1, 'ArrowDown'), 1, 'nothing below')
  assert.equal(step(g, 3, 'ArrowUp'), 3, 'a gap above and nothing past it')
  assert.equal(step(g, 3, 'End'), 3)
  assert.equal(step(g, 3, 'Home'), 0)
  const s = { kind: 'strip' as const, leds: [4, 5, 6] }
  assert.equal(step(s, 4, 'ArrowLeft'), 4)
  assert.equal(step(s, 4, 'ArrowDown'), 5)
  assert.equal(step(s, 4, 'End'), 6)
  assert.equal(step(s, 5, 'a'), 5)
})
