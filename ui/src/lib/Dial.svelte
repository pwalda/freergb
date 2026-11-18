<script lang="ts">
  let {
    value,
    label,
    onchange,
  }: {
    /** Degrees 0..359: 0 points right, 90 down. */
    value: number
    label: string
    onchange: (deg: number) => void
  } = $props()

  const STEP = 15
  const NAMES = ['right', 'down and right', 'down', 'down and left', 'left', 'up and left', 'up', 'up and right']
  const norm = (d: number) => ((Math.round(d / STEP) * STEP) % 360 + 360) % 360
  const named = (d: number) => (d % 45 === 0 ? `, ${NAMES[d / 45]}` : '')

  let el = $state<HTMLDivElement>()
  let dragging = $state(false)

  function at(e: PointerEvent) {
    const r = el!.getBoundingClientRect()
    const deg = (Math.atan2(e.clientY - (r.top + r.height / 2), e.clientX - (r.left + r.width / 2)) * 180) / Math.PI
    const v = norm(deg)
    if (v !== value) onchange(v)
  }
  function key(e: KeyboardEvent) {
    const d: Record<string, number> = { ArrowRight: STEP, ArrowUp: STEP, ArrowLeft: -STEP, ArrowDown: -STEP, PageUp: 45, PageDown: -45 }
    let v: number | undefined
    if (e.key in d) v = norm(value + d[e.key])
    else if (e.key === 'Home') v = 0
    else if (e.key === 'End') v = 270
    if (v === undefined) return
    e.preventDefault()
    if (v !== value) onchange(v)
  }
  const rad = $derived((value * Math.PI) / 180)
</script>

<div
  bind:this={el}
  class="dial"
  class:dragging
  role="slider"
  tabindex="0"
  aria-label={label}
  aria-valuemin="0"
  aria-valuemax="345"
  aria-valuenow={value}
  aria-valuetext={`${value} degrees${named(value)}`}
  style:--x={`${Math.cos(rad) * 15}px`}
  style:--y={`${Math.sin(rad) * 15}px`}
  style:--a={`${value}deg`}
  onkeydown={key}
  onpointerdown={(e) => {
    dragging = true
    el!.setPointerCapture(e.pointerId)
    at(e)
  }}
  onpointermove={(e) => dragging && at(e)}
  onpointerup={() => (dragging = false)}
  onpointercancel={() => (dragging = false)}
>
  <span class="needle" aria-hidden="true"></span>
  <span class="knob" aria-hidden="true"></span>
</div>

<style>
  /* A ring the size of two knobs, a lamp needle from the centre and the slider's knob riding the
     ring: the slider's own parts bent round. */
  .dial {
    position: relative;
    flex: none;
    width: 46px;
    height: 46px;
    border-radius: 50%;
    box-shadow: inset 0 0 0 4px var(--line2);
    cursor: pointer;
    touch-action: none;
  }
  .needle {
    position: absolute;
    left: 50%;
    top: 50%;
    width: 15px;
    height: 2px;
    margin-top: -1px;
    border-radius: 2px;
    background: var(--lamp);
    transform-origin: 0 50%;
    transform: rotate(var(--a));
  }
  .knob {
    position: absolute;
    left: 50%;
    top: 50%;
    width: 16px;
    height: 16px;
    margin: -8px 0 0 -8px;
    border-radius: 50%;
    background: var(--text);
    box-shadow: 0 2px 6px var(--shadow);
    transform: translate(var(--x), var(--y));
    transition: scale var(--fast) var(--ease);
  }
  .dial:hover .knob {
    scale: 1.1;
  }
  .dial.dragging .knob {
    scale: 1.25;
  }
</style>
