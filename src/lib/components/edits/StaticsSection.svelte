<script lang="ts">
  import { app } from '../../app.svelte';
  import type { EditableGame } from '../../backend';
  import { dexNum, speciesName } from '../../edits/changes';
  import { clampNum } from '../../edits/items';
  import { issuesOf } from '../../edits/issues';
  import { speciesPicker } from '../../edits/pickers';
  import { sameRef } from '../../edits/sections';
  import PickButton from '../PickButton.svelte';
  import IssueNote from './IssueNote.svelte';

  let { view }: { view: EditableGame } = $props();

  const rows = $derived(
    view.statics.map((s, i) => {
      const ref = { kind: 'static', index: i } as const;
      const e = app.edits.get(ref);
      return { i, ref, def: s, cur: e?.kind === 'static' ? e : s.value, edited: !!e, issues: issuesOf(app.editIssues, ref) };
    })
  );
  const set = (r: (typeof rows)[number], species: number, level: number) => app.setEdit({ kind: 'static', index: r.i, species, level }, { kind: 'static', ...r.def.value });

  // A jump from the Changes list or a warning brings its encounter into view.
  let card: HTMLElement;
  $effect(() => {
    const focus = app.editFocus;
    if (focus?.kind !== 'static') return;
    const el = card.querySelector<HTMLElement>(`[data-index="${focus.index}"]`);
    el?.scrollIntoView({ block: 'nearest' });
    el?.querySelector('button')?.focus();
  });
</script>

<section class="card list" bind:this={card}>
  <div class="mrow mhead" aria-hidden="true"><span class="slotn">#</span><span class="grow">Pokémon</span><span class="lv">Level</span><span class="end"></span></div>
  {#each rows as r (r.i)}
    <div class="row" class:cur={app.editFocus !== null && sameRef(app.editFocus, r.ref)} data-index={r.i}>
      <div class="mrow">
        <span class="mono slotn">{r.i + 1}</span>
        <div class="grow"><PickButton num={dexNum(r.cur.species)} name={speciesName(view, r.cur.species)} label="Static encounter {r.i + 1}" onclick={() => app.openPicker(speciesPicker(view, app.rom.name, { label: 'Static encounter ' + (r.i + 1), current: r.cur.species, original: r.def.value.species, apply: (v) => set(r, v, r.cur.level) }))} /></div>
        {#if r.def.egg}
          <span class="lv egg" title="An egg hatches at its own level">Egg</span>
        {:else}
          <input class="num mono" type="number" min="1" max="100" value={r.cur.level} aria-label="Level of static encounter {r.i + 1}" onchange={(e) => {
            set(r, r.cur.species, clampNum(e.currentTarget.value, 1, 100));
            e.currentTarget.value = String(rows[r.i].cur.level);
          }} />
        {/if}
        <span class="dot end" class:off={!r.edited} aria-hidden="true"></span>
      </div>
      <IssueNote issues={r.issues} />
    </div>
  {/each}
</section>

<style>
  .list { display: flex; flex-direction: column; }
  .row { display: flex; flex-direction: column; gap: 8px; }
  .row:has(:global(.ewarn)), .row:has(:global(.note)) { padding-bottom: 8px; }
  .grow { flex: 1 1 200px; min-width: 0; }
  .lv { width: 84px; flex: none; }
  .egg { font-size: 13px; color: var(--muted); }
  .end { width: 8px; }
</style>
