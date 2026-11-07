<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import Icon from './Icon.svelte';

  const win = getCurrentWindow();
  let maximized = $state(false);

  $effect(() => {
    let stop: (() => void) | undefined;
    let live = true;
    const sync = async () => {
      maximized = await win.isMaximized();
    };
    void sync();
    void win.onResized(sync).then((unlisten) => (live ? (stop = unlisten) : unlisten()));
    return () => {
      live = false;
      stop?.();
    };
  });
</script>

<div class="controls">
  <button class="wc" aria-label="Minimize" onclick={() => win.minimize()}>
    <Icon d="M5 12h14" size={16} />
  </button>
  <button class="wc" aria-label={maximized ? 'Restore' : 'Maximize'} onclick={() => win.toggleMaximize()}>
    {#if maximized}
      <Icon d="M8 8V5h11v11h-3M5 8h11v11H5z" size={15} />
    {:else}
      <Icon d="M5 5h14v14H5z" size={15} />
    {/if}
  </button>
  <button class="wc close" aria-label="Close" onclick={() => win.close()}>
    <Icon d="M6 6l12 12M18 6L6 18" size={16} />
  </button>
</div>

<style>
  .controls { display: flex; align-self: stretch; }
  .wc { width: 48px; display: grid; place-items: center; border: 0; background: transparent; color: var(--muted); cursor: pointer; }
  .wc:hover { background: var(--panel2); color: var(--text); }
  .wc:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .close:hover { background: #E5484D; color: #fff; }
</style>
