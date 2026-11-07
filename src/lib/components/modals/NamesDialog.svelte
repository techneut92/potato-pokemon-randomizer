<script lang="ts">
  import { app, NAME_LISTS } from '../../app.svelte';
  import Dialog from './Dialog.svelte';

  let tab = $state(0);
  let draft = $state([...app.names]);
  const count = $derived(draft[tab].split('\n').filter((x) => x.trim()).length);
  const save = async () => {
    if (await app.saveNames(draft)) app.modal = null;
  };
</script>

<Dialog id="d-names" width={880} title="Custom Names Editor" lede="Names used by “Randomize Trainer Names”, “Randomize Trainer Class Names” and the in-game trades’ nicknames and trainers. One per line; the built-in lists to start with.">
  {#snippet aside()}<span class="count">{count} names</span>{/snippet}
  {#snippet toolbar()}
    <div class="tabs" role="tablist" aria-label="Name lists">
      {#each NAME_LISTS as [label], i (label)}
        <button class="tab" class:cur={i === tab} role="tab" aria-selected={i === tab} onclick={() => (tab = i)}>{label}</button>
      {/each}
    </div>
  {/snippet}
  <textarea class="inp mono" aria-label={NAME_LISTS[tab][0]} bind:value={draft[tab]}></textarea>
  {#snippet footer()}
    <button class="btn ghost" onclick={() => app.closeModal()}>Close</button>
    <button class="btn primary lg" onclick={save}>Save</button>
  {/snippet}
</Dialog>

<style>
  .count { font-size: 13px; color: var(--faint); }
  /* Fills the dialog body: as tall as the window allows, between 160 and 280 px. */
  textarea { flex: 1 1 auto; min-height: clamp(96px, 100dvh - 400px, 280px); padding: 14px; line-height: 1.7; resize: none; }
  /* Narrow dialogs: the lists in one row that scrolls sideways, so the text keeps its room. */
  @container sheet (max-width: 519px) {
    .tabs { flex-wrap: nowrap; overflow-x: auto; scrollbar-width: none; }
    .tab { flex: none; }
  }
</style>
