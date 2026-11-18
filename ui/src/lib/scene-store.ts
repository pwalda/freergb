// The UI's copy of the backend scene list, with edits applied in order.
//
// Edits are optimistic (the UI updates at once) and sent one at a time, each carrying the
// whole list, so two quick edits on different cards cannot overwrite each other. Backend
// "scenes" events are ignored while our own edits are in flight (they would be stale echoes);
// after a failed edit the backend's list replaces ours.

import type { Scene } from './effects.ts'

export class SceneStore {
  scenes: Scene[] = []
  private inFlight = 0
  private failed = false
  private chain: Promise<void> = Promise.resolve()

  private send: (scenes: Scene[]) => Promise<void>
  private reload: () => Promise<Scene[]>
  private changed: (scenes: Scene[]) => void
  private onError: (message: string) => void

  constructor(
    send: (scenes: Scene[]) => Promise<void>,
    reload: () => Promise<Scene[]>,
    changed: (scenes: Scene[]) => void,
    onError: (message: string) => void,
  ) {
    this.send = send
    this.reload = reload
    this.changed = changed
    this.onError = onError
  }

  private set(scenes: Scene[]) {
    this.scenes = scenes
    this.changed(scenes)
  }

  /** The backend's list: the initial load, or an event (a change from outside this UI). */
  external(scenes: Scene[]) {
    if (this.inFlight === 0) this.set(scenes)
  }

  /** Resolves once every edit made so far has been sent, so the backend's list is ours. */
  idle(): Promise<void> {
    return this.chain
  }

  /** Resolves when this edit (and every earlier one) has been sent. */
  edit(fn: (scenes: Scene[]) => Scene[]): Promise<void> {
    const next = fn(this.scenes)
    this.set(next)
    this.inFlight++
    this.chain = this.chain.then(async () => {
      try {
        await this.send(next)
      } catch (e) {
        this.failed = true
        this.onError(String(e))
      } finally {
        this.inFlight--
      }
      if (this.inFlight === 0 && this.failed) {
        this.failed = false
        try {
          this.set(await this.reload())
        } catch {
          /* keep what we have; the next event corrects it */
        }
      }
    })
    return this.chain
  }
}
