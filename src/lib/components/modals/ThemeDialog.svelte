<script lang="ts">
  import { app, type Theme } from '../../app.svelte';
  import Dialog from './Dialog.svelte';

  const THEMES: [string, Theme, string, string][] = [['Dark', 'dark', '#111216', '#20232B'], ['Light', 'light', '#F3F3F6', '#FFFFFF']];
  const ACCENTS: [string, string][] = [['Coral', '#FF5A5F'], ['Blue', '#4F8BFF'], ['Teal', '#2EC4A6'], ['Amber', '#FFB020']];
</script>

<Dialog id="d-theme" title="Change UI Theme">
  <div class="themes">
    {#each THEMES as [label, id, bg, side] (id)}
      <button class="tile theme" class:cur={app.theme === id} aria-pressed={app.theme === id} onclick={() => (app.theme = id)}>
        <span class="preview" style:background={bg}><span class="p-side" style:background={side}></span><span class="p-top" style:background={side}></span><span class="p-acc"></span></span>
        <span class="tlabel">{label}</span>
      </button>
    {/each}
  </div>
  <div class="stack">
    <span class="lbl">Accent</span>
    <div class="accents">
      {#each ACCENTS as [name, hex] (hex)}
        <button class="swatch" aria-label={name} aria-pressed={app.accent === hex} style:background={hex} style:box-shadow={app.accent === hex ? '0 0 0 2px var(--panel), 0 0 0 4px var(--text)' : 'none'} onclick={() => (app.accent = hex)}></button>
      {/each}
    </div>
  </div>
  {#snippet footer()}<button class="btn primary lg" onclick={() => app.closeModal()}>Done</button>{/snippet}
</Dialog>

<style>
  .themes { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }
  .theme { align-items: stretch; padding: 10px; }
  .preview { display: block; height: 80px; border-radius: 9px; border: 1px solid var(--line2); position: relative; overflow: hidden; }
  .p-side { position: absolute; left: 8px; top: 8px; width: 28%; bottom: 8px; border-radius: 5px; }
  .p-top { position: absolute; left: 38%; right: 8px; top: 8px; height: 14px; border-radius: 5px; }
  .p-acc { position: absolute; right: 8px; bottom: 8px; width: 34%; height: 14px; border-radius: 5px; background: var(--accent); }
  .tlabel { color: var(--text); font-size: 14px; }
  .stack { display: flex; flex-direction: column; gap: 10px; }
  .accents { display: flex; gap: 10px; }
  .swatch { width: 36px; height: 36px; border-radius: 50%; border: 0; cursor: pointer; }
  .swatch:focus-visible { outline: 2px solid var(--accent); outline-offset: 4px; }
</style>
