<script lang="ts">
  import { app } from '../../app.svelte';
  import Dialog from './Dialog.svelte';
  import Field from '../Field.svelte';

  let picked = $state<string | null>(null);
  const browse = async () => {
    picked = (await app.pickUpdate()) ?? picked;
  };
  const load = async () => {
    if (picked && (await app.loadUpdate(picked))) app.modal = null;
  };
</script>

<Dialog id="d-update" title="Load Game Update" lede="For 3DS games: include an official update so the randomizer works on the patched data.">
  <Field label="Update file" value={picked ?? 'No file chosen'}><button class="btn" onclick={browse}>Browse…</button></Field>
  <div class="warn"><span>With an update loaded, the game can only be saved as a LayeredFS folder. Use the same update on your console or emulator. If the update file is moved or renamed, load it again.</span></div>
  {#snippet footer()}
    <button class="btn ghost" onclick={() => app.closeModal()}>Cancel</button>
    <button class="btn primary lg" disabled={!picked || app.busy} onclick={load}>Load Update</button>
  {/snippet}
</Dialog>
