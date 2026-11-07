<script lang="ts">
  import { app } from '../../app.svelte';
  import type { EditableGame } from '../../backend';
  import type { WildSlotEdit } from '../../generated/WildSlotEdit';
  import { dexNum, speciesName, wildSetName } from '../../edits/changes';
  import { clampNum } from '../../edits/items';
  import { issuesOf } from '../../edits/issues';
  import { speciesPicker } from '../../edits/pickers';
  import { copy, EDIT_SECTIONS } from '../../edits/sections';
  import { slotChances, slotGroup, slotGroups, slotName, slotRole, wildChips, wildNote, type SlotGroup } from '../../edits/wild';
  import Icon from '../Icon.svelte';
  import PickButton from '../PickButton.svelte';
  import IssueNote from './IssueNote.svelte';
  import ListPane, { type ListRow } from './ListPane.svelte';

  let { view }: { view: EditableGame } = $props();
  const sec = EDIT_SECTIONS.find((s) => s.id === 'wild')!;
  const gen = $derived(view.generation);

  const slotsOf = (i: number): WildSlotEdit[] => {
    const e = app.edits.get({ kind: 'wildSet', index: i });
    return e?.kind === 'wildSet' ? e.slots : view.wildSets[i].value.slots;
  };
  const rows: ListRow[] = $derived(
    view.wildSets.map((_, i) => ({ id: i, label: wildSetName(view, i), sub: [...new Set(slotsOf(i).map((s) => speciesName(view, s.species)))].join(', '), edited: app.edits.edited({ kind: 'wildSet', index: i }) }))
  );
  const i = $derived(Math.min(Math.max(0, app.editSel.wild ?? 0), view.wildSets.length - 1));
  const def = $derived(view.wildSets[i]);
  const ref = $derived({ kind: 'wildSet', index: i } as const);
  const slots = $derived(slotsOf(i));
  const levels = $derived(view.rules.wild.levels[i] ?? (gen <= 2 ? 'single' : 'range'));
  const chances = $derived(slotChances(gen, def.kind, def.value.slots.length));
  const groups = $derived(slotGroups(gen, slots.length));
  let picked = $state<SlotGroup>('wild');
  const group = $derived(groups.some((g) => g.id === picked) ? picked : 'wild');
  const shown = $derived(slots.map((s, k) => ({ s, k })).filter(({ k }) => slotGroup(gen, slots.length, k) === group));

  const setSlots = (change: (slots: WildSlotEdit[]) => void) => {
    const next = copy(slots);
    change(next);
    app.setEdit({ kind: 'wildSet', index: i, slots: next }, { kind: 'wildSet', ...def.value });
  };
  /** Sets one end of a slot's level range, moving the other end along so the range stays whole. */
  const setLevel = (k: number, end: 'min' | 'max' | 'both', text: string) =>
    setSlots((all) => {
      const v = clampNum(text, 1, 100);
      const s = all[k];
      if (end !== 'max') s.minLevel = v;
      if (end !== 'min') s.maxLevel = v;
      if (s.maxLevel < s.minLevel) {
        if (end === 'max') s.minLevel = v;
        else s.maxLevel = v;
      }
    });
  /** Sets one end of the range the whole table shares. */
  const setShared = (end: 'min' | 'max', text: string) =>
    setSlots((all) => {
      const v = clampNum(text, 1, 100);
      for (const s of all) {
        if (end === 'min') {
          s.minLevel = v;
          if (s.maxLevel < v) s.maxLevel = v;
        } else {
          s.maxLevel = v;
          if (s.minLevel > v) s.minLevel = v;
        }
      }
    });
</script>

