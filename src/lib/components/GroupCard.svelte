<script lang="ts">
  import type { GroupView } from '../options/engine';
  import { app } from '../app.svelte';
  import OptionRow from './OptionRow.svelte';
  import Pills from './Pills.svelte';
  import Tip from './Tip.svelte';
  import TierMatrix from './TierMatrix.svelte';
  import PlayerGraphics from './PlayerGraphics.svelte';

  let { g }: { g: GroupView } = $props();
  const hid = $derived('h-' + g.key);
</script>

<section class="card" class:off={g.unsupported} class:wide={g.wide} aria-labelledby={hid}>
  <div class="title">
    <h2 class="disp" id={hid}>{g.title}</h2>
    {#if g.tag}<span class="badge">{g.tag}</span>{/if}
    {#if g.changed}<span class="badge hot">{g.changed}</span>{/if}
  </div>

  {#if g.modes.length}
    <div class="modes"><Pills choices={g.modes} label={g.title} onpick={(i) => app.set(g.key, i)} /></div>
    {#if g.modeTip}<p class="mdesc">{g.modeTip}</p>{/if}
  {/if}

  <div class="opts" class:wide={g.wide}>
    {#each g.opts as o (o.key)}<OptionRow {o} />{/each}
  </div>

  {#if g.matrix}<TierMatrix rows={g.matrix} />{/if}
  {#if g.cpg}<PlayerGraphics disabled={g.mode === 0} />{/if}

  {#if g.tweaks?.length}
    <div class="tweaks">
      {#each g.tweaks as t (t.key)}
        <label class="row">
          <span class="lab"><span>{t.label}</span><Tip text={t.tip} label={t.label} /></span>
          <input type="checkbox" class="sw" checked={t.on} onchange={() => app.set(t.key, !t.on)} />
        </label>
      {/each}
    </div>
  {/if}

  {#if g.note}<p class="note">{g.note}</p>{/if}
</section>

<style>
  .wide {
    grid-column: 1 / -1;
  }
  .title {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 14px;
  }
  h2 {
    margin: 0;
    font-size: 18px;
  }
  .badge:not(.hot) {
    margin-left: 0;
  }
  .modes {
    margin-bottom: 12px;
  }
  .tweaks {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 280px), 1fr));
    column-gap: 28px;
  }
  .note {
    margin: 14px 0 0;
    font-size: 13px;
    color: var(--muted);
  }
</style>
