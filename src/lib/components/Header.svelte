<script lang="ts">
  import { app } from '../app.svelte';
  import Icon from './Icon.svelte';
  import WindowControls from './WindowControls.svelte';
  import ModeTabs from './ModeTabs.svelte';
  import { isDesktop, isMac } from '../desktop';
  import { openSupport } from '../support';

  let open = $state(false);
  let tools: HTMLElement;
  const go = (fn: () => void) => () => {
    open = false;
    fn();
  };
  const onMain = $derived(app.screen === 'main');
</script>

<!-- The Tools menu closes on Escape and on any press outside it (capture phase, so a drag on the title bar counts too). -->
<svelte:window
  onkeydown={(e) => e.key === 'Escape' && (open = app.navOpen = false)}
  onpointerdowncapture={(e) => open && !tools.contains(e.target as Node) && (open = false)}
  onblur={() => (open = false)}
/>

<!-- In the desktop app this is the title bar: empty space and the brand drag the window, double-click maximizes. -->
<header data-tauri-drag-region class:desktop={isDesktop} class:mac={isDesktop && isMac}>
  {#if onMain}
    <button class="btn ghost menu-btn" aria-label="Categories" aria-expanded={app.navOpen} onclick={() => (app.navOpen = !app.navOpen)}>
      <Icon d="M4 6h16M4 12h16M4 18h16" />
    </button>
  {/if}
  <div class="brand" data-tauri-drag-region>
    <img src="/icon.svg" width="34" height="34" alt="" />
    <div class="names">
      <span class="disp name">Potato Pokémon Randomizer</span>
      <span class="ver">Version {__APP_VERSION__}</span>
    </div>
  </div>
  {#if onMain && app.features.edits}<ModeTabs />{/if}
  <div class="grow" data-tauri-drag-region></div>
  <div class="tools" bind:this={tools}>
    <button class="btn ghost support" aria-label="Support the randomizer on Ko-fi" title="Support the randomizer on Ko-fi" onclick={() => void openSupport()}>
      <Icon d="M12 20s-7-4.4-9.2-8.6A5 5 0 0 1 12 6.2a5 5 0 0 1 9.2 5.2C19 15.6 12 20 12 20z" />
    </button>
    <button class="btn ghost theme" aria-label="Switch light or dark theme" onclick={() => (app.theme = app.theme === 'dark' ? 'light' : 'dark')}>
      <Icon d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z" />
    </button>
    <button class="btn" aria-label="Tools" aria-expanded={open} aria-haspopup="true" onclick={() => (open = !open)}>
      <Icon d="M4 7h9m4 0h3M4 17h3m4 0h9M15 5v4M9 15v4" /> <span class="label">Tools</span>
    </button>
    {#if open}
      <div class="menu" role="menu">
        <button class="mi" role="menuitem" disabled={!onMain} onclick={go(() => (app.features.settingsString ? (app.modal = 'string') : app.notYet('Settings strings')))}>Get / Load Settings String</button>
        <button class="mi" role="menuitem" onclick={go(() => (app.features.batch ? (app.modal = 'batch') : app.notYet('Batch randomization')))}>Batch Randomization Settings</button>
        <button class="mi" role="menuitem" onclick={go(() => (app.modal = 'names'))}>Custom Names Editor</button>
        <button class="mi" role="menuitem" onclick={go(() => (app.modal = 'theme'))}>Change UI Theme</button>
        {#if onMain && app.rom.genNum >= 6}
          <hr />
          <button class="mi" role="menuitem" onclick={go(() => (app.features.gameUpdates ? (app.modal = 'update') : app.notYet('Game updates')))}>Load Game Update…</button>
          {#if app.update}<button class="mi" role="menuitem" onclick={go(() => app.unloadUpdate())}>Unload Game Update</button>{/if}
        {/if}
        <hr />
        <label class="mi"><input type="checkbox" class="chk" bind:checked={app.keepLoaded} /> Keep game loaded after randomizing</label>
        <hr />
        <button class="mi" role="menuitem" onclick={go(() => void openSupport())}>Support on Ko-fi…</button>
      </div>
    {/if}
  </div>
  {#if isDesktop && !isMac}<WindowControls />{/if}
</header>

<style>
  /* One row at every width: the title shrinks first, then the version line and the Tools label go. */
  header { flex: none; display: flex; align-items: center; gap: 16px; min-height: 58px; padding: 0 20px; border-bottom: 1px solid var(--line); background: var(--panel); }
  header.desktop { padding-right: 0; user-select: none; -webkit-user-select: none; }
  header.mac { padding-left: 84px; padding-right: 20px; }
  /* Only elements carrying data-tauri-drag-region start a drag, so the brand's contents pass clicks through. */
  .desktop .brand img, .desktop .names, .desktop .names * { pointer-events: none; }
  .brand { flex: 0 1 auto; min-width: 0; display: flex; align-items: center; gap: 12px; }
  .brand img { flex: none; }
  .names { min-width: 0; display: flex; flex-direction: column; line-height: 1.15; }
  .name, .ver { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .name { font-size: 16px; font-weight: 700; }
  .ver { font-size: 12px; color: var(--faint); }
  .grow { flex: 1 1 0; min-width: 8px; align-self: stretch; }
  /* Ko-fi's own colour, so the button reads as the support link. */
  .support { color: #ff5e5b; }
  .support:hover { color: #ff5e5b; border-color: color-mix(in srgb, #ff5e5b 45%, transparent); }
  .tools { flex: none; display: flex; align-items: center; gap: 8px; position: relative; }
  .menu-btn { display: none; flex: none; padding: 0 10px; }
  @media (max-width: 719px) {
    .menu-btn { display: inline-flex; }
  }
  @media (max-width: 900px) {
    header { gap: 10px; padding-left: 12px; }
    header.mac { padding-left: 84px; }
    .ver, .label { display: none; }
  }
  /* The smallest windows: the logo stands for the name. */
  @media (max-width: 560px) {
    .brand { flex: none; }
    .names { display: none; }
    /* Both are also in the Tools menu. */
    .theme, .support { display: none; }
  }
  /* Short windows: a lower title bar leaves more room for the content. */
  @media (max-height: 720px) {
    header { min-height: 48px; }
    .ver { display: none; }
    .tools .btn { min-height: 36px; }
  }
</style>
