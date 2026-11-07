<script lang="ts">
  import { app } from '../app.svelte';
  import Icon from './Icon.svelte';

  import { editsText } from '../edits/sections';

  const status = $derived(`${app.view.total} changed${app.edits.count ? ' · ' + editsText(app.edits.count) : ''} · Race Mode ${app.race ? 'on' : 'off'} · Batch ${app.batch.on ? 'on (' + app.batch.n + ')' : 'off'}`);
</script>

<footer>
  <button class="btn" aria-label="Load Settings" onclick={() => (app.features.settingsFiles ? app.loadSettingsFile() : app.notYet('Settings files'))}><span>Load<span class="long">{' '}Settings</span></span></button>
  <button class="btn" aria-label="Save Settings" onclick={() => (app.features.settingsFiles ? app.saveSettingsFile() : app.notYet('Settings files'))}><span>Save<span class="long">{' '}Settings</span></span></button>
  <button class="btn ghost string" onclick={() => (app.features.settingsString ? (app.modal = 'string') : app.notYet('Settings strings'))}>Settings String</button>
  <div class="grow"></div>
  {#if app.saveBlock}
    <span class="blocked" role="status"><Icon d="M12 3l9.5 17h-19zM12 10v4M12 17h.01" size={16} /><span>{app.saveBlock}</span><button class="linkbtn" onclick={() => app.showFirstIssue()}>Show</button></span>
  {:else}
    <span class="status">{status}</span>
  {/if}
  <button class="btn" onclick={() => (app.features.seedFiles ? (app.modal = 'preset') : app.notYet('Premade seeds'))}>Premade Seed</button>
  <button class="btn primary lg" aria-label="Randomize and Save" disabled={!!app.saveBlock} onclick={() => app.randomize()}>
    <Icon d="M16 3h5v5M4 20L21 3M21 16v5h-5M15 15l6 6M4 4l5 5" stroke={2.2} /> <span>Randomize<span class="long">{' '}&amp; Save</span></span>
  </button>
</footer>

<style>
  footer { flex: none; z-index: 30; display: flex; align-items: center; gap: 10px; flex-wrap: wrap; padding: 14px 20px; border-top: 1px solid var(--line); background: var(--panel); }
  .grow { flex: 1 1 0; }
  .status { font-size: 13px; color: var(--muted); }
  @media (max-width: 1100px) {
    .status { display: none; }
    footer { gap: 8px; padding: 12px; }
  }
  /* Also in the Tools menu; on narrow windows the footer keeps one row for the main actions. */
  @media (max-width: 719px) {
    .string { display: none; }
  }
  /* The smallest windows: shorter labels keep the actions on one row. */
  @media (max-width: 560px) {
    .long { display: none; }
    footer .btn { padding: 0 12px; }
  }
  @media (max-height: 720px) {
    footer { padding-block: 8px; }
    footer .btn { min-height: 36px; }
    footer .btn.lg { min-height: 40px; }
  }
</style>
