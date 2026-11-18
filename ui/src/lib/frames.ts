// One animation loop for every LED preview on screen, so a hero, six tiles and a list of
// devices cost one requestAnimationFrame callback per frame, and nothing while none is mounted.

type Draw = (seconds: number) => void

const draws = new Set<Draw>()
const t0 = performance.now()
let raf = 0

function tick(now: number) {
  const t = (now - t0) / 1000
  draws.forEach((d) => d(t))
  raf = draws.size > 0 ? requestAnimationFrame(tick) : 0
}

export const reducedMotion = () =>
  typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches

/** Calls `draw(seconds)` every frame until the returned function is called. */
export function onFrame(draw: Draw): () => void {
  draws.add(draw)
  if (!raf) raf = requestAnimationFrame(tick)
  return () => {
    draws.delete(draw)
    if (draws.size === 0 && raf) {
      cancelAnimationFrame(raf)
      raf = 0
    }
  }
}
