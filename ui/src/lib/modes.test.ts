import { test } from 'node:test'
import assert from 'node:assert/strict'
import type { ModeView } from './api.ts'
import { buildScene } from './effects.ts'
import { lookOfParams, modeColorCount, modeNameOf, modeStart, modeValue, withZoneLook, zoneLookOf, zoneParams } from './modes.ts'

const breathing: ModeView = {
  index: 3,
  name: 'Breathing',
  speed: 0.5,
  brightness: 1,
  directions: ['left', 'right', 'up', 'down'],
  direction: 'up',
  colors_min: 2,
  colors_max: 3,
  colors: ['#ff0000', '#0000ff'],
  saveable: true,
  whole_device: false,
}
const off: ModeView = { ...breathing, name: 'Off', speed: null, brightness: null, directions: [], direction: null, colors_min: 0, colors_max: 0, colors: [] }

test('a mode starts from the values the device reports for it', () => {
  const p = modeStart(breathing)
  assert.deepEqual(
    [p.kind, p.mode, p.speed, p.brightness, p.modeDirection, p.colors],
    ['device_mode', 'Breathing', 0.5, 1, 'up', ['#ff0000', '#0000ff']],
  )
  const scene = buildScene(p, { devices: [{ name: 'RAM', location: '', serial: '', nth: 0 }] })
  assert.equal(scene.effect.mode, 'Breathing')
  assert.equal(scene.effect.mode_direction, 'up')
  assert.equal('density' in scene.effect || 'direction' in scene.effect, false, 'no software-only fields')
})

test('a mode without settings builds a scene with none of them', () => {
  const p = modeStart(off)
  assert.deepEqual(p.colors, [])
  assert.equal(p.modeDirection, null)
  const e = buildScene(p).effect
  assert.equal('mode_direction' in e, false)
  assert.deepEqual(e.colors, [])
})

test('colour pickers: none for a mode with no colours, at least one otherwise, never more than it takes', () => {
  assert.equal(modeColorCount(off), 0)
  assert.equal(modeColorCount(breathing), 2)
  assert.equal(modeColorCount({ ...breathing, colors_min: 0, colors_max: 4 }), 1)
  assert.equal(modeColorCount({ ...breathing, colors_min: 5, colors_max: 3 }), 3)
  // a device that reports fewer colours than it needs gets the last one repeated
  assert.deepEqual(modeStart({ ...breathing, colors: ['#123456'] }).colors, ['#123456', '#123456'])
})

test('picker values tell modes from effects apart, even for a mode named like an effect', () => {
  assert.equal(modeNameOf(modeValue('Rainbow')), 'Rainbow')
  assert.equal(modeNameOf('rainbow'), undefined)
  assert.equal(modeNameOf(modeValue('a:b')), 'a:b')
})

test('a zone look starts from the zone mode the device reports and round-trips through the controls', () => {
  const l = zoneLookOf(2, breathing)
  assert.deepEqual(l, { zone: 2, mode: 'Breathing', colors: ['#ff0000', '#0000ff'], speed: 0.5, brightness: 1, mode_direction: 'up' })
  assert.deepEqual(lookOfParams(2, zoneParams(l)), l)
  assert.equal('mode_direction' in zoneLookOf(0, off), false)
})

test('zone looks: one per zone, in zone order, dropped when the zone follows the device', () => {
  const a = zoneLookOf(2, breathing)
  const b = zoneLookOf(0, off)
  let looks = withZoneLook(undefined, 2, a)
  looks = withZoneLook(looks, 0, b)
  assert.deepEqual(looks.map((l) => l.zone), [0, 2])
  looks = withZoneLook(looks, 2, { ...a, speed: 0.9 })
  assert.deepEqual(looks.map((l) => [l.zone, l.speed]), [[0, 0.5], [2, 0.9]])
  assert.deepEqual(withZoneLook(looks, 0, null).map((l) => l.zone), [2])
})

test('a device mode scene carries its zone looks, and only when it has some', () => {
  const key = { name: 'Hub', location: '', serial: '', nth: 0 }
  const p = { ...modeStart(breathing), zones: [zoneLookOf(1, off)] }
  const e = buildScene(p, { devices: [key] }).effect
  assert.deepEqual(e.zones, [{ zone: 1, mode: 'Off', colors: [], speed: 0.5, brightness: 1 }])
  assert.equal('zones' in buildScene({ ...p, zones: [] }, { devices: [key] }).effect, false)
})
