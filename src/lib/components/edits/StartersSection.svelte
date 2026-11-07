<script lang="ts">
  import { app } from '../../app.svelte';
  import type { EditableGame } from '../../backend';
  import type { Edit } from '../../generated/Edit';
  import { dexNum, heldItemLabel, speciesName, starterRole } from '../../edits/changes';
  import { itemName, itemTag } from '../../edits/items';
  import { issuesOf } from '../../edits/issues';
  import { itemPicker, speciesPicker } from '../../edits/pickers';
  import { sameRef } from '../../edits/sections';
  import PickButton from '../PickButton.svelte';
  import IssueNote from './IssueNote.svelte';

  let { view }: { view: EditableGame } = $props();

  const cards = $derived(
    view.starters.map(({ slot, species: original }) => {
      const ref = { kind: 'starter', slot } as const;
      const edit = app.edits.get(ref);
      const species = edit?.kind === 'starter' ? edit.species : original;
      return { slot, ref, original, species, edited: !!edit, ...starterRole(slot, view.starters.length, view.generation), issues: issuesOf(app.editIssues, ref) };
    })
  );

  // Held items: one per starter shows on each card; one for all of them (Gen 3) gets a card of its own.
  const held = $derived(
    view.starterHeldItems.map(({ slot, item: original }) => {
      const ref = { kind: 'starterHeldItem', slot } as const;
      const edit = app.edits.get(ref);
      const item = edit?.kind === 'starterHeldItem' ? edit.item : original;
      return { slot, ref, original, item, edited: !!edit, label: heldItemLabel(view, slot), issues: issuesOf(app.editIssues, ref) };
    })
  );
  const perStarter = $derived(held.length === view.starters.length);
  const heldOf = (slot: number) => (perStarter ? held.find((h) => h.slot === slot) : undefined);
  const pickHeld = (h: (typeof held)[number]) =>
    app.openPicker(
      itemPicker(view, app.rom.name, {
        label: h.label,
        current: h.item,
        original: h.original,
        apply: (item) => app.setEdit({ kind: 'starterHeldItem', slot: h.slot, item }, { kind: 'starterHeldItem', slot: h.slot, item: h.original })
      }, true)
    );

  const pick = (c: (typeof cards)[number]) =>
    app.openPicker(
      speciesPicker(view, app.rom.name, {
        label: c.role,
        current: c.species,
        original: c.original,
        apply: (species) => {
          const edit: Edit = { kind: 'starter', slot: c.slot, species };
          app.setEdit(edit, { kind: 'starter', slot: c.slot, species: c.original });
        }
      })
    );

  // A jump from the Changes list or a warning brings its starter into view.
  let grid: HTMLElement;
  $effect(() => {
    const focus = app.editFocus;
    if (focus?.kind !== 'starter' && focus?.kind !== 'starterHeldItem') return;
    const held = focus.kind === 'starterHeldItem' && !perStarter;
    const el = grid.querySelector<HTMLElement>(held ? `[data-held="${focus.slot}"]` : `[data-slot="${focus.slot}"]`);
    el?.scrollIntoView({ block: 'nearest' });
    el?.querySelector('button')?.focus();
  });
</script>

<div class="grid" bind:this={grid}>
  {#each cards as c (c.slot)}
    <section class="card starter" class:cur={app.editFocus !== null && (sameRef(app.editFocus, c.ref) || (perStarter && sameRef(app.editFocus, { kind: 'starterHeldItem', slot: c.slot })))} data-slot={c.slot} aria-label={c.role}>
      <div class="top">
        <div class="min0"><p class="sub">{c.role}</p><h2 class="disp">{speciesName(view, c.species)}</h2></div>
        <span class="dot" class:off={!c.edited} aria-hidden="true"></span>
        {#if c.edited}<span class="sr">Edited</span>{/if}
      </div>
      <PickButton size="lg" num={dexNum(c.species)} name={speciesName(view, c.species)} label={c.role} onclick={() => pick(c)} />
      {#if heldOf(c.slot)}
        {@const h = heldOf(c.slot)!}
        <div class="fld">
          <span class="flab">Held item <span class="dot" class:off={!h.edited} aria-hidden="true"></span></span>
          <PickButton num={h.item === null ? '' : itemTag(itemName(view, h.item))} name={itemName(view, h.item)} label={h.label} onclick={() => pickHeld(h)} />
        </div>
      {/if}
      <span class="note-line">{c.note}</span>
      <IssueNote issues={[...c.issues, ...(heldOf(c.slot)?.issues ?? [])]} />
    </section>
  {/each}
  {#if !perStarter}
    {#each held as h (h.slot)}
      <section class="card starter" class:cur={app.editFocus !== null && sameRef(app.editFocus, h.ref)} data-held={h.slot} aria-label={h.label}>
        <div class="top">
          <div class="min0"><p class="sub">{h.label}</p><h2 class="disp">{itemName(view, h.item)}</h2></div>
          <span class="dot" class:off={!h.edited} aria-hidden="true"></span>
          {#if h.edited}<span class="sr">Edited</span>{/if}
        </div>
        <PickButton size="lg" num={h.item === null ? '' : itemTag(itemName(view, h.item))} name={itemName(view, h.item)} label={h.label} onclick={() => pickHeld(h)} />
        <span class="note-line">{held.length === 1 ? 'Every starter holds this item when you get it.' : 'The item this starter holds when you get it.'}</span>
        <IssueNote issues={h.issues} />
      </section>
    {/each}
  {/if}
</div>

<style>
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 300px), 1fr)); gap: 16px; }
  .starter { display: flex; flex-direction: column; gap: 14px; min-width: 0; }
  .fld .flab { display: inline-flex; align-items: center; gap: 8px; }
  .starter.cur { box-shadow: inset 0 0 0 1px var(--accent); }
  .top { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
  .min0 { min-width: 0; }
  .sub { margin: 0; }
  h2 { margin: 4px 0 0; font-size: 22px; }
  .note-line { font-size: 13px; color: var(--muted); }
  .sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
</style>
