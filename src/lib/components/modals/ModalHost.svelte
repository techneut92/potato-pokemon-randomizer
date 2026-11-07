<script lang="ts">
  import type { Component } from 'svelte';
  import { app, type ModalName } from '../../app.svelte';
  import LimitDialog from './LimitDialog.svelte';
  import PresetDialog from './PresetDialog.svelte';
  import BatchDialog from './BatchDialog.svelte';
  import SettingsStringDialog from './SettingsStringDialog.svelte';
  import NamesDialog from './NamesDialog.svelte';
  import ThemeDialog from './ThemeDialog.svelte';
  import UpdateDialog from './UpdateDialog.svelte';
  import RunDialog from './RunDialog.svelte';
  import DoneDialog from './DoneDialog.svelte';
  import PickerDialog from './PickerDialog.svelte';
  import ImportEditsDialog from './ImportEditsDialog.svelte';
  import OutputDialog from './OutputDialog.svelte';

  /** Registry of dialogs by name; the app state decides which one is open. */
  const DIALOGS: Record<Exclude<ModalName, null>, Component> = {
    limit: LimitDialog,
    preset: PresetDialog,
    batch: BatchDialog,
    string: SettingsStringDialog,
    names: NamesDialog,
    theme: ThemeDialog,
    update: UpdateDialog,
    run: RunDialog,
    done: DoneDialog,
    pick: PickerDialog,
    importEdits: ImportEditsDialog,
    output: OutputDialog
  };
  const Active = $derived(app.modal ? DIALOGS[app.modal] : null);
</script>

{#if Active}<Active />{/if}
