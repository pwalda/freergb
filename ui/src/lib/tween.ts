// Named values that glide to a new target with an ease-out quart; read them every frame.
// Under reduced motion they jump straight to the target.

// Read without DOM types: this module is also type-checked and tested under Node.
const reducedMotion = () => {
  const mm = (globalThis as { matchMedia?: (q: string) => { matches: boolean } }).matchMedia
  return typeof mm === 'function' && mm('(prefers-reduced-motion: reduce)').matches
}

const ease = (x: number) => 1 - Math.pow(1 - x, 4)

interface Tw {
  a: number
  b: number
  t0: number
  d: number
}

export class Tweens {
  private m = new Map<string, Tw>()
  private clock: () => number
  constructor(clock: () => number = () => performance.now()) {
    this.clock = clock
  }
  set(k: string, v: number) {
    this.m.set(k, { a: v, b: v, t0: 0, d: 1 })
  }
  /** Glide from wherever `k` is now to `v` over `ms`; a repeat of the current target is ignored. */
  to(k: string, v: number, ms: number) {
    const w = this.m.get(k)
    if (!w || reducedMotion()) return this.set(k, v)
    if (w.b === v) return
    this.m.set(k, { a: this.get(k), b: v, t0: this.clock(), d: Math.max(ms, 1) })
  }
  get(k: string, fallback = 0): number {
    const w = this.m.get(k)
    if (!w) return fallback
    const p = Math.min(1, (this.clock() - w.t0) / w.d)
    return w.a + (w.b - w.a) * ease(p)
  }
  has(k: string) {
    return this.m.has(k)
  }
}
