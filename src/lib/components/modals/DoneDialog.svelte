<script lang="ts">
  import { app } from '../../app.svelte';
  import Dialog from './Dialog.svelte';
  import CopyField from './CopyField.svelte';

  const batch = $derived(app.run.batch);
  const title = $derived(batch ? `${batch.roms.length} ROM${batch.roms.length === 1 ? '' : 's'} randomized` : 'Your game has been randomized');
  const lede = $derived(batch ? 'Saved to ' + batch.dir : app.run.layeredFs ? 'Saved as LayeredFS folder: ' + app.run.output : 'Saved to ' + app.run.output);
  const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;
  const changed = $derived(app.run.starters.filter((s) => s.before !== s.after));
  const held = $derived(app.run.starterHeldItems.filter((s) => s.before !== s.after));
  /** Eight upper-case hexadecimal digits. */
  const checkValue = $derived((app.run.checkValue >>> 0).toString(16).toUpperCase().padStart(8, '0'));
</script>

<Dialog id="d-done" {title} {lede}>
  {#if batch}
  {#if batch.cancelled}<p class="lede">The batch was cancelled; the ROMs below were saved.</p>{/if}
  <div class="changes">
    <span class="strong">ROMs and their seeds</span>
    <ul>
      {#each batch.roms as r (r.output)}
        <li><span class="mono">{fileName(r.output)}</span> <span class="before">· seed {r.seed}{r.log ? ' · log ' + fileName(r.log) : ''}</span></li>
      {/each}
    </ul>
  </div>
  {#if app.run.settingsString}<CopyField label="Settings string" value={app.run.settingsString} />{/if}
  {:else}
  {#if changed.length}
    <div class="changes">
      <span class="strong">New starters</span>
      <ul>
        {#each app.run.starters as s, i (i)}
          <li><span class="before">{s.before}</span> → <span class="strong">{s.after}</span></li>
        {/each}
      </ul>
    </div>
  {/if}
  {#if held.length}
    <div class="changes">
      <span class="strong">Starter held item</span>
      <ul>
        {#each held as s, i (i)}
          <li><span class="before">{s.before}</span> → <span class="strong">{s.after}</span></li>
        {/each}
      </ul>
    </div>
  {/if}
  {#if app.run.baseStatsChanged}
    <div class="changes">
      <span class="strong">Base statistics</span>
      <span>{app.run.baseStatsChanged} of {app.run.species} species changed.</span>
    </div>
  {/if}
  {#if app.run.typesChanged}
    <div class="changes">
      <span class="strong">Types</span>
      <span>{app.run.typesChanged} of {app.run.species} species changed.</span>
    </div>
  {/if}
  {#if app.run.abilitiesChanged}
    <div class="changes">
      <span class="strong">Abilities</span>
      <span>{app.run.abilitiesChanged} of {app.run.species} species changed.</span>
    </div>
  {/if}
  {#if app.run.expCurvesChanged}
    <div class="changes">
      <span class="strong">EXP curves</span>
      <span>{app.run.expCurvesChanged} of {app.run.species} species changed.</span>
    </div>
  {/if}
  {#if app.run.evolutionsChanged}
    <div class="changes">
      <span class="strong">Evolutions</span>
      <span>{app.run.evolutionsChanged} of {app.run.species} species changed.</span>
    </div>
  {/if}
  {#if app.run.typePairsChanged}
    <div class="changes">
      <span class="strong">Type effectiveness</span>
      <span>{app.run.typePairsChanged} of {app.run.typePairs} pairs changed.</span>
    </div>
  {/if}
  {#if app.run.movesChanged}
    <div class="changes">
      <span class="strong">Moves</span>
      <span>{app.run.movesChanged} of {app.run.moves} changed.</span>
    </div>
  {/if}
  {#if app.run.tmsChanged}
    <div class="changes">
      <span class="strong">TM moves</span>
      <span>{app.run.tmsChanged} of {app.run.tms} changed.</span>
    </div>
  {/if}
  {#if app.run.tmHmCompatChanged}
    <div class="changes">
      <span class="strong">TM/HM compatibility</span>
      <span>{app.run.tmHmCompatChanged} of {app.run.species} species changed.</span>
    </div>
  {/if}
  {#if app.run.tutorsChanged}
    <div class="changes">
      <span class="strong">Move tutors</span>
      <span>{app.run.tutorsChanged} of {app.run.tutors} changed.</span>
    </div>
  {/if}
  {#if app.run.tutorCompatChanged}
    <div class="changes">
      <span class="strong">Move tutor compatibility</span>
      <span>{app.run.tutorCompatChanged} of {app.run.species} species changed.</span>
    </div>
  {/if}
  {#if app.run.movesetsChanged}
    <div class="changes">
      <span class="strong">Movesets</span>
      <span>{app.run.movesetsChanged} of {app.run.species} species changed.</span>
    </div>
  {/if}
  {#if app.run.wildSetsChanged}
    <div class="changes">
      <span class="strong">Wild Pokémon</span>
      <span>{app.run.wildSetsChanged} of {app.run.wildSets} encounter sets changed.</span>
    </div>
  {/if}
  {#if app.run.trainersChanged}
    <div class="changes">
      <span class="strong">Trainers</span>
      <span>{app.run.trainersChanged} of {app.run.trainers} changed.</span>
    </div>
  {/if}
  {#if app.run.staticsChanged}
    <div class="changes">
      <span class="strong">Static Pokémon</span>
      <span>{app.run.staticsChanged} of {app.run.statics} static encounters changed.</span>
    </div>
  {/if}
  {#if app.run.totemsChanged}
    <div class="changes">
      <span class="strong">Totem Pokémon</span>
      <span>{app.run.totemsChanged} of {app.run.totems} Totem Pokémon changed.</span>
    </div>
  {/if}
  {#if app.run.tradesChanged}
    <div class="changes">
      <span class="strong">In-game trades</span>
      <span>{app.run.tradesChanged} of {app.run.trades} trades changed.</span>
    </div>
  {/if}
  {#if app.run.fieldItemsChanged}
    <div class="changes">
      <span class="strong">Field items</span>
      <span>{app.run.fieldItemsChanged} of {app.run.fieldItems} changed.</span>
    </div>
  {/if}
  {#if app.run.shopsChanged}
    <div class="changes">
      <span class="strong">Shops</span>
      <span>{app.run.shopsChanged} of {app.run.shops} changed.</span>
    </div>
  {/if}
  {#if app.run.pickupItemsChanged}
    <div class="changes">
      <span class="strong">Pickup items</span>
      <span>{app.run.pickupItemsChanged} of {app.run.pickupItems} changed.</span>
    </div>
  {/if}
  <p class="lede">Give these to other people so they can produce exactly the same randomization.</p>
  <CopyField label="Random seed" value={app.run.seed} />
  {#if app.race}
    <CopyField label="Race check value" value={checkValue} />
    <p class="hint">Distribute this with the settings if you’re the race maker. If you received it in a race, compare it to the value the race maker gave.</p>
  {/if}
  {#if app.run.settingsString}<CopyField label="Settings string" value={app.run.settingsString} />{/if}
  {#if app.features.log}
    <label class="log">
      <input type="checkbox" class="chk" bind:checked={app.run.saveLog} disabled={app.race} />
      <span class="col">
        <span class="strong">Save a log of this randomization</span>
        <span class="hint">{app.race ? 'Race Mode is on, so no log can be saved.' : 'The log reveals everything that changed. Leave it off if you’re racing.'}</span>
      </span>
    </label>
  {/if}
  {/if}
  {#snippet footer()}
    {#if app.features.seedFiles && app.run.shared}<button class="btn" onclick={() => app.saveSeedFile()}>Produce Seed File…</button>{/if}
    <button class="btn primary lg" onclick={() => app.finish()}>Done</button>
  {/snippet}
</Dialog>

<style>
  .changes { display: flex; flex-direction: column; gap: 8px; padding: 14px; border-radius: 12px; background: var(--panel2); font-size: 14px; }
  .changes ul { margin: 0; padding: 0; list-style: none; display: flex; flex-direction: column; gap: 4px; }
  .before { color: var(--muted); }
  .log { display: flex; gap: 12px; align-items: flex-start; padding: 14px; border-radius: 12px; background: var(--panel2); font-size: 14px; }
  .log .chk { margin-top: 2px; }
  .col { display: flex; flex-direction: column; gap: 4px; }
  .strong { font-weight: 600; }
  .hint { color: var(--muted); font-size: 13px; }
</style>
