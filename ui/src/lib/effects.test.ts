import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { COLOR_COUNT, EFFECTS, SHELVES, renderAt, shelfEffects, type EffectSpec } from './effects.ts'

interface Vectors {
  cases: { spec: EffectSpec; samples: { t: number; x: number; y: number; rgb: number[] }[] }[]
}

const vectors: Vectors = JSON.parse(
  readFileSync(
    new URL('../../../crates/freergb-effects/tests/vectors.json', import.meta.url),
    'utf8',
  ),
)

test('the preview reproduces the Rust renderer on every golden sample', () => {
  let n = 0
  for (const { spec, samples } of vectors.cases) {
    for (const { t, x, y, rgb } of samples) {
      const got = renderAt(spec, t, x, y)
      for (let i = 0; i < 3; i++) {
        // 1 tolerance: sin/cos may differ in the last ulp between V8 and Rust.
        assert.ok(
          Math.abs(got[i] - rgb[i]) <= 1,
          `${JSON.stringify(spec)} t=${t} x=${x} y=${y}: js ${got} vs rust ${rgb}`,
        )
      }
      n++
    }
  }
  assert.ok(n > 300, `only ${n} samples checked`)
})

test('the vectors cover every effect type', () => {
  const kinds = new Set(vectors.cases.map((c) => c.spec.type))
  for (const k of [
    'static', 'gradient', 'spectrum_cycle', 'rainbow', 'breathing', 'wave',
    'comet', 'marquee', 'bouncing_beam', 'starfield', 'fire', 'rain',
  ]) {
    assert.ok(kinds.has(k as never), k)
  }
})

import { buildScene, paramsOf, type Scene, type SceneParams, type Targets } from './effects.ts'

const fixtures: { input: SceneParams; targets?: Targets; group?: string; scene: Scene }[] = JSON.parse(
  readFileSync(new URL('./scene-fixtures.json', import.meta.url), 'utf8'),
)

test('the panel builds exactly the scenes in scene-fixtures.json (which the Rust side accepts)', () => {
  assert.equal(fixtures.length, 20)
  for (const { input, targets, group, scene } of fixtures) {
    assert.deepEqual(buildScene(input, targets, group), scene, input.kind)
  }
})

test('the fixtures include scenes that name devices by key', () => {
  const named = fixtures.filter((f) => f.targets && f.targets !== 'all')
  assert.ok(named.length >= 2)
  assert.ok(named.some((f) => (f.targets as { devices: unknown[] }).devices.length > 1))
  assert.ok(fixtures.some((f) => f.scene.group), 'and a group look')
})

test('editing a scene and sending it back changes nothing: paramsOf inverts buildScene', () => {
  for (const { scene } of fixtures) {
    assert.deepEqual(buildScene(paramsOf(scene), scene.targets, scene.group), scene, scene.effect.type)
  }
})

test('every software effect sits on exactly one shelf, and no shelf holds more than six', () => {
  for (const s of SHELVES) assert.ok(shelfEffects(s.id).length <= 6, s.id)
  assert.equal(SHELVES.reduce((n, s) => n + shelfEffects(s.id).length, 0), EFFECTS.length)
  for (const k of Object.keys(COLOR_COUNT)) {
    if (k === 'painted' || k === 'device_mode') continue
    assert.ok(EFFECTS.some((e) => e.id === k), `${k} is offered`)
  }
})
