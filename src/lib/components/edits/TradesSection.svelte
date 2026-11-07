<script lang="ts">
  import { app } from '../../app.svelte';
  import type { EditableGame } from '../../backend';
  import type { TradeEdit } from '../../generated/TradeEdit';
  import { dexNum, speciesName } from '../../edits/changes';
  import { clampNum, itemName, itemTag } from '../../edits/items';
  import { issuesOf } from '../../edits/issues';
  import { itemPicker, speciesPicker } from '../../edits/pickers';
  import { copy, sameRef } from '../../edits/sections';
  import { ivLabels, ivLegend, readIv, shownIv, tradeNote } from '../../edits/trades';
  import PickButton from '../PickButton.svelte';
  import IssueNote from './IssueNote.svelte';

  let { view }: { view: EditableGame } = $props();
  const r = $derived(view.rules.trades);
  const ANY = 'Any Pokémon';

  const cards = $derived(
    view.trades.map((def, i) => {
      const ref = { kind: 'trade', index: i } as const;
      const e = app.edits.get(ref);
      return { i, ref, def, cur: e?.kind === 'trade' ? e : def, edited: !!e, issues: issuesOf(app.editIssues, ref) };
    })
  );
  const set = (c: (typeof cards)[number], change: (t: TradeEdit) => void) => {
    const next = copy(c.cur);
    change(next);
    app.setEdit({ kind: 'trade', ...next }, { kind: 'trade', ...c.def });
  };
  const sp = (n: number | null) => (n === null ? ANY : speciesName(view, n));

  // A jump from the Changes list or a warning brings its trade into view.
  let grid: HTMLElement;
  $effect(() => {
    const focus = app.editFocus;
    if (focus?.kind !== 'trade') return;
    const el = grid.querySelector<HTMLElement>(`[data-index="${focus.index}"]`);
    el?.scrollIntoView({ block: 'nearest' });
    el?.querySelector('button')?.focus();
  });
</script>

<div class="grid" bind:this={grid}>
  {#each cards as c (c.i)}
    {@const ask = 'What trade ' + (c.i + 1) + ' asks for'}
    {@const give = 'What trade ' + (c.i + 1) + ' gives'}
    <section class="card trade" class:cur={app.editFocus !== null && sameRef(app.editFocus, c.ref)} data-index={c.i} aria-label="Trade {c.i + 1}">
      <div class="top">
        <h2 class="disp">Trade {c.i + 1} <span class="dot" class:off={!c.edited} aria-hidden="true"></span></h2>
        <button class="btn ghost sm" disabled={!c.edited} onclick={() => app.edits.revert(c.ref)}>Revert</button>
      </div>
      <div class="fgrid">
        <div class="fld">
          <span class="flab">You give</span>
          <PickButton num={c.cur.requested === null ? '' : dexNum(c.cur.requested)} name={sp(c.cur.requested)} label={ask} onclick={() => app.openPicker(c.def.requested === null ? speciesPicker(view, app.rom.name, { label: ask, current: c.cur.requested, original: c.def.requested, apply: (v) => set(c, (t) => (t.requested = v)) }, ANY) : speciesPicker(view, app.rom.name, { label: ask, current: c.cur.requested ?? 0, original: c.def.requested, apply: (v) => set(c, (t) => (t.requested = v)) }))} />
        </div>
        <div class="fld">
          <span class="flab">You get</span>
          <PickButton num={dexNum(c.cur.given)} name={sp(c.cur.given)} label={give} onclick={() => app.openPicker(speciesPicker(view, app.rom.name, { label: give, current: c.cur.given, original: c.def.given, apply: (v) => set(c, (t) => (t.given = v)) }))} />
        </div>
        <label class="fld"><span class="flab">Nickname</span><input class="txt mono" maxlength={r.maxNicknameLength} disabled={!r.texts} value={c.cur.nickname} onchange={(e) => set(c, (t) => (t.nickname = e.currentTarget.value.slice(0, r.maxNicknameLength)))} /></label>
        {#if r.originalTrainer}
          <label class="fld"><span class="flab">Original trainer</span><input class="txt mono" maxlength={r.maxOtNameLength} disabled={!r.texts} value={c.cur.otName} onchange={(e) => set(c, (t) => (t.otName = e.currentTarget.value.slice(0, r.maxOtNameLength)))} /></label>
          <label class="fld"><span class="flab">Trainer ID</span><input class="txt mono" type="number" min="0" max={r.maxOtId} value={c.cur.otId} onchange={(e) => {
            set(c, (t) => (t.otId = clampNum(e.currentTarget.value, 0, r.maxOtId)));
            e.currentTarget.value = String(cards[c.i].cur.otId);
          }} /></label>
        {/if}
        {#if r.heldItem}
          <div class="fld">
            <span class="flab">Held item</span>
            <PickButton num={c.cur.heldItem === null ? '' : itemTag(itemName(view, c.cur.heldItem))} name={itemName(view, c.cur.heldItem)} label="Held item of trade {c.i + 1}" onclick={() => app.openPicker(itemPicker(view, app.rom.name, { label: 'Held item of trade ' + (c.i + 1), current: c.cur.heldItem, original: c.def.heldItem, apply: (v) => set(c, (t) => (t.heldItem = v)) }, true))} />
          </div>
        {/if}
      </div>
      {#if r.ivs > 0 && c.cur.ivs.length}
        {@const labels = ivLabels(c.cur.ivs.length)}
        <fieldset class="ivset">
          <legend class="flab">{ivLegend(r)}</legend>
          <div class="ivgrid">
            {#each c.cur.ivs as iv, k (k)}
              <label class="fld iv">
                <span class="ivl">{labels[k] ?? 'IV ' + (k + 1)}</span>
                <input class="num mono" type="number" min="0" max={r.maxIv} placeholder={r.randomIv !== null ? 'Any' : ''} value={shownIv(iv, r)} aria-label="{r.maxIv === 15 ? 'DV' : 'IV'} {labels[k] ?? k + 1} of trade {c.i + 1}{r.randomIv !== null ? ', empty means random' : ''}" onchange={(e) => {
                  set(c, (t) => (t.ivs[k] = readIv(e.currentTarget.value, r)));
                  e.currentTarget.value = shownIv(cards[c.i].cur.ivs[k], r);
                }} />
              </label>
            {/each}
          </div>
        </fieldset>
      {/if}
      <IssueNote issues={c.issues} />
    </section>
  {/each}
</div>
<p class="hint">{tradeNote(r)}</p>

<style>
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 420px), 1fr)); gap: 16px; }
  .trade { display: flex; flex-direction: column; gap: 14px; min-width: 0; }
  .trade.cur { box-shadow: inset 0 0 0 1px var(--accent); }
  .top { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
  h2 { margin: 0; font-size: 18px; display: flex; align-items: center; gap: 10px; }
  .iv { gap: 4px; }
  .iv input { width: 100%; }
  .ivl { font-size: 12px; color: var(--faint); }
  .hint { margin: 0; }
</style>