<div class="split">
  <ListPane label={sec.label} hint={sec.hint ?? 'Search'} plural={sec.plural ?? ''} {rows} selected={i} onpick={(id) => app.selectEdit('wild', id)} />

  <section class="card detail" aria-label="Edit selected">
    <div class="dhead">
      <div class="dtitle">
        <h2 class="disp">{wildSetName(view, i)}</h2>
        <div class="chips">
          {#each wildChips(def) as c (c)}<span class="chip">{c}</span>{/each}
          {#if gen <= 4 && def.rate > 0}<span class="chip">Encounter rate {def.rate}</span>{/if}
        </div>
      </div>
      <button class="btn ghost sm" disabled={!app.edits.edited(ref)} onclick={() => app.edits.revert(ref)}>Revert</button>
    </div>

    {#if levels === 'shared' && slots.length}
      <div class="shared">
        <label class="fld fixed"><span class="flab">Lowest level</span><input class="num mono" type="number" min="1" max="100" value={slots[0].minLevel} onchange={(e) => setShared('min', e.currentTarget.value)} /></label>
        <label class="fld fixed"><span class="flab">Highest level</span><input class="num mono" type="number" min="1" max="100" value={slots[0].maxLevel} onchange={(e) => setShared('max', e.currentTarget.value)} /></label>
        <span class="hint">One level range for the whole table.</span>
      </div>
    {/if}
    {#if groups.length > 1}
      <div class="pills" role="radiogroup" aria-label="Slot group">
        {#each groups as g (g.id)}
          <button class="pill" class:cur={group === g.id} role="radio" aria-checked={group === g.id} onclick={() => (picked = g.id)}>{g.label} ({g.count})</button>
        {/each}
      </div>
    {/if}

    <div class="slots">
      {#each shown as { s, k } (k)}
        {@const name = slotName(gen, slots.length, k)}
        {@const role = slotRole(gen, slots.length, k)}
        <div class="mrow slot">
          <span class="mono slotn">{name}</span>
          {#if chances}<span class="mono pct">{chances[k]}</span>{/if}
          <div class="who">
            <PickButton num={dexNum(s.species)} name={speciesName(view, s.species)} label="Pokémon in slot {name}" onclick={() => app.openPicker(speciesPicker(view, app.rom.name, { label: 'Pokémon in slot ' + name, current: s.species, original: def.value.slots[k]?.species, apply: (v) => setSlots((all) => (all[k].species = v)) }))} />
            {#if role}<span class="ell role">{role}</span>{/if}
          </div>
          {#if levels === 'single'}
            <input class="num mono" type="number" min="1" max="100" value={s.minLevel} aria-label="Level in slot {name}" onchange={(e) => setLevel(k, 'both', e.currentTarget.value)} />
          {:else if levels === 'range'}
            <span class="range">
              <input class="num mono" type="number" min="1" max="100" value={s.minLevel} aria-label="Lowest level in slot {name}" onchange={(e) => setLevel(k, 'min', e.currentTarget.value)} />
              <span aria-hidden="true" class="dash">–</span>
              <input class="num mono" type="number" min="1" max="100" value={s.maxLevel} aria-label="Highest level in slot {name}" onchange={(e) => setLevel(k, 'max', e.currentTarget.value)} />
            </span>
          {:else if levels === 'fixed'}
            <span class="mono fixedlv">Lv. {s.minLevel}{s.maxLevel !== s.minLevel ? '–' + s.maxLevel : ''}</span>
          {/if}
        </div>
      {/each}
    </div>
    <IssueNote issues={issuesOf(app.editIssues, ref)} />
    <div class="note">
      <Icon d="M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 11v5M12 8h.01" size={16} />
      <span>{wildNote(levels, gen)}</span>
    </div>
  </section>
</div>

<style>
  .shared { display: flex; gap: 12px; align-items: flex-end; flex-wrap: wrap; }
  .shared .hint { padding-bottom: 10px; }
  .fixed { flex: none; }
  .slots { display: flex; flex-direction: column; }
  .pct { width: 52px; flex: none; color: var(--muted); font-size: 13px; }
  /* A slot is one line while the pane has room; on narrow panes its levels go under the species. */
  .slot { flex-wrap: nowrap; }
  .who { flex: 1 1 200px; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .who :global(.pickbtn) { width: 100%; max-width: 320px; }
  .slot .num { width: 64px; }
  @container pane (max-width: 440px) {
    .slot { flex-wrap: wrap; }
    .slot > :last-child:not(.who) { margin-left: 36px; }
  }
  .role { font-size: 12px; color: var(--faint); }
  /* width: auto, or the global .range (the slider) makes it fill the row and squeeze the species. */
  .range { flex: none; width: auto; display: inline-flex; gap: 6px; align-items: center; }
  .dash { color: var(--faint); }
  .fixedlv { font-size: 13px; color: var(--muted); }
</style>
