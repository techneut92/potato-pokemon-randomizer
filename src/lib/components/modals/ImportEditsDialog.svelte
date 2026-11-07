<script lang="ts">
  import { app } from '../../app.svelte';
  import { replaceQuestion } from '../../edits/files';
  import Dialog from './Dialog.svelte';

  const pending = $derived(app.pendingImport);
  const lede = $derived(pending ? replaceQuestion(app.rom.name, app.edits.count, pending.file, pending.list.edits.length) : undefined);
</script>

<Dialog id="d-import-edits" title="Replace your edits?" {lede} width={520}>
  {#snippet footer()}
    <button class="btn ghost" data-autofocus onclick={() => app.closeModal()}>Keep my edits</button>
    <button class="btn primary lg" onclick={() => app.confirmImport()}>Replace edits</button>
  {/snippet}
</Dialog>
