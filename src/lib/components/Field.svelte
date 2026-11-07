<script lang="ts">
  import type { Snippet } from 'svelte';

  /** Labelled read-only value with an action, e.g. a path with "Browse…". */
  let { label, value, children }: { label: string; value: string; children?: Snippet } = $props();
</script>

<div class="field">
  <span class="lbl">{label}</span>
  <span class="field-row"><input class="inp mono" readonly {value} aria-label={label} />{@render children?.()}</span>
</div>

<style>
  .field {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .field-row {
    display: flex;
    gap: 8px;
  }
</style>
