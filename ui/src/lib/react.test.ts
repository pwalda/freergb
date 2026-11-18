import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import type { EffectSpec } from './effects.ts'
import { renderLed, type Readings } from './react.ts'

interface Case {
  spec: EffectSpec
  readings: Readings
  samples: { t: number; x: number; desk: [number, number]; rgb: [number, number, number] }[]
}

test('looks that follow a reading reproduce react.rs on every golden sample', () => {
  const { cases }: { cases: Case[] } = JSON.parse(
    readFileSync(new URL('../../../crates/freergb-effects/tests/react_vectors.json', import.meta.url), 'utf8'),
  )
  const sources = new Set(cases.map((c) => c.spec.react?.source))
  for (const s of ['sound', 'cpu_load', 'cpu_temp', 'spectrum', 'screen']) assert.ok(sources.has(s as never), s)
  let n = 0
  for (const c of cases)
    for (const { t, x, desk, rgb } of c.samples) {
      const got = renderLed(c.spec, c.readings, t, x, 0, desk)
      for (let i = 0; i < 3; i++)
        assert.ok(Math.abs(got[i] - rgb[i]) <= 1, `${JSON.stringify(c.spec)} t=${t} x=${x} desk=${desk}: js ${got} vs rust ${rgb}`)
      n++
    }
  assert.ok(n > 1000, `only ${n} samples`)
})
