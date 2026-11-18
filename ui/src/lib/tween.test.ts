import { test } from 'node:test'
import assert from 'node:assert/strict'
import { Tweens } from './tween.ts'

test('a tween eases out from its current value to the target and stays there', () => {
  let now = 0
  const t = new Tweens(() => now)
  t.set('p', 0)
  t.to('p', 10, 100)
  assert.equal(t.get('p'), 0)
  now = 50
  assert.ok(t.get('p') > 5, 'ease-out: past half way at half time')
  now = 100
  assert.equal(t.get('p'), 10)
  now = 500
  assert.equal(t.get('p'), 10)
})

test('retargeting mid-way starts from where it is; repeating the target changes nothing', () => {
  let now = 0
  const t = new Tweens(() => now)
  t.set('p', 0)
  t.to('p', 10, 100)
  now = 50
  const mid = t.get('p')
  t.to('p', 10, 100)
  assert.equal(t.get('p'), mid)
  t.to('p', 0, 100)
  assert.equal(t.get('p'), mid)
  assert.equal(t.get('missing', 3), 3)
})

test('under reduced motion a tween jumps straight to its target', () => {
  const g = globalThis as { matchMedia?: unknown }
  const was = g.matchMedia
  g.matchMedia = () => ({ matches: true })
  try {
    const t = new Tweens(() => 0)
    t.set('p', 0)
    t.to('p', 10, 100)
    assert.equal(t.get('p'), 10)
  } finally {
    g.matchMedia = was
  }
})
