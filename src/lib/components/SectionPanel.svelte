<script lang="ts">
  import { app } from '../app.svelte';
  import GroupCard from './GroupCard.svelte';

  const cur = $derived(app.view.sections.find((s) => s.id === app.sec) ?? app.view.sections[0]);
  const hits = $derived(app.view.hits.slice(0, 48));

  // The panel scrolls on its own, so a new section or search starts at the top.
  let pane: HTMLElement;
  $effect(() => {
    void app.sec;
    void app.query;
    pane.scrollTop = 0;
  });
</script>

<main bind:this={pane}>
  {#if app.query.trim()}
    <div class="results">
      <h1 class="disp">Results for “{app.query}”</h1>
      <p class="lede">{hits.length ? app.view.hits.length + ' options found. Select one to open its category.' : 'No options match. Try another word.'}</p>
      <div class="hits">
        {#each hits as h, i (i)}
          <button class="hit" onclick={() => app.goTo(h.section)}>{h.label}<small>{h.where}</small></button>
        {/each}
      </div>
    </div>
  {:else}
    <div class="head">
      <div class="intro">
        <h1 class="disp">{cur.label}</h1>
        <p class="lede">{cur.desc}</p>
      </div>
      <div class="actions">
        <span class="badge count" class:hot={cur.count > 0}>{cur.count} changed</span>
        <button class="btn ghost" onclick={() => app.resetSection(cur.keys)}>Reset section</button>
      </div>
    </div>
    <div class="grid">
      {#each cur.groups as g (g.key)}<GroupCard {g} />{/each}
    </div>
  {/if}
</main>

<style>
  main { flex: 1; min-width: 0; overflow-y: auto; padding: var(--padtop) var(--padx) 40px; scrollbar-width: thin; }
  h1 { margin: 0; font-size: var(--h1); }
  .results { display: flex; flex-direction: column; gap: 16px; max-width: 880px; }
  .results h1 { font-size: 28px; }
  .hits { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 260px), 1fr)); gap: 10px; }
  /* The head stays one row while it fits: the intro shrinks before the actions wrap. */
  .head { display: flex; align-items: flex-end; justify-content: space-between; gap: 12px 16px; flex-wrap: wrap; margin-bottom: 24px; }
  .intro { flex: 1 1 240px; min-width: 0; display: flex; flex-direction: column; gap: 6px; max-width: 720px; }
  .intro .lede { font-size: 15px; }
  .actions { display: flex; gap: 8px; align-items: center; }
  .count { height: 28px; padding: 0 12px; }
  @media (max-height: 720px) {
    .head { margin-bottom: 16px; }
    .intro .lede { font-size: 14px; }
  }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 420px), 1fr)); gap: 16px; align-items: start; }
</style>
