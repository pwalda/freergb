import { test } from 'node:test'
import assert from 'node:assert/strict'
import { buildScene, DEFAULT_PARAMS, OWN_PALETTE, paramsOf } from './effects.ts'
import { lookParams } from './looks.ts'
import { OFF_SCENE } from './scenes.ts'

test('choosing a look keeps the running look’s tuning and changes only the effect', () => {
  const running = { ...DEFAULT_PARAMS, kind: 'wave' as const, colors: ['#111111', '#222222', '#333333'], speed: 1.5, brightness: 0.4, reverse: true, perDevice: true }
  const next = lookParams('breathing', running)
  assert.deepEqual(next, { ...running, kind: 'breathing' })
  assert.notEqual(next.colors, running.colors, 'a copy, so editing one never edits the other')
})

test('after Lights off a look starts from the defaults, not from black', () => {
  // As paramsOf reads the Lights off scene: one black colour, the unused slots filled in.
  const off = paramsOf(OFF_SCENE)
  assert.deepEqual(lookParams('wave', off).colors, DEFAULT_PARAMS.colors)
})

test('with nothing running a look starts from the defaults', () => {
  assert.deepEqual(lookParams('static'), { ...DEFAULT_PARAMS, kind: 'static' })
})

test('fire starts from its own warm palette, but keeps its colours once it is running', () => {
  const running = { ...DEFAULT_PARAMS, kind: 'wave' as const, colors: ['#000000', '#0000ff', '#ffffff'] }
  assert.deepEqual(lookParams('fire', running).colors, OWN_PALETTE.fire)
  const fire = { ...DEFAULT_PARAMS, kind: 'fire' as const, colors: ['#111111', '#222222', '#333333'] }
  assert.deepEqual(lookParams('fire', fire).colors, fire.colors)
})

test('a two-colour look switched to a three-colour one brings in no colour the user did not choose', () => {
  const comet = paramsOf(buildScene({ ...DEFAULT_PARAMS, kind: 'comet', colors: ['#ff0000', '#ff8000'] }))
  for (const kind of ['marquee', 'starfield', 'gradient', 'breathing', 'wave'] as const) {
    for (const c of lookParams(kind, comet).colors) assert.ok(['#ff0000', '#ff8000'].includes(c), `${kind}: ${c}`)
  }
})
