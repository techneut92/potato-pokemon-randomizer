<script lang="ts">
  import { app } from '../../app.svelte';
  import type { EditableGame } from '../../backend';
  import type { SpeciesEdit } from '../../generated/SpeciesEdit';
  import type { StatsEdit } from '../../generated/StatsEdit';
  import { clampNum } from '../../edits/items';
  import { issuesOf } from '../../edits/issues';
  import { abilityName, abilityPicker, typePicker } from '../../edits/pickers';
  import { copy, EDIT_SECTIONS } from '../../edits/sections';
  import PickButton from '../PickButton.svelte';
  import IssueNote from './IssueNote.svelte';
  import ListPane, { type ListRow } from './ListPane.svelte';

  let { view }: { view: EditableGame } = $props();
  const sec = EDIT_SECTIONS.find((s) => s.id === 'mons')!;

  /** A species as edited, or as the game has it. */
  const valueOf = (n: number): SpeciesEdit => {
    const e = app.edits.get({ kind: 'species', number: n });
    return e?.kind === 'species' ? e : view.species[n - 1].value;
  };
  const typesText = (v: SpeciesEdit) => v.primaryType + (v.secondaryType ? ' / ' + v.secondaryType : '');

  const rows: ListRow[] = $derived(
    view.species.map((s) => ({ id: s.value.number, label: s.name, sub: '#' + s.value.number + ' · ' + typesText(valueOf(s.value.number)), edited: app.edits.edited({ kind: 'species', number: s.value.number }) }))
  );
  const n = $derived(Math.min(Math.max(1, app.editSel.mons ?? 1), view.species.length));
  const ref = $derived({ kind: 'species', number: n } as const);
  const original = $derived(view.species[n - 1].value);
  const cur = $derived(valueOf(n));

  const set = (change: (v: SpeciesEdit) => void) => {
    const next = copy(cur);
    change(next);
    app.setEdit({ kind: 'species', ...next }, { kind: 'species', ...original });
  };

  // Gen 1 has one Special stat, kept as the special attack; its special defense stays 0.
  type Stat = { key: keyof StatsEdit; label: string };
  const stats: Stat[] = $derived(
    view.generation === 1
      ? [{ key: 'hp', label: 'HP' }, { key: 'attack', label: 'Attack' }, { key: 'defense', label: 'Defense' }, { key: 'speed', label: 'Speed' }, { key: 'spAttack', label: 'Special' }]
      : [{ key: 'hp', label: 'HP' }, { key: 'attack', label: 'Attack' }, { key: 'defense', label: 'Defense' }, { key: 'spAttack', label: 'Sp. Atk' }, { key: 'spDefense', label: 'Sp. Def' }, { key: 'speed', label: 'Speed' }]
  );
  const total = (s: StatsEdit) => stats.reduce((t, x) => t + s[x.key], 0);
  const setStat = (x: Stat, text: string) =>
    set((v) => {
      const value = clampNum(text, 1, 255);
      v.stats[x.key] = value;
    });

  const abilityLabel = (k: number) => (k === 2 ? 'Hidden ability' : 'Ability ' + (k + 1));
</script>

<div class="split">
  <ListPane label={sec.label} hint={sec.hint ?? 'Search'} plural={sec.plural ?? ''} {rows} selected={n} onpick={(id) => app.selectEdit('mons', id)} />

  <section class="card detail" aria-label="Edit selected">
    <div class="dhead">
      <div class="dtitle"><p class="sub">#{n}</p><h2 class="disp">{view.species[n - 1].name}</h2></div>
      <button class="btn ghost sm" disabled={!app.edits.edited(ref)} onclick={() => app.edits.revert(ref)}>Revert</button>
    </div>
    <div class="fgrid">
      <div class="fld">
        <span class="flab">Type 1</span>
        <PickButton name={cur.primaryType} label="Type 1" onclick={() => app.openPicker(typePicker(view, app.rom.name, { label: 'Type 1', current: cur.primaryType, original: original.primaryType, apply: (t) => set((v) => (v.primaryType = t ?? v.primaryType)) }))} />
      </div>
      <div class="fld">
        <span class="flab">Type 2</span>
        <PickButton name={cur.secondaryType ?? 'None'} label="Type 2" onclick={() => app.openPicker(typePicker(view, app.rom.name, { label: 'Type 2', current: cur.secondaryType, original: original.secondaryType, apply: (t) => set((v) => (v.secondaryType = t)) }, true))} />
      </div>
    </div>
    {#if cur.abilities.length && view.abilities.length}
      <div class="fgrid">
        {#each cur.abilities as a, k (k)}
          <div class="fld">
            <span class="flab">{abilityLabel(k)}</span>
            <PickButton name={abilityName(view, a)} label={abilityLabel(k)} onclick={() => app.openPicker(abilityPicker(view, app.rom.name, { label: abilityLabel(k), current: a, original: original.abilities[k], apply: (id) => set((v) => (v.abilities[k] = id)) }, k > 0))} />
          </div>
        {/each}
      </div>
    {/if}
    <div class="stats">
      <div class="mrow mhead" aria-hidden="true"><span class="statlab">Stat</span><span class="vh">Value</span></div>
      {#each stats as x (x.key)}
        {@const value = cur.stats[x.key]}
        <div class="mrow">
          <label class="statlab" for="st-{x.key}">{x.label}</label>
          <input id="st-{x.key}" class="num mono" type="number" min="1" max="255" {value} onchange={(e) => {
            setStat(x, e.currentTarget.value);
            e.currentTarget.value = String(cur.stats[x.key]);
          }} />
          <div class="meter bar"><div class="fill" class:changed={value !== original.stats[x.key]} style:width="{Math.round((value / 255) * 100)}%"></div></div>
        </div>
      {/each}
    </div>
    <div class="note">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><circle cx="12" cy="12" r="9"></circle><path d="M12 11v5M12 8h.01"></path></svg>
      <span>Base stat total <strong class="mono">{total(cur.stats)}</strong> (was {total(original.stats)}).{view.generation === 1 ? ' This game has one Special stat; later games split it into Sp. Atk and Sp. Def.' : ''}</span>
    </div>
    <IssueNote issues={issuesOf(app.editIssues, ref)} />
  </section>
</div>

<style>
  .sub { margin: 0; }
  .stats { display: flex; flex-direction: column; }
  .vh { width: 84px; }
  .bar { flex: 1 1 120px; min-width: 60px; }
  .fill { height: 100%; border-radius: 999px; background: var(--accent); }
  .fill.changed { background: var(--amber); }
  strong { color: var(--text); }
</style>
