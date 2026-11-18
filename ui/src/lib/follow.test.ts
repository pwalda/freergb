import { test } from 'node:test'
import assert from 'node:assert/strict'
import { colorSlots } from './effects.ts'
import { drivesFor, firstDrive, readout } from './follow.ts'

test('each source offers only what it can move on the look', () => {
  assert.deepEqual(drivesFor('wave', 'sound'), ['brightness', 'color', 'amount'])
  assert.deepEqual(drivesFor('static', 'cpu_temp'), ['brightness', 'color'])
  assert.deepEqual(drivesFor('rainbow', 'cpu_load'), ['brightness', 'amount'])
  assert.deepEqual(drivesFor('rainbow', 'screen'), ['color'])
  assert.deepEqual(drivesFor('comet', 'spectrum'), ['brightness'])
  assert.equal(firstDrive('static', 'cpu_temp'), 'color')
  assert.equal(firstDrive('rainbow', 'gpu_temp'), 'brightness')
})

test('a reading that picks the colour gets a low and a high colour even on a one-colour look', () => {
  assert.equal(colorSlots({ kind: 'static', react: { source: 'cpu_temp', drives: 'color' } }), 2)
  assert.equal(colorSlots({ kind: 'static', react: { source: 'screen', drives: 'color' } }), 1)
  assert.equal(colorSlots({ kind: 'gradient', react: { source: 'sound', drives: 'color' } }), 3)
  assert.equal(colorSlots({ kind: 'static', react: null }), 1)
})

test('readings read as people read them, and say when there is none yet', () => {
  assert.equal(readout('cpu_temp', { cpu_temp: 61.6 }), '62 °C')
  assert.equal(readout('sound', { sound: 0.37 }), '37%')
  assert.equal(readout('beat', { beat: 0.8 }), '80%')
  assert.equal(firstDrive('wave', 'beat'), 'brightness')
  assert.equal(readout('gpu_load', {}), null)
  assert.equal(readout('screen', { screen: { width: 1, height: 1, colors: ['#000000'] } }), 'Live')
})
