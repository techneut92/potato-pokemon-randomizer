<script lang="ts">
  import { app } from '../../app.svelte';
  import type { EditableGame } from '../../backend';
  import { itemName, itemTag } from '../../edits/items';
  import { issuesOf } from '../../edits/issues';
  import { itemPicker } from '../../edits/pickers';
  import { sameRef } from '../../edits/sections';
  import Icon from '../Icon.svelte';
  import PickButton from '../PickButton.svelte';
  import IssueNote from './IssueNote.svelte';

  let { view }: { view: EditableGame } = $props();
  /** A long list shows this many; the filter narrows it. */
  const SHOWN = 400;

  let query = $state('');
  const all = $derived(
    view.fieldItems.map((def) => {
      const ref = { kind: 'fieldItem', index: def.index } as const;
      const e = app.edits.get(ref);
      const item = e?.kind === 'fieldItem' ? e.item : def.item;
      // Items the game needs where they are stay put.
      return { ref, def, item, edited: !!e, locked: view.rules.fieldItems.required.includes(def.item), issues: issuesOf(app.editIssues, ref) };
    })
  );
  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return q ? all.filter((f) => itemName(view, f.item).toLowerCase().includes(q)) : all;
  });
  const anyLocked = $derived(all.some((f) => f.locked));

  // A jump from the Changes list or a warning brings its field item into view.
  let grid: HTMLElement;
  $effect(() => {
    const focus = app.editFocus;
    if (focus?.kind !== 'fieldItem') return;
    query = '';
    const el = grid.querySelector<HTMLElement>(`[data-index="${focus.index}"]`);
    el?.scrollIntoView({ block: 'nearest' });
    el?.querySelector('button')?.focus();
  });
</script>

<section class="card field">
  <div class="bar">
    <label class="search grow"><Icon d="M11 4a7 7 0 1 0 0 14 7 7 0 1 0 0-14zM20 20l-3.5-3.5" size={16} /><input type="search" placeholder="Filter by item" aria-label="Filter by item" bind:value={query} /></label>
    <span class="hint">{shown.length} of {all.length} items{shown.length > SHOWN ? ' · showing the first ' + SHOWN + ', filter to narrow' : ''}</span>
  </div>
  <div class="grid" bind:this={grid}>
    {#each shown.slice(0, SHOWN) as f (f.def.index)}
      <div class="tilewrap">
        <div class="tile" class:cur={app.editFocus !== null && sameRef(app.editFocus, f.ref)} data-index={f.def.index}>
          <span class="mono pos">{f.def.index + 1}</span>
          <PickButton num={itemTag(itemName(view, f.item))} name={itemName(view, f.item)} label="Field item {f.def.index + 1}" disabled={f.locked && !f.edited} onclick={() => app.openPicker(itemPicker(view, app.rom.name, { label: 'Field item ' + (f.def.index + 1), current: f.item, original: f.def.item, apply: (v) => app.setEdit({ kind: 'fieldItem', index: f.def.index, item: v ?? f.def.item }, { kind: 'fieldItem', ...f.def }) }))} />
          {#if f.locked}<span class="lock" role="img" aria-label="Stays in place"><Icon d="M5 11h14v10H5zM8 11V8a4 4 0 0 1 8 0v3" size={16} /></span>{/if}
          <span class="dot" class:off={!f.edited} aria-hidden="true"></span>
        </div>
        <IssueNote issues={f.issues} />
      </div>
    {/each}
  </div>
  {#if anyLocked}<p class="hint">Locked items are ones the game gives nowhere else, like TMs it expects to find in that spot; they stay where they are.</p>{/if}
</section>

<style>
  .field { display: flex; flex-direction: column; gap: 14px; }
  .bar { display: flex; gap: 8px; flex-wrap: wrap; align-items: center; }
  .grow { flex: 1 1 240px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 220px), 1fr)); gap: 8px; }
  .tilewrap { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  /* A row: the global .tile (theme previews) is a column. */
  .tile { display: flex; flex-direction: row; align-items: center; gap: 8px; min-height: 52px; padding: 6px 10px; border-radius: 12px; background: var(--panel2); min-width: 0; }
  .tile.cur { box-shadow: inset 0 0 0 1px var(--accent); }
  .tile :global(.pickbtn) { flex: 1 1 auto; width: auto; min-width: 0; background: var(--panel); max-width: none; }
  .pos { width: 34px; flex: none; color: var(--faint); font-size: 12px; }
  .lock { flex: none; display: inline-flex; color: var(--faint); }
  .hint { margin: 0; }
</style>
