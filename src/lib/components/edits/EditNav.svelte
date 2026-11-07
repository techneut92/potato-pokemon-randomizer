<script lang="ts">
  import { app } from '../../app.svelte';
  import { sectionSize } from '../../edits/changes';
  import { EDIT_SECTIONS, type EditSection } from '../../edits/sections';
  import Icon from '../Icon.svelte';
  import Tip from '../Tip.svelte';

  const HOW = 'Edits are applied after randomizing, so they win over random results. With every randomizer option off, the ROM gets only these edits.';

  /** How many objects a section lists, once the editor has the game's data. */
  const size = (s: EditSection): number | null => (app.editView ? sectionSize(app.editView, s.id) : null);
  /** The sections the game has something to edit in; all of them until its data is read. Edited ones stay, so they can be reverted. */
  const shown = $derived(EDIT_SECTIONS.filter((s) => s.id === 'starters' || size(s) !== 0 || app.edits.countOf(s.kinds) > 0));
</script>

<!-- On short windows the explanation shrinks to one line with a tip, so the sections stay in view. -->
<div class="note how">
  <Icon d="M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 11v5M12 8h.01" size={16} />
  <span class="full">{HOW}</span>
  <span class="short">Edits apply after randomizing <Tip text={HOW} label="manual edits" /></span>
</div>

<nav aria-label="Object types">
  {#each shown as s (s.id)}
    {@const cur = s.id === app.editSec}
    {@const edited = app.edits.countOf(s.kinds)}
    {@const n = size(s)}
    <button class="nav" class:cur aria-current={cur ? 'page' : undefined} onclick={() => app.goToEdit(s.id)}>
      <Icon d={s.icon} />
      {s.label}
      {#if edited}<span class="badge hot">{edited} edited</span>{:else if n !== null}<span class="badge">{n}</span>{/if}
    </button>
  {/each}
</nav>

<div class="files">
  <button class="btn sm" disabled={!app.editView} onclick={() => app.importEdits()}>Import edits…</button>
  <button class="btn sm" disabled={!app.editView || !app.edits.count} onclick={() => app.exportEdits()}>Export edits…</button>
</div>

<style>
  nav { display: flex; flex-direction: column; gap: 2px; }
  .files { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; padding-top: 12px; border-top: 1px solid var(--line); }
  .files .btn { padding: 0 8px; }
  .short { display: none; align-items: center; gap: 8px; }
  @media (max-height: 760px) {
    .how { padding: 8px 12px; align-items: center; }
    .how :global(svg) { margin-top: 0; }
    .full { display: none; }
    .short { display: inline-flex; }
  }
</style>
