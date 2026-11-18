import { test } from 'node:test'
import assert from 'node:assert/strict'
import { buildScene, DEFAULT_PARAMS, type DeviceKey, type Scene } from './effects.ts'
import {
  deviceScene,
  lightsOut,
  OFF_SCENE,
  globalScene,
  groupLookCount,
  groupOf,
  groupScene,
  inheritedScene,
  withGroupScene,
  keyId,
  ownEffectCount,
  sameScene,
  withDeviceScene,
  withGlobalScene,
} from './scenes.ts'

const key = (nth: number, name = 'RAM'): DeviceKey => ({ name, location: 'i2c', serial: '', nth })
const A = key(0)
const B = key(1)
const on = (k: DeviceKey, kind: Scene['effect']['type']): Scene =>
  buildScene({ ...DEFAULT_PARAMS, kind }, { devices: [k] })
const all = (kind: Scene['effect']['type']): Scene => buildScene({ ...DEFAULT_PARAMS, kind })

test('a device is found by all four key fields, and only by a scene that names it alone', () => {
  const scenes = [on(A, 'wave'), on(B, 'static')]
  assert.equal(deviceScene(scenes, A)?.effect.type, 'wave')
  assert.equal(deviceScene(scenes, B)?.effect.type, 'static')
  assert.equal(deviceScene(scenes, key(0, 'Other')), undefined)
  assert.equal(deviceScene(scenes, key(2)), undefined)
  const group = buildScene(DEFAULT_PARAMS, { devices: [A, B] })
  assert.equal(deviceScene([group], A), undefined, 'a group scene is not A’s own effect')
})

test('the global scene is the last "all" scene', () => {
  assert.equal(globalScene([on(A, 'wave')]), undefined)
  assert.equal(globalScene([all('rainbow'), on(A, 'wave'), all('breathing')])?.effect.type, 'breathing')
})

test('changing one device leaves every other device’s scene exactly as it was', () => {
  const before = [all('rainbow'), on(A, 'wave'), on(B, 'static')]
  const after = withDeviceScene(before, A, on(A, 'breathing'))
  assert.equal(deviceScene(after, A)?.effect.type, 'breathing')
  assert.deepEqual(deviceScene(after, B), before[2])
  assert.deepEqual(globalScene(after), before[0])
  assert.equal(after.length, 3, 'A’s old scene was replaced, not kept')
})

test('removing a device’s effect hands it back to the all-devices scene', () => {
  const before = [all('rainbow'), on(A, 'wave')]
  const after = withDeviceScene(before, A, null)
  assert.equal(deviceScene(after, A), undefined)
  assert.equal(globalScene(after)?.effect.type, 'rainbow')
  assert.deepEqual(withDeviceScene([on(B, 'static')], A, null), [on(B, 'static')])
})

test('group scenes that include the device survive an edit and are overridden by it', () => {
  const group = buildScene(DEFAULT_PARAMS, { devices: [A, B] })
  const after = withDeviceScene([group], A, on(A, 'static'))
  assert.deepEqual(after[0], group)
  assert.equal(after.at(-1), after[1], 'the override comes last so it wins')
  assert.equal(deviceScene(after, A)?.effect.type, 'static')
})

test('keyId tells apart devices whose names are the same', () => {
  assert.notEqual(keyId(A), keyId(B))
  assert.equal(keyId(A), keyId({ ...A }))
})

test('ownEffectCount counts devices, not scenes', () => {
  assert.equal(ownEffectCount([all('rainbow')]), 0)
  assert.equal(ownEffectCount([all('rainbow'), on(A, 'wave'), on(B, 'static')]), 2)
  assert.equal(ownEffectCount([on(A, 'wave'), on(A, 'static')]), 1)
})

test('sameScene ignores property order but not values', () => {
  const s = on(A, 'wave')
  const reordered = JSON.parse(JSON.stringify({ canvas: s.canvas, targets: s.targets, effect: s.effect }))
  assert.ok(sameScene(s, reordered))
  assert.ok(!sameScene(s, { ...s, effect: { ...s.effect, speed: 1.5 } }))
  assert.ok(sameScene(undefined, undefined))
  assert.ok(!sameScene(s, undefined))
})

