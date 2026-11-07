<script lang="ts">
  import { app } from '../../app.svelte';
  import type { EditableGame } from '../../backend';
  import type { TrainerPokemonEdit } from '../../generated/TrainerPokemonEdit';
  import { dexNum, speciesName } from '../../edits/changes';
  import { clampNum, itemName, itemTag } from '../../edits/items';
  import { issuesOf } from '../../edits/issues';
  import { itemPicker, moveName, movePicker, speciesPicker } from '../../edits/pickers';
  import { copy, EDIT_SECTIONS } from '../../edits/sections';
  import { addedPokemon, inFilter, limitsOf, tagLabel, teamNote, teamText, TRAINER_FILTERS, trainerFilters, trainerLabels, withMove } from '../../edits/trainers';
  import Icon from '../Icon.svelte';
  import PickButton from '../PickButton.svelte';
  import IssueNote from './IssueNote.svelte';
  import ListPane, { type ListRow } from './ListPane.svelte';

  let { view }: { view: EditableGame } = $props();
  const sec = EDIT_SECTIONS.find((s) => s.id === 'trainers')!;

  const labels = $derived(trainerLabels(view.trainers));
  const teamOf = (index: number, fallback: TrainerPokemonEdit[]): TrainerPokemonEdit[] => {
    const e = app.edits.get({ kind: 'trainer', index });
    return e?.kind === 'trainer' ? e.pokemon : fallback;
  };
  const rows: ListRow[] = $derived(
    view.trainers.map((t, i) => {
      const role = tagLabel(t.tag);
      const team = teamOf(t.value.index, t.value.pokemon).map((p) => speciesName(view, p.species) + ' ' + p.level).join(', ');
      return { id: t.value.index, label: labels[i], sub: (role ? role + ' · ' : '') + team, edited: app.edits.edited({ kind: 'trainer', index: t.value.index }), tags: TRAINER_FILTERS.filter(([f]) => inFilter(f, t.tag)).map(([f]) => f) };
    })
  );

  // Without a choice yet, the first gym leader: the first trainer most people look for.
  const fallback = $derived(view.trainers.find((t) => t.tag === 'GYM1-LEADER')?.value.index ?? view.trainers[0]?.value.index ?? 1);
  const pos = $derived(Math.max(0, view.trainers.findIndex((t) => t.value.index === (app.editSel.trainers ?? fallback))));
  const def = $derived(view.trainers[pos]);
  const index = $derived(def.value.index);
  const ref = $derived({ kind: 'trainer', index } as const);
  const team = $derived(teamOf(index, def.value.pokemon));
  const limits = $derived(limitsOf(view, index));
  const moves = $derived(limits.customMoves && view.rules.trainers.customMoves && view.moves.length > 1);
  const heldItems = $derived(limits.heldItems && view.rules.trainers.heldItems);

  const setTeam = (change: (team: TrainerPokemonEdit[]) => void) => {
    const next = copy(team);
    change(next);
    app.setEdit({ kind: 'trainer', index, pokemon: next }, { kind: 'trainer', ...def.value });
  };

  /** The starter a Pokémon of this team follows, when the game sets it from one. */
  const linkOf = (k: number) => view.rules.starterLinks.find((l) => l.trainer === index && l.slot === k);
  const starterNow = (slot: number) => {
    const e = app.edits.get({ kind: 'starter', slot });
    return e?.kind === 'starter' ? e.species : (view.starters.find((s) => s.slot === slot)?.species ?? 0);
  };

  // Rival and friend battles of Gen 2 on come in one variant per starter; Gen 1 numbers them.
  const pickNote = $derived.by(() => {
    const m = /^(RIVAL|FRIEND)\d+-(\d)$/.exec(def.tag ?? '');
    if (!m) return '';
    const k = Number(m[2]);
    const family = (t: string | null) => (t ?? '').replace(/-\d$/, '');
    const variants = view.trainers.filter((t) => family(t.tag) === family(def.tag)).length > 1;
    if (!variants) return '';
    if (view.generation >= 2 && k < Math.min(3, view.starters.length)) return 'Faced if you pick ' + speciesName(view, starterNow(k));
    return 'Variant ' + (k + 1);
  });

  let bulk = $state(20);

  const moveLabel = (p: TrainerPokemonEdit, m: number) => (m || !p.moves ? moveName(view, m) : 'No move');
</script>

