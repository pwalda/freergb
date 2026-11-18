<script lang="ts">
  import { onMount } from 'svelte'
  import Icon from './Icon.svelte'
  import Logo from './Logo.svelte'
  import { about, openLink, type About, type LinkId } from './api'
  // Shipped inside the app, so the licences travel with every copy, online or not.
  import licence from '../../../LICENSE?raw'
  import notices from '../../../THIRD-PARTY-NOTICES.md?raw'

  let { onclose }: { onclose: () => void } = $props()

  let info = $state<About | null>(null)
  type Text = 'licence' | 'notices' | 'libraries'
  let text = $state<Text | null>(null)
  /** Every library's licence, written by the build (ui/licenses.ts); fetched when first shown. */
  let libraries = $state('')
  let error = $state('')

  async function open(id: LinkId) {
    error = ''
    try {
      await openLink(id)
    } catch (e) {
      error = String(e)
    }
  }
  async function toggle(t: Text) {
    text = text === t ? null : t
    if (t === 'libraries' && !libraries) {
      try {
        const r = await fetch('licenses.txt')
        libraries = r.ok ? await r.text() : 'The list of libraries comes with release builds.'
      } catch {
        libraries = 'The list of libraries comes with release builds.'
      }
    }
  }
  const LABEL: Record<Text, string> = {
    licence: 'GNU GPL text',
    notices: 'Third-party notices',
    libraries: 'Open-source libraries',
  }

  let dialog: HTMLDialogElement
  onMount(() => {
    dialog.showModal()
    about().then(
      (a) => (info = a),
      (e) => (error = String(e)),
    )
  })
  function close() {
    dialog.close()
    onclose()
  }
</script>

{#snippet out(id: LinkId, label: string)}
  <button class="link" onclick={() => open(id)}>{label}<Icon name="out" size={12} /></button>
{/snippet}

<dialog
  class="sheet panel"
  bind:this={dialog}
  aria-labelledby="about-title"
  oncancel={(e) => {
    e.preventDefault()
    close()
  }}
>
  <header>
    <div class="brand">
      <Logo />
      <h2 id="about-title" class="word" aria-label="freeRGB">free<b>RGB</b></h2>
      {#if info}<span class="ver num">{info.version}</span>{/if}
    </div>
    <button class="ibtn" aria-label="Close" onclick={close}><Icon name="close" /></button>
  </header>

  <div class="body">
    <p class="lead">
      A front-end for OpenRGB. Every device freeRGB lights is supported by OpenRGB and the people who build it. freeRGB is made
      independently and is not part of the OpenRGB project.
    </p>
    <div class="links">
      {@render out('source', 'Source code')}
      {@render out('issues', 'Report a problem')}
      {@render out('openrgb', 'OpenRGB')}
    </div>

    <div class="give">
      <p>No ads, no tracking, no paid features. If freeRGB is useful to you, you can support its development. Donations are gifts: everyone gets the same app.</p>
      <div class="acts">
        <button class="btn sm" onclick={() => open('sponsor')}>GitHub Sponsors</button>
        <button class="btn sm" onclick={() => open('kofi')}>Ko-fi</button>
      </div>
    </div>

    <section aria-labelledby="licence-title">
      <h3 id="licence-title">Licence</h3>
      <p>
        Free software under the GNU General Public License, version 2 or later. You may share and change it under its terms. It
        comes with no warranty.
      </p>
      {#if info?.bundled_openrgb}
        <p>
          Bundled: OpenRGB {info.bundled_openrgb}, unmodified, under the same licence. Its source is attached to every freeRGB
          release.
        </p>
        <div class="links">
          {@render out('openrgb-source', `OpenRGB ${info.bundled_openrgb} source`)}
          {@render out('releases', 'Releases')}
        </div>
      {/if}
      <div class="texts">
        {#each Object.entries(LABEL) as [t, label] (t)}
          <button class="btn sm" aria-expanded={text === t} onclick={() => toggle(t as Text)}>{label}</button>
        {/each}
      </div>
      {#if text}
        <!-- Focusable so the long text scrolls from the keyboard. -->
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
        <pre class="well" tabindex="0" aria-label={LABEL[text]}>{text === 'licence' ? licence : text === 'notices' ? notices : libraries || 'Loading…'}</pre>
      {/if}
    </section>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
  </div>
</dialog>

<style>
  .sheet {
    position: fixed;
    inset: 0;
    margin: auto;
    width: min(520px, calc(100vw - 32px));
    max-height: calc(100vh - 32px);
    padding: 0;
    border: 0;
    color: var(--text);
    overflow-y: auto;
  }
  .sheet:not([open]) {
    display: none;
  }
  .sheet::backdrop {
    background: var(--scrim);
  }
  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 14px;
    padding: 22px 22px 0;
  }
  header .ibtn {
    flex: none;
  }
  /* The rail's lockup, a size up. */
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .brand :global(.mark) {
    font-size: 40px;
  }
  .word {
    font-family: 'Doto Variable', var(--mono);
    font-size: 28px;
    font-weight: 700;
    letter-spacing: 0.01em;
    color: var(--muted);
  }
  .word b {
    font-weight: 900;
    color: var(--text);
  }
  .ver {
    align-self: flex-end;
    margin-bottom: 5px;
    font-size: 12px;
    color: var(--muted);
  }
  .body {
    display: grid;
    gap: 16px;
    padding: 16px 22px 22px;
    font-size: 13.5px;
  }
  .body p {
    max-width: 62ch;
    text-wrap: pretty;
  }
  .lead {
    color: var(--text);
  }
  .links {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 18px;
  }
  /* A page outside the app: plain text with an arrow, not a button. */
  .link {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 0;
    border: 0;
    background: none;
    font: inherit;
    font-size: 13px;
    color: var(--text);
    text-decoration: underline;
    text-decoration-color: var(--line2);
    text-underline-offset: 3px;
    cursor: pointer;
    transition: text-decoration-color var(--fast) var(--ease);
  }
  .link:hover {
    text-decoration-color: currentColor;
  }
  .link :global(svg) {
    color: var(--muted);
  }
  .give {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 18px;
    padding: 14px 0;
    border-block: 1px solid var(--line);
  }
  .give p {
    flex: 1 1 30ch;
    color: var(--muted);
  }
  .give .acts {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 6px;
    flex: none;
  }
  section {
    display: grid;
    gap: 10px;
  }
  h3 {
    font-size: 13.5px;
    font-weight: 500;
  }
  section p {
    color: var(--muted);
  }
  .texts {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .btn[aria-expanded='true'] {
    background: var(--raise);
  }
  .well {
    margin: 0;
    max-height: min(320px, 45vh);
    overflow: auto;
    padding: 12px 14px;
    border-radius: 12px;
    background: var(--stage);
    box-shadow: inset 0 0 0 1px var(--line);
    font-family: var(--mono);
    font-size: 11.5px;
    line-height: 1.55;
    color: var(--muted);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    user-select: text;
  }
</style>
