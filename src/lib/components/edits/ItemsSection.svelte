<script lang="ts">
  import { app } from '../../app.svelte';
  import type { EditableGame } from '../../backend';
  import { catLabel, ITEM_CATS, isTm, itemCat, itemName, money, priceLimit, priceRuleText, readPrice } from '../../edits/items';
  import { issuesOf } from '../../edits/issues';
  import { EDIT_SECTIONS } from '../../edits/sections';
  import Icon from '../Icon.svelte';
  import IssueNote from './IssueNote.svelte';
  import ListPane, { type ListRow } from './ListPane.svelte';

  let { view }: { view: EditableGame } = $props();
  const sec = EDIT_SECTIONS.find((s) => s.id === 'items')!;

  const priceOf = (id: number, original: number): number => {
    const e = app.edits.get({ kind: 'itemPrice', item: id });
    return e?.kind === 'itemPrice' ? e.price : original;
  };
  const rows: ListRow[] = $derived(
    view.itemPrices.map((p) => {
      const name = itemName(view, p.item);
      const v = priceOf(p.item, p.price);
      return { id: p.item, label: name, sub: 'Buy ' + money(v) + ' · Sell ' + money(Math.floor(v / 2)), edited: app.edits.edited({ kind: 'itemPrice', item: p.item }), tags: [itemCat(name)] };
    })
  );
  const filters = $derived(ITEM_CATS.filter(([id]) => id === 'all' || rows.some((r) => r.tags?.includes(id))));

  const def = $derived(view.itemPrices.find((p) => p.item === app.editSel.items) ?? view.itemPrices[0]);
  const id = $derived(def.item);
  const ref = $derived({ kind: 'itemPrice', item: id } as const);
  const name = $derived(itemName(view, id));
  const price = $derived(priceOf(id, def.price));
  const limit = $derived(priceLimit(view, id));
  const soldIn = $derived(
    view.shops
      .filter((s, k) => {
        const e = app.edits.get({ kind: 'shop', index: k });
        return (e?.kind === 'shop' ? e.items : s.value.items).includes(id);
      })
      .map((s) => s.name)
  );
  const soldText = $derived(soldIn.length ? 'Sold in ' + (soldIn.length === 1 ? '1 shop' : soldIn.length + ' shops') + ': ' + soldIn.join(', ') + '.' : 'No shop sells this item right now.');
</script>

<div class="split">
  <ListPane label={sec.label} hint={sec.hint ?? 'Search'} plural={sec.plural ?? ''} {rows} selected={id} {filters} filterLabel="Item category" searchSub={false} onpick={(item) => app.selectEdit('items', item)} />

  <section class="card detail" aria-label="Edit selected">
    <div class="dhead">
      <div class="dtitle">
        <h2 class="disp">{name}</h2>
        <div class="chips"><span class="chip">{catLabel(itemCat(name))}</span><span class="chip">Was {money(def.price)}</span></div>
      </div>
      <button class="btn ghost sm" disabled={!app.edits.edited(ref)} onclick={() => app.edits.revert(ref)}>Revert</button>
    </div>
    <div class="prices">
      <label class="fld fixed"><span class="flab">Buy price</span><input class="num mono buy" type="number" min="0" max={limit.max} step={limit.step} value={price} onchange={(e) => {
        app.setEdit({ kind: 'itemPrice', item: id, price: readPrice(e.currentTarget.value) }, { kind: 'itemPrice', ...def });
        e.currentTarget.value = String(price);
      }} /></label>
      <div class="fld fixed"><span class="flab" id="sell-lab">Sell price</span><output class="mono sell" aria-labelledby="sell-lab">{money(Math.floor(price / 2))}</output></div>
      <span class="hint half">Half the buy price, set by the game.</span>
    </div>
    <IssueNote issues={issuesOf(app.editIssues, ref)} />
    <span class="sold">{soldText}</span>
    <div class="note">
      <Icon d="M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 11v5M12 8h.01" size={16} />
      <span>{priceRuleText(limit, isTm(view, id))} The price applies in every shop that sells this item; selling gives back half, rounded down.</span>
    </div>
  </section>
</div>

<style>
  .prices { display: flex; gap: 16px; align-items: flex-end; flex-wrap: wrap; }
  .fixed { flex: none; }
  .buy { width: 128px; }
  .sell { display: flex; align-items: center; min-height: 40px; font-size: 14px; color: var(--muted); }
  .half { padding-bottom: 10px; }
  .sold { font-size: 13px; color: var(--muted); }
</style>
