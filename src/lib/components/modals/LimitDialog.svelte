<script lang="ts">
  import { app } from '../../app.svelte';
  import Dialog from './Dialog.svelte';

  /** Only the generations up to the game's own can be picked. */
  const gens = app.rom?.genNum ?? 7;
  let inc = $state([...app.inc]);
  let related = $state(app.related);

  const apply = () => {
    app.inc = inc;
    app.related = related;
    app.modal = null;
    app.set('gen.limit.on', true);
    const any = inc.some((v, i) => v && i < gens);
    app.say(any ? 'Pokémon limited to the selected generations.' : 'No generations selected — choose at least one before randomizing.', any ? 'ok' : 'err');
  };
</script>

<Dialog id="d-limit" title="Choose Pokémon to allow" lede="Only Pokémon from the selected generations can appear anywhere in the randomized game.">
  <div>
    <div class="gen header"><span class="left">Generation</span><span>Include</span></div>
    {#each inc.slice(0, gens) as _, i (i)}
      <div class="gen">
        <span class="left">Generation {i + 1}</span>
        <span><input type="checkbox" class="chk" bind:checked={() => !!inc[i], (v) => (inc[i] = v ? 1 : 0)} aria-label={'Include Generation ' + (i + 1)} /></span>
      </div>
    {/each}
    <label class="gen related">
      <span class="left">Also allow their evolutionary relatives from other generations</span>
      <span><input type="checkbox" class="chk" bind:checked={related} /></span>
    </label>
  </div>
  {#snippet footer()}
    <button class="btn ghost" onclick={() => app.closeModal()}>Cancel</button>
    <button class="btn primary lg" onclick={apply}>Apply</button>
  {/snippet}
</Dialog>

<style>
  .gen { grid-template-columns: 1fr 90px; }
  .header { border-top: 0; font-size: 12px; color: var(--faint); font-weight: 600; text-transform: uppercase; letter-spacing: 0.06em; }
  .left { justify-content: flex-start !important; }
  .related { cursor: pointer; }
</style>
