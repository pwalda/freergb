<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import Icon from './Icon.svelte'
  import {
    actionOf,
    actionValue,
    byTime,
    clock,
    timeText,
    DAY_NAMES,
    EVERY_DAY,
    MAX_RULES,
    minutes,
    type Action,
    type Schedule,
  } from './schedule.ts'

  let {
    schedule,
    profiles,
    onsave,
    onclose,
  }: {
    schedule: Schedule
    /** Profile names, for the actions. */
    profiles: string[]
    onsave: (s: Schedule) => Promise<void>
    onclose: () => void
  } = $props()

  /** A copy to edit; Save keeps it. Each rule gets a key so rows keep focus while sorting. */
  const start = untrack(() => schedule)
  let n = 0
  let rules = $state(byTime(start.rules).map((r) => ({ ...r, key: n++ })))
  let sleep = $state<Action>(start.sleep)
  let wake = $state<Action>(start.wake)
  let shutdown = $state<Action>(start.shutdown)
  let busy = $state(false)
  let error = $state('')

  const DAY_LETTER = ['M', 'T', 'W', 'T', 'F', 'S', 'S']
  /** A profile that no longer exists still shows, so the select never lies about the value. */
  const names = (a: Action) => (a.kind === 'profile' && !profiles.includes(a.name) ? [...profiles, a.name] : profiles)
  const noDays = $derived(rules.some((r) => (r.days & EVERY_DAY) === 0))

  let dialog: HTMLDialogElement
  onMount(() => dialog.showModal())
  function close() {
    dialog.close()
    onclose()
  }
  function add() {
    // After the last time, an hour on; the first new one is the usual "lights out".
    const last = rules.at(-1)
    const at = last ? (last.at + 60) % (24 * 60) : 23 * 60
    rules.push({ at, days: EVERY_DAY, action: { kind: 'off' }, key: n++ })
  }
  function setTime(i: number, v: string) {
    const m = minutes(v)
    if (m !== null) rules[i].at = m
  }
  async function save(e: SubmitEvent) {
    e.preventDefault()
    if (busy || noDays) return
    busy = true
    error = ''
    try {
      await onsave({ rules: byTime(rules).map(({ at, days, action }) => ({ at, days, action })), sleep, wake, shutdown })
      close()
    } catch (err) {
      error = String(err)
    } finally {
      busy = false
    }
  }
</script>

