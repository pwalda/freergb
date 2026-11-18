import { test } from 'node:test'
import assert from 'node:assert/strict'
import { buildScene, DEFAULT_PARAMS, type Scene } from './effects.ts'
import { SceneStore } from './scene-store.ts'

const scene = (kind: Scene['effect']['type']): Scene => buildScene({ ...DEFAULT_PARAMS, kind })

/** A store whose "backend" resolves each send only when the test says so. */
function rig(initial: Scene[] = []) {
  const sent: Scene[][] = []
  const gates: Array<{ ok: () => void; fail: (e: string) => void }> = []
  const errors: string[] = []
  let backend = initial
  const store = new SceneStore(
    (s) =>
      new Promise<void>((resolve, reject) => {
        sent.push(s)
        gates.push({
          ok: () => {
            backend = s
            resolve()
          },
          fail: (e) => reject(new Error(e)),
        })
      }),
    async () => backend,
    () => {},
    (m) => errors.push(m),
  )
  store.external(initial)
  return { store, sent, gates, errors, setBackend: (s: Scene[]) => (backend = s) }
}
const tick = () => new Promise((r) => setTimeout(r, 0))

test('an edit shows at once and is sent in full', async () => {
  const { store, sent, gates } = rig()
  const done = store.edit(() => [scene('wave')])
  assert.equal(store.scenes[0].effect.type, 'wave')
  await tick()
  assert.equal(sent.length, 1)
  gates[0].ok()
  await done
})

test('two quick edits are sent in order and the second keeps the first', async () => {
  const { store, sent, gates } = rig()
  const first = store.edit((s) => [...s, scene('wave')])
  const second = store.edit((s) => [...s, scene('static')])
  await tick()
  assert.equal(sent.length, 1, 'the second waits for the first')
  gates[0].ok()
  await tick()
  gates[1].ok()
  await Promise.all([first, second])
  assert.deepEqual(
    sent.map((s) => s.map((x) => x.effect.type)),
    [['wave'], ['wave', 'static']],
  )
})

test('a stale echo from the backend while edits are in flight does not undo them', async () => {
  const { store, gates } = rig()
  const done = store.edit(() => [scene('wave')])
  store.external([]) // the event for something older arrives mid-flight
  assert.equal(store.scenes.length, 1)
  await tick()
  gates[0].ok()
  await done
  store.external([scene('static')]) // idle again: a change from outside is taken
  assert.equal(store.scenes[0].effect.type, 'static')
})

test('a rejected edit is reported and the backend’s list replaces the optimistic one', async () => {
  const { store, gates, errors } = rig([scene('rainbow')])
  const done = store.edit(() => [scene('wave')])
  await tick()
  gates[0].fail('brightness must be between 0 and 1')
  await done
  assert.match(errors[0], /brightness/)
  assert.equal(store.scenes[0].effect.type, 'rainbow')
})

test('after a failure the following edit still goes out', async () => {
  const { store, sent, gates } = rig()
  const a = store.edit(() => [scene('wave')])
  const b = store.edit((s) => [...s, scene('static')])
  await tick()
  gates[0].fail('nope')
  await tick()
  gates[1].ok()
  await Promise.all([a, b])
  assert.equal(sent.length, 2)
})

test('undo is an edit that sends the earlier list back, after the change it undoes', async () => {
  const before = [scene('wave'), scene('static')]
  const { store, sent, gates } = rig(before)
  const prev = store.scenes
  const replaced = store.edit(() => [scene('rainbow')])
  const undone = store.edit(() => prev)
  assert.deepEqual(store.scenes, before, 'shown at once')
  await tick()
  gates[0].ok()
  await tick()
  gates[1].ok()
  await Promise.all([replaced, undone])
  assert.deepEqual(sent, [[scene('rainbow')], before])
})

test('idle waits for every edit in flight', async () => {
  const { store, gates } = rig()
  void store.edit(() => [scene('wave')])
  void store.edit(() => [scene('static')])
  let idle = false
  void store.idle().then(() => (idle = true))
  await tick()
  gates[0].ok()
  await tick()
  assert.equal(idle, false)
  gates[1].ok()
  await tick()
  await tick()
  assert.equal(idle, true)
})
