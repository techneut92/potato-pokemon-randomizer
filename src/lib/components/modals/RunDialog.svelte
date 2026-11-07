<script lang="ts">
  import { app } from '../../app.svelte';
  import Dialog from './Dialog.svelte';
  import Icon from '../Icon.svelte';

  const idx = $derived(app.runSteps.indexOf(app.run.step));
</script>

<Dialog id="d-run" title="Randomizing…" lede={app.rom.name}>
  <div>
    <div class="bar-head"><span>{app.run.step}</span><span class="mono">{app.run.percent}%</span></div>
    <div class="bar" role="progressbar" aria-valuenow={app.run.percent} aria-valuemin={0} aria-valuemax={100} aria-label="Randomization progress"><div style:width="{app.run.percent}%"></div></div>
  </div>
  <div class="steps">
    {#each app.runSteps as s, i (s)}
      <div class="step" class:past={i < idx} class:now={i === idx}>
        <Icon d={i < idx ? 'M5 12l5 5L20 7' : i === idx ? 'M12 3a9 9 0 1 0 9 9' : 'M12 12h.01'} stroke={2.4} />
        {s}
      </div>
    {/each}
  </div>
  {#snippet footer()}
    <span class="batch">{app.run.count ? `Saving ${app.run.rom} of ${app.run.count}` : 'Single ROM'}</span>
    <button class="btn ghost" onclick={() => app.cancelRun()}>Cancel</button>
  {/snippet}
</Dialog>

<style>
  .bar-head { display: flex; justify-content: space-between; font-size: 13px; color: var(--muted); margin-bottom: 8px; }
  .bar { height: 10px; border-radius: 999px; background: var(--panel2); overflow: hidden; }
  .bar div { height: 100%; background: var(--accent); border-radius: 999px; }
  .steps { display: flex; flex-direction: column; gap: 2px; }
  .step { display: flex; align-items: center; gap: 12px; font-size: 14px; min-height: 30px; color: var(--faint); }
  .step.past { color: var(--muted); }
  .step.now { color: var(--text); }
  .batch { margin-right: auto; align-self: center; font-size: 13px; color: var(--muted); }
</style>
