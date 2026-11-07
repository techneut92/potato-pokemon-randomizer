<script lang="ts">
  import { app } from '../../app.svelte';
  import type { EditableGame } from '../../backend';
  import { itemName, itemTag, money } from '../../edits/items';
  import { issuesOf, sharedIssues } from '../../edits/issues';
  import { itemPicker } from '../../edits/pickers';
  import { copy, EDIT_SECTIONS } from '../../edits/sections';
  import { moveItem, sharedUse, shopCountText, shopRuleText, shopSpace, spaceText } from '../../edits/shops';
  import Icon from '../Icon.svelte';
  import PickButton from '../PickButton.svelte';
  import IssueNote from './IssueNote.svelte';
  import ListPane, { type ListRow } from './ListPane.svelte';

  let { view }: { view: EditableGame } = $props();
  const sec = EDIT_SECTIONS.find((s) => s.id === 'shops')!;
  const size = $derived(view.rules.shops.size);

  const itemsOf = (i: number): number[] => {
    const e = app.edits.get({ kind: 'shop', index: i });
    return e?.kind === 'shop' ? e.items : view.shops[i].value.items;
  };
  const rows: ListRow[] = $derived(view.shops.map((s, i) => ({ id: i, label: s.name, sub: itemsOf(i).map((id) => itemName(view, id)).join(', '), edited: app.edits.edited({ kind: 'shop', index: i }) })));
  const i = $derived(Math.min(Math.max(0, app.editSel.shops ?? 0), view.shops.length - 1));
  const def = $derived(view.shops[i]);
  const ref = $derived({ kind: 'shop', index: i } as const);
  const items = $derived(itemsOf(i));

  const space = $derived(shopSpace(size, itemsOf, (k) => app.edits.edited({ kind: 'shop', index: k })));
  const spare = $derived(size.kind === 'shared' ? size.bytes - sharedUse(size, (k) => view.shops[k].value.items, () => true) : null);
  const meterColor = $derived(!space ? '' : space.free > 0 ? 'var(--ok)' : space.free === 0 ? 'var(--accent)' : 'var(--amber)');

  const priceOf = (id: number): number | null => {
    const e = app.edits.get({ kind: 'itemPrice', item: id });
    if (e?.kind === 'itemPrice') return e.price;
    return view.itemPrices.find((p) => p.item === id)?.price ?? null;
  };

  const setItems = (change: (items: number[]) => void) => {
    const next = copy(items);
    change(next);
    app.setEdit({ kind: 'shop', index: i, items: next }, { kind: 'shop', ...def.value });
  };
  // Adding is always possible: a shop the game can't store that way gets a message to fix before saving.
  const add = () => app.openPicker(itemPicker(view, app.rom.name, { label: 'New item for ' + def.name, current: null, apply: (v) => v !== null && setItems((x) => x.push(v)) }));
</script>

<div class="split">
  <ListPane label={sec.label} hint={sec.hint ?? 'Search'} plural={sec.plural ?? ''} {rows} selected={i} onpick={(id) => app.selectEdit('shops', id)} />

  <section class="card detail" aria-label="Edit selected">
    <div class="dhead">
      <div class="dtitle">
        <h2 class="disp">{def.name}</h2>
        <div class="chips">
          <span class="chip">{shopCountText(items.length, def.value.items.length, size)}</span>
          {#if def.special}<span class="chip">Special counter</span>{/if}
          {#if !def.mainGame}<span class="chip">After the story</span>{/if}
        </div>
      </div>
      <button class="btn ghost sm" disabled={!app.edits.edited(ref)} onclick={() => app.edits.revert(ref)}>Revert</button>
    </div>

    <div class="rows">
      {#each items as id, k (k)}
        {@const name = itemName(view, id)}
        {@const price = priceOf(id)}
        <div class="mrow">
          <span class="mono slotn">{k + 1}</span>
          <div class="grow"><PickButton num={itemTag(name)} {name} label="Item {k + 1}" onclick={() => app.openPicker(itemPicker(view, app.rom.name, { label: 'Item ' + (k + 1), current: id, original: def.value.items[k], apply: (v) => v !== null && setItems((x) => (x[k] = v)) }))} /></div>
          <span class="price">
            <span class="amount"><span class="dot" class:off={!app.edits.edited({ kind: 'itemPrice', item: id })} aria-hidden="true"></span><span class="mono">{price === null ? 'No price' : money(price)}</span></span>
            {#if price !== null}<button class="linkbtn small" aria-label="Change the price of {name}" onclick={() => app.goToEdit('items', { kind: 'itemPrice', item: id })}>Change price</button>{/if}
          </span>
          <span class="moves">
            <button class="btn ghost ico" aria-label="Move {name} up" disabled={k === 0} onclick={() => setItems((x) => x.splice(0, x.length, ...moveItem(x, k, k - 1)))}><Icon d="M6 15l6-6 6 6" size={16} /></button>
            <button class="btn ghost ico" aria-label="Move {name} down" disabled={k === items.length - 1} onclick={() => setItems((x) => x.splice(0, x.length, ...moveItem(x, k, k + 1)))}><Icon d="M6 9l6 6 6-6" size={16} /></button>
            <button class="btn ghost ico" aria-label="Remove {name}" onclick={() => setItems((x) => x.splice(k, 1))}><Icon d="M6 6l12 12M18 6L6 18" size={16} /></button>
          </span>
        </div>
      {/each}
    </div>
    <div><button class="btn sm" onclick={add}>Add item…</button></div>
    <div class="foot">
      {#if space}
        <div class="meterhead"><span>Spare shop space</span><span class="mono">{spaceText(space)}</span></div>
        <div class="meter" role="meter" aria-label="Spare shop space" aria-valuemin={0} aria-valuemax={space.cap} aria-valuenow={space.used}><div class="fill" style:width="{space.cap ? Math.min(100, Math.round((space.used / space.cap) * 100)) : 0}%" style:background={meterColor}></div></div>
      {/if}
      <span class="hint">{shopRuleText(size, view.generation, spare)}</span>
      <span class="hint">Prices belong to the item, not the shop: a price change applies to every shop in the game.</span>
      <IssueNote issues={[...issuesOf(app.editIssues, ref), ...sharedIssues(app.editIssues, 'shops')]} />
    </div>
  </section>
</div>

<style>
  .rows { display: flex; flex-direction: column; }
  .grow { flex: 1 1 200px; min-width: 0; }
  .price { flex: none; min-width: 96px; display: inline-flex; flex-direction: column; align-items: flex-end; }
  .amount { display: inline-flex; align-items: center; gap: 6px; font-size: 13px; }
  .small { font-size: 12px; font-weight: 500; color: var(--muted); }
  .moves { display: inline-flex; gap: 2px; }
  .foot { display: flex; flex-direction: column; gap: 8px; }
  .meterhead { display: flex; justify-content: space-between; gap: 12px; font-size: 13px; color: var(--muted); }
  .fill { height: 100%; border-radius: 999px; }
</style>
