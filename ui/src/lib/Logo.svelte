<script lang="ts">
  // The mark: a lowercase f set in the app's own LED dots. The lit dots drift through the hues
  // and a pulse runs along the stroke, top to bottom.
  const F = ['..XX.', '.X..X', '.X...', 'XXX..', '.X...', '.X...', '.X...']
  const dots: { x: number; y: number; on: number }[] = []
  let lit = 0
  F.forEach((row, y) => [...row].forEach((c, x) => dots.push({ x: 8 + x * 12, y: 4 + y * 12, on: c === 'X' ? lit++ : -1 })))
</script>

<svg class="mark" viewBox="0 0 64 84" aria-hidden="true">
  {#each dots as d}
    {#if d.on >= 0}
      <circle class="on" cx={d.x} cy={d.y} r="4.6" style:--i={d.on} style:--h={(d.on * 24 + 340) % 360} />
    {:else}
      <circle cx={d.x} cy={d.y} r="4.6" />
    {/if}
  {/each}
</svg>

<style>
  @property --hh {
    syntax: '<number>';
    inherits: false;
    initial-value: 0;
  }
  .mark {
    display: block;
    height: 1em;
    width: 0.8em;
    flex: none;
  }
  circle {
    fill: var(--line2);
  }
  .on {
    fill: hsl(calc(var(--h) + var(--hh)) 85% 60%);
    animation:
      hue 8s linear infinite,
      chase 2.6s ease-in-out infinite;
    animation-delay: 0s, calc(var(--i) * 110ms);
  }
  :global([data-theme='light']) .on {
    fill: hsl(calc(var(--h) + var(--hh)) 78% 50%);
  }
  @keyframes hue {
    to {
      --hh: 360;
    }
  }
  @keyframes chase {
    0%,
    30%,
    100% {
      opacity: 0.5;
    }
    12% {
      opacity: 1;
    }
  }
</style>
