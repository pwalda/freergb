<script lang="ts">
  import { tick } from 'svelte'
  import type { Detection, Device, Profiles, ServerState, UpdateInfo } from './api'
  import { scanDetail } from './detection'
  import { deviceKind } from './device-kind'
  import Icon from './Icon.svelte'
  import Logo from './Logo.svelte'
  import MiniStrip from './MiniStrip.svelte'
  import type { Lit } from './preview'
  import { actionLabel, byTime, daysLabel, powerLine, timeText, type Schedule } from './schedule.ts'

  let {
    server,
    statusText,
    statusShort,
    scan,
    scanning,
    onrescan,
    devices,
    names,
    owns,
    inGroup,
    groups,
    selectedGroup,
    allLabel,
    allSample,
    samples,
    selected,
    theme,
    profiles,
    onselect,
    onselectgroup,
    onnewgroup,
    ontheme,
    update,
    onsettings,
    onretry,
    onapply,
    onsaveprofile,
    oneditprofile,
    schedule,
    oneditschedule,
  }: {
    server: ServerState | null
    statusText: string
    statusShort: string
    scan: Detection
    /** Connected and OpenRGB is looking for devices. */
    scanning: boolean
    onrescan: () => void
    devices: Device[]
    names: string[]
    /** Per device: the name of its own look, or null when it follows all devices. */
    owns: (string | null)[]
    /** Per device: the group whose look it follows, if any. */
    inGroup: (string | null)[]
    groups: { name: string; count: number; label: string; own: boolean; sample: ((u: number, t: number) => Lit) | null }[]
    /** The group in view, if any. */
    selectedGroup: string | null
    allLabel: string
    allSample: ((u: number, t: number) => Lit) | null
    samples: ((u: number, t: number) => Lit)[]
    selected: number
    theme: string
    profiles: Profiles
    onselect: (i: number) => void
    onselectgroup: (name: string) => void
    onnewgroup: () => void
    ontheme: (e: MouseEvent) => void
    /** A newer freeRGB is ready: a quiet way into Settings, where it installs. */
    update: UpdateInfo | null
    onsettings: () => void
    onretry: () => void
    onapply: (name: string) => void
    onsaveprofile: () => void
    oneditprofile: (name: string) => void
    schedule: Schedule
    oneditschedule: () => void
  } = $props()

  let list = $state<HTMLDivElement>()
  let litTop = $state(0)
  let litHeight = $state(0)
  let instant = $state(true)

  // The selection is one lit plate that glides to the chosen target.
  $effect(() => {
    void selected
    void selectedGroup
    void devices.length
    void groups.length
    void tick().then(() => {
      const on = list?.querySelector<HTMLElement>('.t.on')
      if (!on) return
      litTop = on.offsetTop
      litHeight = on.offsetHeight
      requestAnimationFrame(() => (instant = false))
    })
  })
  const connected = $derived(server?.kind === 'connected')
  const allOn = $derived(selected < 0 && selectedGroup === null)
  /** The rail shows a few times; the sheet has them all. */
  const SHOWN = 4
  const shownRules = $derived(byTime(schedule.rules).slice(0, SHOWN))
  const moreRules = $derived(Math.max(0, schedule.rules.length - SHOWN))
</script>

