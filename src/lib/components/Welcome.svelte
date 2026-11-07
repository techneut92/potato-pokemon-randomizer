<script lang="ts">
  import { app } from '../app.svelte';
  import { SUPPORTED_GAMES } from '../data/roms';
  import Icon from './Icon.svelte';

  const EXTENSIONS = ['.gb', '.gbc', '.gba', '.nds', '.3ds', '.cia', '.cxi'];
</script>

<main>
  <div class="top">
    <section class="drop">
      <div class="badge-icon"><Icon d="M12 15V3M7 8l5-5 5 5M4 15v4a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-4" size={34} /></div>
      <h1 class="disp">Drop a ROM here to start</h1>
      <p>Or open one from disk. The randomizer reads the game, then shows only the options that game supports.</p>
      <div class="row-btns">
        <button class="btn primary lg" disabled={app.busy} onclick={() => app.openRom()}>{app.busy ? 'Opening…' : 'Open ROM…'}</button>
        <button class="btn lg" onclick={() => (app.features.seedFiles ? (app.modal = 'preset') : app.notYet('Premade seeds'))}>Premade Seed</button>
      </div>
      <div class="row-btns small">{#each EXTENSIONS as e (e)}<span class="chip mono">{e}</span>{/each}</div>
    </section>
    <section class="card recents">
      <h2 class="disp">Recent ROMs</h2>
      {#each app.recent as r (r.path)}
        <button class="recent" disabled={app.busy} title={r.path} onclick={() => app.openPath(r.path)}>
          <span class="gen">{r.gen}</span>
          <span class="col"><span>{r.name}</span><span class="mono path">{r.path.startsWith('sample:') ? '[path to rom]' : r.path}</span></span>
        </button>
      {:else}
        <p class="empty">ROMs you open appear here.</p>
      {/each}
      <div class="grow"></div>
      <div class="more">
        <button class="btn" onclick={() => (app.modal = 'names')}>Custom Names Editor</button>
        <button class="btn" onclick={() => (app.modal = 'theme')}>Change UI Theme</button>
      </div>
    </section>
  </div>
  <section class="supported">
    <h2 class="disp">Supported games</h2>
    <div class="games">
      {#each SUPPORTED_GAMES as [name, system, games] (name)}
        <div class="card game">
          <div class="game-head"><span class="disp">{name}</span><span class="system">{system}</span></div>
          <div class="chips">{#each games as t (t)}<span class="chip">{t}</span>{/each}</div>
        </div>
      {/each}
    </div>
  </section>
</main>

<style>
  main { flex: 1; width: 100%; max-width: 1160px; margin: 0 auto; padding: 56px 24px 64px; display: flex; flex-direction: column; gap: 40px; }
  .top { display: flex; flex-wrap: wrap; gap: 24px; align-items: stretch; }
  .drop { flex: 999 1 520px; min-width: 0; border: 2px dashed var(--line2); border-radius: 22px; padding: 56px 32px; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 18px; text-align: center; background: var(--panel); }
  .badge-icon { width: 72px; height: 72px; border-radius: 20px; background: var(--panel2); display: flex; align-items: center; justify-content: center; color: var(--accent); }
  h1 { margin: 0; font-size: 34px; line-height: 1.1; }
  p { margin: 0; max-width: 440px; color: var(--muted); font-size: 15px; }
  .row-btns { display: flex; gap: 10px; flex-wrap: wrap; justify-content: center; }
  .row-btns.small { gap: 6px; }
  .recents { flex: 1 1 320px; display: flex; flex-direction: column; gap: 6px; }
  .recents h2 { margin: 0 0 8px; font-size: 18px; }
  .gen { flex: none; width: 36px; height: 36px; border-radius: 9px; background: var(--panel2); display: flex; align-items: center; justify-content: center; font: 600 12px Inter, sans-serif; color: var(--muted); }
  .col { display: flex; flex-direction: column; min-width: 0; }
  .path { font-size: 12px; color: var(--faint); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .empty { color: var(--faint); font-size: 14px; }
  .grow { flex: 1; }
  .more { border-top: 1px solid var(--line); padding-top: 14px; display: flex; gap: 8px; flex-wrap: wrap; }
  .supported { display: flex; flex-direction: column; gap: 16px; }
  .supported h2 { margin: 0; font-size: 20px; }
  .games { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 250px), 1fr)); gap: 12px; }
  .game { padding: 16px; display: flex; flex-direction: column; gap: 10px; }
  .game-head { display: flex; align-items: baseline; justify-content: space-between; font-weight: 600; font-size: 16px; }
  .system { font-size: 12px; color: var(--faint); font-weight: 400; }
  .chips { display: flex; flex-wrap: wrap; gap: 6px; }
  /* Short windows: a lower drop zone, so Recent ROMs show without scrolling. */
  @media (max-height: 720px) {
    main { padding: 24px 24px 40px; gap: 28px; }
    .drop { padding: 28px 24px; gap: 14px; }
    h1 { font-size: 28px; }
    .badge-icon { width: 56px; height: 56px; border-radius: 16px; }
  }
</style>
