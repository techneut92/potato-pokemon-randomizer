<script lang="ts">
  import type { EditIssue } from '../../backend';
  import Icon from '../Icon.svelte';

  /** The issues about one object, shown with it: errors as a warning, warnings as a note. */
  let { issues }: { issues: EditIssue[] } = $props();
  const errors = $derived(issues.filter((i) => i.severity === 'error'));
  const warnings = $derived(issues.filter((i) => i.severity !== 'error'));
</script>

{#if errors.length}
  <div class="ewarn" role="status">
    <Icon d="M12 3l9.5 17h-19zM12 10v4M12 17h.01" size={16} />
    <div class="msgs">
      {#each errors as m, i (i)}<span>{m.message}</span>{/each}
    </div>
  </div>
{/if}
{#if warnings.length}
  <div class="note" role="status">
    <Icon d="M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 11v5M12 8h.01" size={16} />
    <div class="msgs">
      {#each warnings as m, i (i)}<span>{m.message}</span>{/each}
    </div>
  </div>
{/if}

<style>
  .msgs { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 4px; }
</style>