<aside class="rail">
  <div class="brand"><Logo /><span class="word">free<b>RGB</b></span></div>
  <div
    class="status"
    class:scanning
    data-kind={server?.kind ?? 'starting'}
    title={scanning && scanDetail(scan) ? `${statusText}\n${scanDetail(scan)}` : statusText}
  >
    <span class="dot"></span>
    <span class="text full" aria-live="polite">{statusText}</span>
    <span class="text short">{statusShort}</span>
    {#if server?.kind === 'failed'}<button class="btn sm" onclick={onretry}>Retry</button>{/if}
    {#if connected}
      <button
        class="ibtn rescan"
        onclick={onrescan}
        disabled={scanning}
        aria-busy={scanning}
        aria-label="Look for devices again"
        title={scanning ? 'Looking for devices…' : 'Look for devices again'}
      >
        <Icon name="rescan" size={14} />
      </button>
    {/if}
    {#if scanning}
      <!-- How far OpenRGB got, and the detector it is running. Sits in the space under the
           status line, so nothing below moves while a scan runs. -->
      <span class="scan" aria-hidden="true">
        <span class="bar" class:unknown={scan.percent === null} style:--p="{scan.percent ?? 0}%"></span>
        <span class="what num">{scanDetail(scan)}</span>
      </span>
    {/if}
  </div>
  <button
    class="ibtn theme"
    onclick={ontheme}
    aria-label={theme === 'dark' ? 'Switch to light theme' : 'Switch to dark theme'}
  >
    <Icon name={theme === 'dark' ? 'sun' : 'moon'} size={15} />
  </button>
  <button class="ibtn settings" onclick={onsettings} aria-label="Settings" title="Settings">
    <Icon name="gear" size={15} />
  </button>
  {#if update}
    <button class="update" onclick={onsettings} title="freeRGB {update.version} is ready to install">
      <span class="ud" aria-hidden="true"></span>Update ready
    </button>
  {/if}

  {#if connected && devices.length > 0}
    <div class="targets-head">
      <h3 id="targets-label">Targets</h3>
      <button class="ibtn add" aria-label="New group of devices" title="New group of devices" onclick={onnewgroup}>
        <Icon name="plus" size={14} />
      </button>
    </div>
    <div class="targets" bind:this={list} role="group" aria-labelledby="targets-label">
      <span class="lit" class:instant style:top="{litTop}px" style:height="{litHeight}px"></span>
      <button class="t" class:on={allOn} aria-pressed={allOn} onclick={() => onselect(-1)}>
        <span class="n">All devices</span>
        <span class="ct num">{devices.length}</span>
        <span class="s">{allLabel}</span>
        <MiniStrip sample={allSample} {theme} />
      </button>
      {#each groups as g (g.name)}
        {@const on = selectedGroup === g.name}
        <button class="t grp" class:on aria-pressed={on} onclick={() => onselectgroup(g.name)}>
          <span class="n">{g.name}</span>
          <span class="ct num">{g.count}</span>
          <span class="s">
            {#if g.own}<span class="own">Group look · {g.label}</span>{:else}Group · {g.label.toLowerCase()}{/if}
          </span>
          <MiniStrip sample={g.sample} {theme} />
        </button>
      {/each}
      {#each devices as d, i (i)}
        <button class="t" class:on={selected === i} aria-pressed={selected === i} onclick={() => onselect(i)}>
          <span class="n">{names[i]}</span>
          <span class="ct num">{d.colors.length}</span>
          <span class="s">
            {#if owns[i]}<span class="own">Own look · {owns[i]}</span>
            {:else if inGroup[i]}In “{inGroup[i]}”
            {:else}{deviceKind(d.device_type).label} · {d.vendor}{/if}
          </span>
          <MiniStrip sample={samples[i]} {theme} />
        </button>
      {/each}
    </div>

    <div class="profiles-head">
      <h3 id="profiles-label">Profiles</h3>
      <button class="ibtn add" aria-label="Save this look as a profile" title="Save this look as a profile" onclick={onsaveprofile}>
        <Icon name="plus" size={14} />
      </button>
    </div>
    <ul class="profiles" aria-labelledby="profiles-label">
      {#each profiles.names as name (name)}
        {@const on = profiles.active === name}
        <li class="p" class:on>
          <button class="pa" aria-current={on ? 'true' : undefined} onclick={() => onapply(name)}>
            <span class="pn">{name}</span>
            {#if on}<span class="own">On</span>{/if}
          </button>
          <button class="ibtn edit" aria-label="Edit profile {name}" title="Edit profile" onclick={() => oneditprofile(name)}>
            <Icon name="pencil" size={13} />
          </button>
        </li>
      {:else}
        <li class="empty">Save a look here to switch back to it later, also from the tray.</li>
      {/each}
    </ul>

    <div class="sched-head">
      <h3 id="schedule-label">Schedule</h3>
      <button class="ibtn add" aria-label="Edit the schedule" title="Edit the schedule" onclick={oneditschedule}>
        <Icon name="pencil" size={13} />
      </button>
    </div>
    <button class="sched" aria-describedby="schedule-label" onclick={oneditschedule}>
      <span class="chipname"><Icon name="clock" size={14} />Schedule</span>
      {#each shownRules as r (r)}
        <span class="rule">
          <span class="num">{timeText(r.at)}</span>
          <span class="sa">{actionLabel(r.action)}</span>
          <span class="sd">{daysLabel(r.days)}</span>
        </span>
      {:else}
        <span class="rule none">No set times</span>
      {/each}
      {#if moreRules > 0}<span class="rule none">and {moreRules} more</span>{/if}
      <span class="pw">{powerLine(schedule)}</span>
    </button>
  {/if}
</aside>

<style>
  .rail {
    grid-area: rail;
    background: var(--rail);
    border-right: 1px solid var(--line);
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    grid-template-rows: auto auto auto minmax(0, 1fr) auto auto auto auto;
    grid-template-areas: 'brand brand' 'status status' 'h3 h3' 'targets targets' 'phead phead' 'profiles profiles' 'shead shead' 'sched sched';
    padding: 16px 12px 14px;
    min-height: 0;
    position: relative;
  }
  .brand {
    grid-area: brand;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 10px;
    font-weight: 650;
    font-size: 16px;
    letter-spacing: -0.025em;
  }
  /* The name in LED dots, like the stage: "free" dimmer, "RGB" fully lit. */
  .word {
    font-family: 'Doto Variable', var(--mono);
    font-size: 21px;
    font-weight: 700;
    letter-spacing: 0.01em;
    color: var(--muted);
  }
  .word b {
    font-weight: 900;
    color: var(--text);
  }
  .brand :global(.mark) {
    font-size: 30px;
  }
  .status {
    grid-area: status;
    margin: 8px 10px 22px;
    color: var(--muted);
    font-size: 12.5px;
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 28px;
    min-width: 0;
  }
  .status .text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .status {
    position: relative;
  }
  .rescan {
    margin-left: auto;
    width: 26px;
    height: 26px;
    flex: none;
    border-color: transparent;
    color: var(--muted);
  }
  .rescan:hover:not(:disabled) {
    color: var(--text);
  }
  .rescan:disabled {
    background: none;
    opacity: 1;
    cursor: progress;
  }
  .scanning .rescan :global(svg) {
    animation: spin 1.1s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .scanning .dot {
    animation: pulse 1.4s ease-in-out infinite;
  }
  .scan {
    position: absolute;
    left: 14px;
    right: 4px;
    top: calc(100% + 1px);
    display: grid;
    gap: 5px;
    animation: in var(--slow) var(--ease) backwards;
  }
  .bar {
    height: 2px;
    border-radius: 1px;
    background: linear-gradient(var(--lamp), var(--lamp)) 0 0 / var(--p) 100% no-repeat, var(--line2);
    transition: background-size var(--slow) var(--ease);
  }
  /* No progress reported yet: a short lamp run sweeps the track. */
  .bar.unknown {
    background: linear-gradient(90deg, transparent, var(--lamp), transparent) -40% 0 / 40% 100% no-repeat, var(--line2);
    animation: sweep 1.3s var(--ease) infinite;
  }
  @keyframes sweep {
    to {
      background-position: 140% 0, 0 0;
    }
  }
  .what {
    font-size: 11px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-height: 1lh;
  }
  @media (prefers-reduced-motion: reduce) {
    .scanning .rescan :global(svg),
    .bar.unknown {
      animation: none;
    }
    .bar.unknown {
      background: var(--line2);
    }
  }
  .short {
    display: none;
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex: none;
    background: var(--muted);
  }
  [data-kind='connected'] .dot {
    background: var(--ok);
  }
  [data-kind='reconnecting'] .dot,
  [data-kind='starting'] .dot {
    background: var(--warn);
    animation: pulse 1.4s ease-in-out infinite;
  }
  [data-kind='failed'] .dot {
    background: var(--bad);
  }
  @keyframes pulse {
    50% {
      opacity: 0.25;
    }
  }
  .theme {
    position: absolute;
    left: 22px;
    bottom: 14px;
    color: var(--muted);
  }
  .settings {
    position: absolute;
    left: 58px;
    bottom: 14px;
    color: var(--muted);
  }
  .theme:hover,
  .settings:hover {
    color: var(--text);
  }
  /* Beside the gear, in the same quiet voice; the lamp dot is what catches the eye. */
  .update {
    position: absolute;
    left: 94px;
    right: 12px;
    bottom: 14px;
    height: 30px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 8px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--muted);
    font: inherit;
    font-size: 12.5px;
    white-space: nowrap;
    overflow: hidden;
    transition:
      background var(--fast),
      color var(--fast);
    animation: in var(--slow) var(--ease) backwards;
  }
  .update:hover {
    color: var(--text);
    background: var(--raise);
  }
  .ud {
    width: 7px;
    height: 7px;
    flex: none;
    border-radius: 50%;
    background: var(--lamp);
  }
  h3 {
    margin: 0 10px 6px;
    font-size: 12px;
    font-weight: 500;
    color: var(--muted);
  }
  .targets {
    grid-area: targets;
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow: auto;
    min-height: 0;
  }
  .targets-head {
    grid-area: h3;
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 2px;
  }
  .targets-head h3 {
    margin-bottom: 0;
  }
  .profiles-head {
    grid-area: phead;
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid var(--line);
  }
  .profiles-head h3 {
    margin-bottom: 0;
  }
  .add {
    width: 26px;
    height: 26px;
    margin-right: 4px;
    border-color: transparent;
    color: var(--muted);
  }
  .add:hover {
    color: var(--text);
  }
  .profiles {
    grid-area: profiles;
    list-style: none;
    margin: 4px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 32vh;
    overflow: auto;
  }
  .p {
    position: relative;
    display: flex;
    align-items: center;
    border-radius: 10px;
    transition: background var(--fast);
  }
  .p:hover {
    background: color-mix(in srgb, var(--raise) 55%, transparent);
  }
  .pa {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    height: 34px;
    padding: 0 10px;
    border: 0;
    border-radius: 10px;
    background: none;
    text-align: left;
    font-size: 13.5px;
    color: var(--text);
  }
  .pn {
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .p .own {
    flex: none;
    font-size: 12px;
  }
  .edit {
    flex: none;
    width: 26px;
    height: 26px;
    margin-right: 4px;
    border-color: transparent;
    color: var(--muted);
    opacity: 0;
    transition:
      opacity var(--fast),
      background var(--fast);
  }
  .p:hover .edit,
  .p:focus-within .edit,
  .edit:focus-visible {
    opacity: 1;
  }
  .edit:hover {
    color: var(--text);
  }
  @media (hover: none) {
    .edit {
      opacity: 1;
    }
  }
  .sched-head {
    grid-area: shead;
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: 12px;
    padding-top: 12px;
    border-top: 1px solid var(--line);
  }
  .sched-head h3 {
    margin-bottom: 0;
  }
  /* One plate for the whole schedule: times as mono readouts, sleep and wake under them. */
  .sched {
    grid-area: sched;
    display: grid;
    gap: 3px;
    margin: 4px 0 44px;
    padding: 7px 10px 8px;
    border: 0;
    border-radius: 10px;
    background: none;
    text-align: left;
    color: var(--text);
    transition: background var(--fast);
  }
  .sched:hover {
    background: color-mix(in srgb, var(--raise) 55%, transparent);
  }
  .chipname {
    display: none;
  }
  .rule {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
    font-size: 13px;
  }
  .rule .num {
    flex: none;
    font-size: 12.5px;
  }
  .sa {
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sd {
    flex: none;
    margin-left: auto;
    font-size: 12px;
    color: var(--muted);
  }
  .rule.none {
    font-size: 12px;
    color: var(--muted);
  }
  .pw {
    margin-top: 3px;
    font-size: 12px;
    color: var(--muted);
    text-wrap: pretty;
  }
  .empty {
    padding: 4px 10px;
    font-size: 12px;
    color: var(--muted);
    text-wrap: pretty;
  }
  .lit {
    position: absolute;
    left: 0;
    right: 0;
    border-radius: 10px;
    background: var(--raise);
    box-shadow:
      inset 0 0 0 1px var(--line),
      0 6px 16px -12px var(--shadow);
    transition:
      top var(--slow) var(--ease),
      height var(--slow) var(--ease);
    pointer-events: none;
  }
  .lit.instant {
    transition: none;
  }
  .t {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 1px 10px;
    padding: 10px;
    border-radius: 10px;
    border: 0;
    background: none;
    text-align: left;
    color: var(--muted);
    transition: background var(--fast);
  }
  .n {
    color: var(--text);
    font-weight: 500;
    font-size: 13.5px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .s {
    grid-column: 1 / -1;
    font-size: 12px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ct {
    font-size: 11.5px;
  }
  .t:not(.on):hover {
    background: color-mix(in srgb, var(--raise) 55%, transparent);
  }
  .t:not(.on) :global(canvas) {
    opacity: 0.6;
  }
  .own {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--text);
  }
  .own::before {
    content: '';
    width: 10px;
    height: 1px;
    background: var(--lamp);
  }
  :global(.boot) .brand,
  :global(.boot) .status,
  :global(.boot) .t {
    animation: in var(--slow) var(--ease) backwards;
  }
  :global(.boot) .t:nth-child(3) {
    animation-delay: 60ms;
  }
  :global(.boot) .t:nth-child(4) {
    animation-delay: 100ms;
  }
  :global(.boot) .t:nth-child(5) {
    animation-delay: 140ms;
  }
  :global(.boot) .t:nth-child(6) {
    animation-delay: 180ms;
  }
  :global(.boot) .t:nth-child(n + 7) {
    animation-delay: 220ms;
  }

  /* Narrow window: the rail becomes a top bar with a row of target chips. */
  @media (max-width: 700px) {
    .rail {
      border-right: 0;
      border-bottom: 1px solid var(--line);
      padding: calc(12px + var(--frame-h)) 12px 10px;
      grid-template-columns: auto minmax(0, 1fr) auto auto;
      grid-template-rows: auto auto auto auto;
      grid-template-areas: 'brand status settings theme' 'targets targets targets targets' 'profiles profiles profiles profiles' 'sched sched sched sched';
      column-gap: 10px;
      align-items: center;
    }
    .brand {
      padding: 0 4px;
    }
    .status {
      margin: 0;
      justify-content: flex-end;
    }
    .full {
      display: none;
    }
    .scan {
      display: none;
    }
    .short {
      display: inline;
    }
    .theme {
      position: static;
      grid-area: theme;
    }
    .settings {
      position: static;
      grid-area: settings;
    }
    h3,
    .targets-head {
      display: none;
    }
    .targets {
      flex-direction: row;
      overflow-x: auto;
      margin: 10px 0 0;
      scroll-snap-type: x mandatory;
      scrollbar-width: none;
    }
    .t {
      flex: 0 0 150px;
      scroll-snap-align: start;
    }
    .lit {
      display: none;
    }
    .t.on {
      background: var(--raise);
      box-shadow: inset 0 0 0 1px var(--line);
    }
    .profiles-head,
    .sched-head,
    .rule,
    .pw {
      display: none;
    }
    .sched {
      justify-self: start;
      margin: 8px 0 0;
      padding: 0 10px;
      height: 28px;
      border: 1px solid var(--line2);
      border-radius: 7px;
    }
    .chipname {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      font-size: 12.5px;
      white-space: nowrap;
    }
    .chipname :global(svg) {
      color: var(--muted);
    }
    .profiles {
      flex-direction: row;
      max-height: none;
      overflow-x: auto;
      margin: 8px 0 0;
      scrollbar-width: none;
    }
    .p {
      flex: none;
      border: 1px solid var(--line2);
      border-radius: 7px;
    }
    .pa {
      height: 28px;
      font-size: 12.5px;
    }
    .edit,
    .empty {
      display: none;
    }
  }
</style>
