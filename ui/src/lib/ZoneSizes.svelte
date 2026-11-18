<script lang="ts">
  import { resizeZone, type Device } from './api'

  let { device, onerror }: { device: Device; onerror: (message: string) => void } = $props()

  const zones = $derived(device.zones.filter((z) => z.resizable))
  /** What the user has dragged to and not yet seen confirmed by the device. */
  let wanted = $state<Record<number, number>>({})
  const timers: Record<number, ReturnType<typeof setTimeout>> = {}

  // Once the device reports the size we asked for, show its number again.
  $effect(() => {
    for (const z of zones) if (wanted[z.index] === z.leds) delete wanted[z.index]
  })
  $effect(() => () => Object.values(timers).forEach(clearTimeout))

  const shown = (z: { index: number; leds: number }) => wanted[z.index] ?? z.leds

  function change(index: number, leds: number, size: number) {
    wanted[index] = size
    clearTimeout(timers[index])
    timers[index] = setTimeout(async () => {
      if (size === leds) return void delete wanted[index]
      try {
        await resizeZone(device.id, index, size)
      } catch (e) {
        delete wanted[index]
        onerror(String(e))
      }
    }, 300)
  }
</script>

{#if zones.length > 0}
  <div class="zones wide">
    {#each zones as z (z.index)}
      <label class="field">
        <span class="lbl">Zone size · {z.name} <b class="num">{shown(z)} LEDs</b></span>
        <input
          type="range"
          min={z.min_leds}
          max={z.max_leds}
          step="1"
          value={shown(z)}
          oninput={(e) => change(z.index, z.leds, e.currentTarget.valueAsNumber)}
          style:--fill="{((shown(z) - z.min_leds) / (z.max_leds - z.min_leds)) * 100}%"
        />
      </label>
    {/each}
  </div>
{/if}

<style>
  .zones {
    display: grid;
    grid-template-columns: var(--ctl-cols, repeat(3, minmax(0, 1fr)));
    gap: 18px 28px;
  }
</style>
