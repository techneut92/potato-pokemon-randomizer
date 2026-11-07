<script lang="ts">
  import { app } from '../../app.svelte';
  import { matches, type PickValue } from '../../picker';
  import Dialog from './Dialog.svelte';

  const pick = app.pick!;
  let query = $state('');
  let filter = $state(0);

  const found = $derived.by(() => {
    const test = pick.filters?.options[filter]?.test ?? (() => true);
    return pick.entries.filter((e) => test(e) && matches(e, query));
  });
  const isCurrent = (v: PickValue) => String(v) === String(pick.current);

  const choose = (v: PickValue) => {
    pick.apply(v);
    app.modal = null;
  };
  const onKey = (e: KeyboardEvent) => {
    if (e.key === 'Enter' && found.length) choose(found[0].value);
  };
</script>

<Dialog id="d-pick" width={880} title={pick.title} lede={pick.lede}>
  {#snippet toolbar()}
    {#if pick.hint}
      <input class="inp" type="search" data-autofocus placeholder={pick.hint} aria-label={pick.hint} bind:value={query} onkeydown={onKey} />
    {/if}
    {#if pick.filters && pick.filters.options.length > 1}
      <div class="pills" role="radiogroup" aria-label={pick.filters.label}>
        {#each pick.filters.options as f, i (f.label)}
          <button class="pill" class:cur={i === filter} role="radio" aria-checked={i === filter} onclick={() => (filter = i)}>{f.label}</button>
        {/each}
      </div>
    {/if}
  {/snippet}
  <div class="dexgrid">
    {#each pick.specials as x (x.num + String(x.value))}
      <button class="dexbtn orig" class:cur={isCurrent(x.value)} onclick={() => choose(x.value)}><span class="mono dexno">{x.num}</span><span class="dl">{x.label}</span></button>
    {/each}
    {#each found as x (String(x.value))}
      <button class="dexbtn" class:cur={isCurrent(x.value)} onclick={() => choose(x.value)}><span class="mono dexno">{x.num}</span><span class="dl">{x.label}</span></button>
    {/each}
  </div>
  {#snippet footer()}
    <span class="count">{found.length} of {pick.entries.length} {pick.noun}{found.length && pick.hint ? ' · Enter picks the first' : ''}</span>
    <button class="btn ghost" onclick={() => app.closeModal()}>Cancel</button>
  {/snippet}
</Dialog>

<style>
  .dexno { color: var(--faint); font-size: 12px; }
  .count { margin-right: auto; align-self: center; font-size: 13px; color: var(--muted); }
</style>
