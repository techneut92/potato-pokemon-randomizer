<script lang="ts">
  import { app } from '../../app.svelte';
  import { describeChanges } from '../../edits/changes';
  import Icon from '../Icon.svelte';

  /** Below 1440 px the list covers the editor, so opening an object, or Escape, closes it. */
  const narrow = () => matchMedia('(max-width: 1439px)').matches;
  const open = (c: (typeof changes)[number]) => {
    if (narrow()) app.changesOpen = false;
    app.goToEdit(c.section.id, c.target);
  };
  const changes = $derived(app.editView ? describeChanges(app.edits.edits, app.editView) : []);
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && !app.modal && narrow() && (app.changesOpen = false)} />

<aside aria-label="Changes">
  <div class="head">
    <h2 class="disp">Changes</h2>
    <button class="btn ghost sm" disabled={!changes.length} onclick={() => app.edits.revertAll()}>Revert all</button>
  </div>
  {#if !changes.length}
    <p class="lede">Nothing edited yet. Pick an object and change it; every edit shows up here.</p>
  {/if}
  <div class="scroll" role="list">
    {#each changes as c (c.key)}
      <div class="change" role="listitem">
        <span class="dot" aria-hidden="true"></span>
        <button class="li" onclick={() => open(c)}><span class="min0"><span class="ell">{c.label}</span><small>{c.detail}</small></span></button>
        <button class="btn ghost ico" aria-label="Revert {c.label}" onclick={() => app.edits.revert(c.target)}><Icon d="M3 12a9 9 0 1 0 3-6.7L3 8M3 3v5h5" size={16} /></button>
      </div>
    {/each}
  </div>
</aside>

<style>
  aside { flex: none; width: clamp(260px, 22vw, 320px); overflow-y: auto; padding: 20px 16px; border-left: 1px solid var(--line); display: flex; flex-direction: column; gap: 12px; scrollbar-width: thin; }
  .head { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
  h2 { margin: 0; font-size: 18px; }
  /* The panel itself scrolls; the list inside it doesn't. */
  .scroll { gap: 6px; overflow: visible; }
  .change { flex: none; display: flex; align-items: center; gap: 10px; min-height: 52px; padding: 6px 10px; border-radius: 12px; background: var(--panel2); }
  .li { flex: 1; min-width: 0; min-height: 40px; padding: 0; }
  .li:hover { background: transparent; }
  .min0 { min-width: 0; flex: 1; }
  /* Below 1440 px: a drawer over the editor, along the right edge, with a scrim (App.svelte). */
  @media (max-width: 1439px) {
    aside { position: absolute; inset: 0 0 0 auto; z-index: 33; width: min(340px, 85vw); background: var(--bg); box-shadow: 0 18px 50px rgba(0, 0, 0, 0.35); }
  }
</style>
