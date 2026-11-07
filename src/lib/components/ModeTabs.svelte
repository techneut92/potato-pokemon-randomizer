<script lang="ts">
  import { app } from '../app.svelte';
</script>

<nav aria-label="Mode" class="seg">
  <button class="btn sm" class:on={app.mode === 'options'} aria-current={app.mode === 'options' ? 'page' : undefined} onclick={() => app.showOptions()}><span class="long">Randomizer options</span><span class="short">Options</span></button>
  <button class="btn sm" class:on={app.mode === 'edits'} aria-current={app.mode === 'edits' ? 'page' : undefined} onclick={() => app.showEdits()}>
    <span class="long">Manual edits</span><span class="short">Edits</span>
    {#if app.edits.count}<span class="badge hot" aria-label="{app.edits.count} edited">{app.edits.count}</span>{/if}
  </button>
</nav>

<style>
  .seg { flex: none; }
  .badge { margin-left: 0; }
  /* Narrow windows: shorter labels keep the header on one row. */
  .short { display: none; }
  @media (max-width: 900px) {
    .seg .btn { padding: 0 10px; }
  }
  @media (max-width: 560px) {
    .long { display: none; }
    .short { display: inline; }
  }
  /* Small windows have a lower title bar: smaller tabs keep some room above and below. */
  @media (max-width: 560px), (max-height: 720px) {
    .seg { padding: 3px; }
    .seg .btn { min-height: 30px; }
  }
</style>
