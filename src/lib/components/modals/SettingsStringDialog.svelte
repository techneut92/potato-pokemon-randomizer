<script lang="ts">
  import { app } from '../../app.svelte';
  import Dialog from './Dialog.svelte';
  import CopyField from './CopyField.svelte';

  let input = $state('');
  let loading = $state(false);
  let current = $state('');
  let asked = 0;
  $effect(() => {
    // Recomputed when an option changes while the dialog is open; only the latest answer counts.
    void app.shared();
    const n = ++asked;
    app.settingsString().then(
      (s) => n === asked && (current = s),
      () => n === asked && (current = '')
    );
  });
  const load = async () => {
    if (!input.trim() || loading) return;
    loading = true;
    if (await app.loadSettingsString(input)) {
      input = '';
      app.modal = null;
    }
    loading = false;
  };
</script>

<Dialog id="d-string" title="Settings String" lede="Every option in one compact string, compatible with UPR FVX 1.6.2. Copy it to share, or paste one to load those settings.">
  <CopyField label="Current settings" value={current} />
  <label class="stack"><span class="lbl">Load from string</span>
    <textarea class="inp mono" rows="4" placeholder="Paste a settings string" bind:value={input}></textarea>
  </label>
  {#snippet footer()}
    <button class="btn ghost" onclick={() => app.closeModal()}>Close</button>
    <button class="btn primary lg" disabled={!input.trim() || loading} onclick={load}>Load String</button>
  {/snippet}
</Dialog>

<style>
  .stack { display: flex; flex-direction: column; gap: 8px; }
  textarea { padding: 12px; min-height: 110px; resize: vertical; }
</style>
