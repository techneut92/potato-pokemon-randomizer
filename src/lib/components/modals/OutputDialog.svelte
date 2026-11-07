<script lang="ts">
  import { app } from '../../app.svelte';
  import Dialog from './Dialog.svelte';

  const ext = $derived(app.rom.ext || '.3ds');
</script>

<Dialog id="d-output" title="3DS Output Choice" lede="Would you like to save your 3DS game as a ROM file or as a LayeredFS folder?" width={560}>
  <ul class="choices">
    <li><span class="strong">ROM file ({ext})</span> <span class="before">· the whole game, to play in an emulator or install.</span></li>
    <li><span class="strong">LayeredFS folder</span> <span class="before">· only the changed files, in a folder named after the game’s title ID. Copy it to your console’s or emulator’s mod folder; the game itself stays as it is.</span></li>
  </ul>
  {#snippet footer()}
    <button class="btn ghost" onclick={() => app.closeModal()}>Cancel</button>
    <button class="btn" data-autofocus onclick={() => app.chooseOutput(false)}>ROM file</button>
    <button class="btn primary lg" onclick={() => app.chooseOutput(true)}>LayeredFS folder</button>
  {/snippet}
</Dialog>

<style>
  .choices { display: flex; flex-direction: column; gap: 8px; margin: 0; padding-left: 18px; }
</style>
