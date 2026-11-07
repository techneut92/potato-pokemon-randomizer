<script lang="ts">
  import { app } from '../app.svelte';

  let { disabled }: { disabled: boolean } = $props();

  const packs = $derived(app.rom.cpgPacks);
  const pickedId = $derived(app.view.val['cpg.pack'] as string | null);
  const picked = $derived(packs.find((p) => p.pack.id === pickedId) ?? packs[0]);
  const CATEGORIES = { pokemon: 'Pokémon', games: 'Games', other: 'Other' } as const;

  function pickRandom() {
    const others = packs.filter((p) => p !== picked);
    if (others.length) app.set('cpg.pack', others[Math.floor(Math.random() * others.length)].pack.id);
  }
</script>

<div class="wrap">
  <div class="list">
    <div class="head">
      <span class="sub">Graphics packs</span>
      <button class="btn" disabled={disabled || packs.length < 2} onclick={pickRandom}>Random</button>
    </div>
    {#if packs.length}
      <div class="grid">
        {#each packs as p (p.pack.id)}
          <button class="tile" class:cur={p === picked} aria-pressed={p === picked} {disabled} onclick={() => app.set('cpg.pack', p.pack.id)}>
            {#if p.samples[0]}<img src={p.samples[0]} alt="" width="48" height="48" />{/if}
            {p.pack.name}
          </button>
        {/each}
      </div>
    {:else}
      <p class="none">No graphics packs for this game.</p>
    {/if}
  </div>
  {#if picked}
    <div class="info">
      <div class="preview" aria-label="Sample images of {picked.pack.name}">
        {#each picked.samples as src, i (i)}<img {src} alt="" />{/each}
      </div>
      <dl>
        <dt>Name</dt><dd class="strong">{picked.pack.name}</dd>
        {#if picked.pack.description}<dt>Description</dt><dd>{picked.pack.description}</dd>{/if}
        {#if picked.pack.from}<dt>From</dt><dd>{picked.pack.from}</dd>{/if}
        {#if picked.pack.creator}<dt>Original creator</dt><dd>{picked.pack.creator}</dd>{/if}
        {#if picked.pack.adapter}<dt>Adapter</dt><dd>{picked.pack.adapter}</dd>{/if}
        {#if picked.pack.category}<dt>Category</dt><dd>{CATEGORIES[picked.pack.category]}</dd>{/if}
      </dl>
    </div>
  {/if}
</div>

<style>
  .wrap { display: flex; flex-wrap: wrap; gap: 20px; margin-top: 16px; }
  .list { flex: 999 1 420px; min-width: 0; }
  .head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 10px; }
  .head .sub { margin: 0; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(104px, 1fr)); gap: 8px; }
  .tile img { display: block; margin: 0 auto 4px; image-rendering: pixelated; }
  .none { color: var(--faint); font-size: 13px; margin: 0; }
  .info { flex: 1 1 240px; padding: 16px; border-radius: 12px; background: var(--panel2); display: flex; flex-direction: column; gap: 10px; }
  .preview { min-height: 128px; border-radius: 10px; border: 1px dashed var(--line2); display: flex; align-items: flex-end; justify-content: center; gap: 12px; padding: 8px; }
  .preview img { image-rendering: pixelated; height: 96px; width: auto; }
  dl { margin: 0; display: grid; grid-template-columns: auto 1fr; gap: 6px 12px; font-size: 13px; }
  dt { color: var(--faint); }
  dd { margin: 0; }
  .strong { font-weight: 600; }
</style>
