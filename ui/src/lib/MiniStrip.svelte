<script lang="ts">
  import { onMount } from 'svelte'
  import { onFrame, reducedMotion } from './frames'
  import type { Lit } from './preview'
  import { fit } from './vial'

  /** `sample(u, t)`: the colour at position u (0..1) along the strip; null draws unlit dots. */
  let { sample, theme }: { sample: ((u: number, t: number) => Lit) | null; theme: string } = $props()

  let canvas = $state<HTMLCanvasElement>()
  let off = [34, 32, 29]
  $effect(() => {
    void theme
    const m = /^#([0-9a-f]{6})$/i.exec(getComputedStyle(document.documentElement).getPropertyValue('--dot-off').trim())
    if (m) off = [0, 2, 4].map((i) => parseInt(m[1].slice(i, i + 2), 16))
  })

  onMount(() =>
    onFrame((seconds) => {
      const box = canvas && fit(canvas)
      if (!box) return
      const [g, W, H] = box
      const t = reducedMotion() ? 0 : seconds
      g.clearRect(0, 0, W, H)
      const n = Math.max(2, Math.floor(W / 9))
      for (let i = 0; i < n; i++) {
        const c = sample ? sample(i / (n - 1), t) : null
        const a = c ? Math.min(1, c[3]) : 0
        const k = c ? [off[0] + (c[0] - off[0]) * a, off[1] + (c[1] - off[1]) * a, off[2] + (c[2] - off[2]) * a] : off
        g.fillStyle = `rgb(${k[0] | 0},${k[1] | 0},${k[2] | 0})`
        g.beginPath()
        g.arc(i * 9 + 4.5, H / 2, 2.5 + a, 0, 7)
        g.fill()
      }
    }),
  )
</script>

<canvas bind:this={canvas} aria-hidden="true"></canvas>

<style>
  canvas {
    grid-column: 1 / -1;
    width: 100%;
    height: 8px;
    margin-top: 7px;
    transition: opacity var(--mid);
  }
</style>