{#snippet chevron()}
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m6 9 6 6 6-6" /></svg>
{/snippet}

<dialog
  class="sheet panel"
  bind:this={dialog}
  aria-labelledby="schedule-title"
  oncancel={(e) => {
    e.preventDefault()
    close()
  }}
>
  <form onsubmit={save}>
    <header>
      <div>
        <h2 id="schedule-title">Schedule</h2>
        <p>freeRGB follows it while it runs, also from the tray. A time missed while it was closed does not fire later.</p>
      </div>
      <button type="button" class="ibtn" aria-label="Close" onclick={close}><Icon name="close" /></button>
    </header>

    <div class="body">
      <section aria-labelledby="times-label">
        <h3 class="name" id="times-label">At set times</h3>
        <ul class="rules">
          {#each rules as r, i (r.key)}
            <li>
              <div class="line">
                <input
                  class="time num"
                  type="time"
                  required
                  value={clock(r.at)}
                  aria-label="Time"
                  onchange={(e) => setTime(i, (e.currentTarget as HTMLInputElement).value)}
                />
                <span class="select">
                  <select
                    aria-label="At {timeText(r.at)}"
                    value={actionValue(r.action)}
                    onchange={(e) => (r.action = actionOf((e.currentTarget as HTMLSelectElement).value))}
                  >
                    <option value="off">Lights off</option>
                    {#if names(r.action).length > 0}
                      <optgroup label="Apply a profile">
                        {#each names(r.action) as p (p)}<option value="p:{p}">{p}</option>{/each}
                      </optgroup>
                    {/if}
                  </select>
                  {@render chevron()}
                </span>
                <button type="button" class="ibtn quiet" aria-label="Remove {timeText(r.at)}" title="Remove" onclick={() => rules.splice(i, 1)}>
                  <Icon name="close" size={14} />
                </button>
              </div>
              <div class="days" role="group" aria-label="Days for {timeText(r.at)}">
                {#each DAY_LETTER as letter, d (d)}
                  <button
                    type="button"
                    class="chip day"
                    aria-pressed={(r.days & (1 << d)) !== 0}
                    aria-label={DAY_NAMES[d]}
                    title={DAY_NAMES[d]}
                    onclick={() => (r.days ^= 1 << d)}>{letter}</button
                  >
                {/each}
                {#if (r.days & EVERY_DAY) === 0}<span class="warn">Pick a day</span>{/if}
              </div>
            </li>
          {:else}
            <li class="empty">No times yet. Add one to turn the lights off at night or switch profile in the morning.</li>
          {/each}
        </ul>
        <button type="button" class="btn sm" disabled={rules.length >= MAX_RULES} onclick={add}>
          <Icon name="plus" size={14} />Add a time
        </button>
      </section>

      <label class="row">
        <span>
          <span class="name">When the PC sleeps</span>
          <span class="desc">Happens just before it sleeps.</span>
        </span>
        <span class="select">
          <select value={actionValue(sleep)} onchange={(e) => (sleep = actionOf((e.currentTarget as HTMLSelectElement).value))}>
            <option value="off">Lights off</option>
            <option value="nothing">Nothing</option>
            {#if names(sleep).length > 0}
              <optgroup label="Apply a profile">
                {#each names(sleep) as p (p)}<option value="p:{p}">{p}</option>{/each}
              </optgroup>
            {/if}
          </select>
          {@render chevron()}
        </span>
      </label>
      <label class="row">
        <span>
          <span class="name">When it wakes</span>
          <span class="desc">The look from before is what showed before it slept.</span>
        </span>
        <span class="select">
          <select value={actionValue(wake)} onchange={(e) => (wake = actionOf((e.currentTarget as HTMLSelectElement).value))}>
            <option value="back">Look from before</option>
            <option value="nothing">Nothing</option>
            {#if names(wake).length > 0}
              <optgroup label="Apply a profile">
                {#each names(wake) as p (p)}<option value="p:{p}">{p}</option>{/each}
              </optgroup>
            {/if}
          </select>
          {@render chevron()}
        </span>
      </label>
      <label class="row">
        <span>
          <span class="name">When it shuts down</span>
          <span class="desc">Also restart and sign-out. Left on, devices use their own modes.</span>
        </span>
        <span class="select">
          <select value={actionValue(shutdown)} onchange={(e) => (shutdown = actionOf((e.currentTarget as HTMLSelectElement).value))}>
            <option value="off">Lights off</option>
            <option value="nothing">Leave them on</option>
          </select>
          {@render chevron()}
        </span>
      </label>
      {#if error}<p class="error" role="alert">{error}</p>{/if}
    </div>

    <footer>
      <span class="grow"></span>
      <button type="button" class="btn" onclick={close}>Cancel</button>
      <button class="btn" disabled={busy || noDays}>Save</button>
    </footer>
  </form>
</dialog>

<style>
  .sheet {
    position: fixed;
    inset: 0;
    margin: auto;
    width: min(440px, calc(100vw - 32px));
    max-height: calc(100vh - 32px);
    padding: 0;
    border: 0;
    color: var(--text);
  }
  .sheet:not([open]) {
    display: none;
  }
  .sheet::backdrop {
    background: var(--scrim);
  }
  form {
    display: flex;
    flex-direction: column;
    max-height: calc(100vh - 32px);
  }
  header,
  footer {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 0 22px;
  }
  header {
    justify-content: space-between;
    align-items: flex-start;
    padding-top: 20px;
  }
  header .ibtn {
    flex: none;
  }
  h2 {
    font-size: 20px;
    font-weight: 600;
    letter-spacing: -0.012em;
  }
  header p {
    font-size: 13px;
    color: var(--muted);
    text-wrap: pretty;
  }
  .body {
    display: grid;
    gap: 18px;
    padding: 18px 22px 18px;
    overflow: auto;
    min-height: 0;
  }
  .name {
    display: block;
    font-size: 13.5px;
    font-weight: 500;
  }
  .desc {
    display: block;
    font-size: 12.5px;
    color: var(--muted);
    text-wrap: pretty;
  }
  .rules {
    list-style: none;
    margin: 8px 0 10px;
    padding: 0;
  }
  .rules li {
    display: grid;
    gap: 8px;
    padding: 10px 0;
    border-top: 1px solid var(--line);
  }
  .rules li:last-child {
    border-bottom: 1px solid var(--line);
  }
  .rules .empty {
    font-size: 12.5px;
    color: var(--muted);
    text-wrap: pretty;
  }
  .line {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .line .select {
    flex: 1;
  }
  .time {
    flex: none;
    /* Room for a 12-hour clock's AM/PM; 24-hour locales leave a little air. */
    width: 124px;
    height: 32px;
    padding: 0 8px;
    border: 1px solid var(--line2);
    border-radius: 8px;
    background: var(--ground);
    color: var(--text);
    font-size: 14px;
    color-scheme: inherit;
  }
  .time:hover {
    border-color: var(--muted);
  }
  .time::-webkit-calendar-picker-indicator {
    opacity: 0.55;
  }
  .quiet {
    flex: none;
    width: 28px;
    height: 28px;
    border-color: transparent;
    color: var(--muted);
  }
  .quiet:hover {
    color: var(--text);
  }
  .days {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .day {
    width: 30px;
    height: 26px;
    padding: 0;
    font-size: 12px;
  }
  .warn {
    margin-left: 6px;
    font-size: 12px;
    color: var(--text);
  }
  .btn :global(svg) {
    margin-left: -2px;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 18px;
  }
  .row .select {
    flex: none;
    width: 168px;
  }
  footer {
    padding-top: 14px;
    padding-bottom: 18px;
    border-top: 1px solid var(--line);
  }
  .grow {
    flex: 1;
  }
</style>
