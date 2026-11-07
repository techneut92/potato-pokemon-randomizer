<script lang="ts">
  import { app } from '../../app.svelte';
  import Dialog from './Dialog.svelte';
  import Field from '../Field.svelte';

  const b = app.batch;
  const off = $derived(!b.on);
  const done = () => {
    app.saveBatch();
    app.closeModal();
  };
  const example = $derived(`Example: ${b.prefix}${b.start}${app.rom.ext} … ${b.prefix}${b.start + b.n - 1}${app.rom.ext}`);
</script>

<Dialog id="d-batch" title="Batch Randomization" lede="Produce several randomized ROMs in one go, each with its own seed. Can’t be combined with Race Mode.">
  <label class="row first"><span class="strong">Enable batch randomization</span><input type="checkbox" class="sw" bind:checked={b.on} /></label>
  <div class="grid">
    <label class="stack"><span class="lbl">Starting index</span><input class="inp" type="number" min="0" bind:value={b.start} disabled={off} /></label>
    <label class="stack"><span class="lbl">Number of randomized ROMs</span><input class="inp" type="number" min="1" max="999" bind:value={b.n} disabled={off} /></label>
    <div class="span"><Field label="Output directory" value={b.dir || 'Asked for when randomizing'}><button class="btn" disabled={off} onclick={() => app.pickBatchDir()}>Browse…</button></Field></div>
    <label class="stack"><span class="lbl">File name prefix</span><input class="inp mono" bind:value={b.prefix} disabled={off} /></label>
    <label class="stack"><span class="lbl">Log file ending</span><input class="inp mono" bind:value={b.logEnding} disabled={off || !b.logs} /></label>
    <label class="row span"><span>Automatically advance index after batch</span><input type="checkbox" class="sw" bind:checked={b.adv} disabled={off} /></label>
    <label class="row span"><span>Generate log files</span><input type="checkbox" class="sw" bind:checked={b.logs} disabled={off} /></label>
  </div>
  <p class="mono example">{example}</p>
  {#snippet footer()}<button class="btn primary lg" onclick={done}>Done</button>{/snippet}
</Dialog>

<style>
  .first { border-top: 0; }
  .strong { font-weight: 600; }
  .grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 14px; }
  @container sheet (max-width: 519px) {
    .grid { grid-template-columns: 1fr; }
  }
  .stack { display: flex; flex-direction: column; gap: 8px; }
  .span { grid-column: 1 / -1; }
  .example { margin: 0; font-size: 12px; color: var(--faint); }
</style>