<div class="split">
  <ListPane label={sec.label} hint={sec.hint ?? 'Search'} plural={sec.plural ?? ''} {rows} selected={index} filters={trainerFilters(view.trainers)} filterLabel="Trainer filter" onpick={(id) => app.selectEdit('trainers', id)} />

  <section class="card detail" aria-label="Edit selected">
    <div class="dhead">
      <div class="dtitle">
        <h2 class="disp">{labels[pos]}</h2>
        <div class="chips">
          <span class="chip">Trainer {index}</span>
          {#if tagLabel(def.tag)}<span class="chip">{tagLabel(def.tag)}</span>{/if}
          {#if pickNote}<span class="chip">{pickNote}</span>{/if}
          {#if limits.starterSlot !== null}<span class="chip">Carries a starter in slot {limits.starterSlot + 1}</span>{/if}
          {#if limits.sharesBattle}<span class="chip">Battles with a partner, bringing its first 3</span>{/if}
        </div>
      </div>
      <button class="btn ghost sm" disabled={!app.edits.edited(ref)} onclick={() => app.edits.revert(ref)}>Revert</button>
    </div>

    <div class="team">
      {#each team as p, k (k)}
        {@const link = linkOf(k)}
        {@const orig = def.value.pokemon[k]}
        {@const species = link ? starterNow(link.starter) : p.species}
        <div class="mcard">
          <div class="mrow first">
            <span class="mono slotn">{k + 1}</span>
            <div class="fld grow">
              <span class="flab">Pokémon</span>
              <PickButton num={dexNum(species)} name={speciesName(view, species)} label="Pokémon in slot {k + 1}" disabled={!!link} onclick={() => app.openPicker(speciesPicker(view, app.rom.name, { label: 'Pokémon in slot ' + (k + 1), current: p.species, original: orig?.species, apply: (v) => setTeam((t) => (t[k].species = v)) }))} />
            </div>
            <label class="fld fixed">
              <span class="flab">Level</span>
              <input class="num mono" type="number" min="1" max="100" value={p.level} disabled={!limits.levels} onchange={(e) => {
                setTeam((t) => (t[k].level = clampNum(e.currentTarget.value, 1, 100)));
                e.currentTarget.value = String(team[k]?.level ?? '');
              }} />
            </label>
            {#if heldItems}
              <div class="fld item">
                <span class="flab">Held item</span>
                <PickButton num={p.heldItem === null ? '' : itemTag(itemName(view, p.heldItem))} name={itemName(view, p.heldItem)} label="Held item in slot {k + 1}" onclick={() => app.openPicker(itemPicker(view, app.rom.name, { label: 'Held item in slot ' + (k + 1), current: p.heldItem, original: orig?.heldItem, apply: (v) => setTeam((t) => (t[k].heldItem = v)) }, true))} />
              </div>
            {/if}
            <button class="btn ghost ico end" aria-label="Remove slot {k + 1}" disabled={!!link} onclick={() => setTeam((t) => t.splice(k, 1))}><Icon d="M6 6l12 12M18 6L6 18" size={16} /></button>
          </div>
          {#if link}<span class="linked">Follows the rival’s starter. Change it under Starters.</span>{/if}
          {#if moves}
            <div class="movegrid" role="group" aria-label="Moves for slot {k + 1}">
              {#each [0, 1, 2, 3] as j (j)}
                {@const m = p.moves?.[j] ?? 0}
                <PickButton size="sm" name={moveLabel(p, m)} label="Move {j + 1} in slot {k + 1}" onclick={() => app.openPicker(movePicker(view, { label: 'Move ' + (j + 1) + ' in slot ' + (k + 1), current: m, original: orig ? (orig.moves?.[j] ?? 0) : undefined, apply: (v) => setTeam((t) => (t[k].moves = withMove(t[k].moves, j, v))) }))} />
              {/each}
            </div>
          {/if}
        </div>
      {/each}
    </div>

    <div class="bulk">
      <button class="btn sm" onclick={() => setTeam((t) => t.push(addedPokemon(t)))}>Add Pokémon</button>
      {#if limits.levels}
        <button class="btn sm" onclick={() => setTeam((t) => t.forEach((p) => (p.level = bulk)))}>Set all levels to</button>
        <input class="num mono" type="number" min="1" max="100" value={bulk} aria-label="Level for the whole team" onchange={(e) => {
          bulk = clampNum(e.currentTarget.value, 1, 100);
          e.currentTarget.value = String(bulk);
        }} />
      {/if}
      <span class="spacer"></span>
      <span class="count">{teamText(team.length, limits.maxPokemon)}</span>
    </div>
    <IssueNote issues={issuesOf(app.editIssues, ref)} />
    <div class="note">
      <Icon d="M4 7h16M4 12h16M4 17h10" size={16} />
      <span>{teamNote(limits, view.generation)}</span>
    </div>
  </section>
</div>

<style>
  .team { display: flex; flex-direction: column; }
  .first { border-top: 0; min-height: 0; }
  /* Species, level, held item and remove fit one row down to an 800 px window. */
  .grow { flex: 1 1 140px; }
  .fixed { flex: none; }
  .item { flex: 1 1 130px; }
  .end { align-self: flex-end; }
  .linked { font-size: 12px; color: var(--faint); padding-left: 36px; }
  .bulk { display: flex; gap: 8px; flex-wrap: wrap; align-items: center; }
  .spacer { flex: 1; }
  .count { font-size: 13px; color: var(--muted); }
</style>