test('retuning the all-devices effect keeps every device’s own effect, and they still win', () => {
  const group = buildScene(DEFAULT_PARAMS, { devices: [A, B] })
  const before = [all('rainbow'), on(A, 'wave'), group]
  const after = withGlobalScene(before, all('breathing'))
  assert.equal(globalScene(after)?.effect.type, 'breathing')
  assert.deepEqual(after.slice(1), before.slice(1))
  assert.equal(after.filter((s) => s.targets === 'all').length, 1)
  assert.equal(after[0].targets, 'all', 'first, so later per-device scenes override it')
})

test('removing the all-devices effect leaves per-device ones; setting one when none ran adds it', () => {
  const before = [on(A, 'wave'), all('rainbow'), all('static')]
  assert.deepEqual(withGlobalScene(before, null), [before[0]])
  assert.deepEqual(withGlobalScene([on(A, 'wave')], all('wave')).map((s) => s.targets), ['all', { devices: [A] }])
})

const C = key(2)
const desk = { name: 'Desk', devices: [A, B] }
const inGroup = (name: string, devices: DeviceKey[], kind: Scene['effect']['type']): Scene =>
  buildScene({ ...DEFAULT_PARAMS, kind }, { devices }, name)

test('a one-device group look is the group’s, not the device’s own', () => {
  const solo = inGroup('Solo', [A], 'wave')
  assert.equal(deviceScene([solo], A), undefined)
  assert.equal(ownEffectCount([solo]), 0)
  assert.equal(groupLookCount([solo, inGroup('Desk', [B, C], 'fire'), on(A, 'static')]), 2)
})

test('a device inherits its group’s look, else the all-devices look', () => {
  const scenes = [all('rainbow'), inGroup('Desk', [A, B], 'fire')]
  assert.equal(groupOf([desk], B)?.name, 'Desk')
  assert.equal(groupOf([desk], C), undefined)
  assert.equal(inheritedScene(scenes, [desk], A)?.effect.type, 'fire')
  assert.equal(inheritedScene(scenes, [desk], C)?.effect.type, 'rainbow')
  assert.equal(inheritedScene([all('rainbow')], [desk], A)?.effect.type, 'rainbow', 'a group without a look follows all')
})

test('a group look sits between the all-devices look and devices’ own looks, and replacing it keeps the rest', () => {
  const own = on(A, 'static')
  const shelf = inGroup('Shelf', [C], 'rain')
  const scenes = withGroupScene([own, all('rainbow'), shelf], 'Desk', inGroup('Desk', [A, B], 'fire'))
  assert.deepEqual(scenes.map((s) => s.group ?? (s.targets === 'all' ? 'all' : 'own')), ['all', 'Shelf', 'Desk', 'own'])
  const again = withGroupScene(scenes, 'Desk', inGroup('Desk', [A, B], 'comet'))
  assert.equal(groupScene(again, 'Desk')?.effect.type, 'comet')
  assert.equal(again.length, 4)
  assert.deepEqual(withGroupScene(again, 'Desk', null).map((s) => s.group ?? '-'), ['-', 'Shelf', '-'])
})

test('lights are off for the black look on every device, also as the tray sends it', () => {
  assert.ok(lightsOut([OFF_SCENE]))
  const fromTray: Scene = { effect: { type: 'static', colors: ['#000000'], speed: 0, density: 1, brightness: 1, direction: 'forward' }, targets: 'all', canvas: 'shared' }
  assert.ok(lightsOut([fromTray]))
  assert.ok(lightsOut([{ ...OFF_SCENE, effect: { ...OFF_SCENE.effect, colors: ['#ff0000'], brightness: 0 } }]), 'or at zero brightness')
  assert.ok(!lightsOut([]), 'effects off is not lights off')
  assert.ok(!lightsOut([{ ...OFF_SCENE, effect: { ...OFF_SCENE.effect, colors: ['#ff0000'] } }]))
  assert.ok(!lightsOut([OFF_SCENE, buildScene({ ...DEFAULT_PARAMS, kind: 'wave' })]), 'a device with its own look is lit')
})
