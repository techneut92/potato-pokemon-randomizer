<script lang="ts">
  import { app } from '../app.svelte';
  import Icon from './Icon.svelte';
</script>

<!-- The loaded game, the same in both views. -->
<div class="card rom">
  <div class="rom-head">
    <div class="cart"><Icon d="M7 3h10a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2zM9 7h6M9 17h2" size={26} /></div>
    <div class="min0"><div class="rom-name">{app.rom.name}</div><div class="meta">{app.rom.meta}</div></div>
  </div>
  <dl>
    <dt>Code</dt><dd class="mono">{app.rom.code}</dd>
    <dt>File</dt><dd class="mono ellipsis" title={app.rom.path}>{app.rom.fileName}</dd>
    <dt>Update</dt><dd class="ellipsis" title={app.update?.path}>{app.update ? app.update.version || app.update.path.split(/[\\/]/).pop() : 'None loaded'}</dd>
  </dl>
  <button class="btn full" onclick={() => app.closeRom()}>Open another ROM</button>
</div>

<style>
  .rom { padding: 16px; display: flex; flex-direction: column; gap: 12px; }
  .rom-head { display: flex; gap: 12px; align-items: center; }
  .cart { flex: none; width: 52px; height: 52px; border-radius: 12px; background: var(--panel2); border: 1px solid var(--line2); display: flex; align-items: center; justify-content: center; color: var(--accent); }
  .min0 { min-width: 0; }
  .rom-name { font-weight: 600; font-size: 15px; }
  .meta { font-size: 12px; color: var(--faint); }
  dl { margin: 0; display: grid; grid-template-columns: auto 1fr; gap: 4px 12px; font-size: 12px; }
  dt { color: var(--faint); }
  dd { margin: 0; }
  .ellipsis { overflow: hidden; text-overflow: ellipsis; }
  .full { width: 100%; }
  /* Low windows: keep the categories in view; the ROM's details are secondary. */
  @media (max-height: 760px) {
    .rom { padding: 12px; gap: 10px; }
    dl { display: none; }
  }
</style>
