<script lang="ts">
  import { app } from '../app.svelte';
  import Icon from './Icon.svelte';
</script>

<div role="status" aria-live="polite">
  {#if app.toast}
    {@const err = app.toast.kind === 'err'}
    <div class="toast" style:color={err ? 'var(--warn)' : 'var(--ok)'}>
      <Icon d={err ? 'M12 3l10 18H2zM12 10v4M12 18h.01' : 'M5 12l5 5L20 7'} />
      <span class="text">{app.toast.text}</span>
    </div>
  {/if}
</div>

<style>
  .text { color: var(--text); }
</style>
