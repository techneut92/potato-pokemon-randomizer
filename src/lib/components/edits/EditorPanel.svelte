<script lang="ts">
  import { app } from '../../app.svelte';
  import { sectionSize } from '../../edits/changes';
  import { issueLinks } from '../../edits/issues';
  import { EDIT_SECTIONS } from '../../edits/sections';
  import Icon from '../Icon.svelte';
  import FieldSection from './FieldSection.svelte';
  import ItemsSection from './ItemsSection.svelte';
  import MonsSection from './MonsSection.svelte';
  import ShopsSection from './ShopsSection.svelte';
  import StartersSection from './StartersSection.svelte';
  import StaticsSection from './StaticsSection.svelte';
  import TradesSection from './TradesSection.svelte';
  import TrainersSection from './TrainersSection.svelte';
  import WildSection from './WildSection.svelte';

  const sec = $derived(EDIT_SECTIONS.find((s) => s.id === app.editSec) ?? EDIT_SECTIONS[0]);
  const errors = $derived(issueLinks(app.editIssues, 'error'));
  const warnings = $derived(issueLinks(app.editIssues, 'warning'));

  // The panel scrolls on its own, so another section starts at the top.
  let pane: HTMLElement;
  $effect(() => {
    void app.editSec;
    pane.scrollTop = 0;
  });
</script>

<main class="editor" bind:this={pane}>
  {#if app.editStatus === 'failed'}
    <section class="card state" role="alert">
      <h1 class="disp">Couldn’t read this game’s data</h1>
      <p class="lede">The manual editor needs the game’s data and it didn’t load. Your randomizer options are not affected.</p>
      <div><button class="btn primary" onclick={() => app.loadEditView()}>Try again</button></div>
    </section>
  {:else if app.editStatus !== 'ready' || !app.editView}
    <section class="card state" role="status">
      <h1 class="disp">Reading {app.rom.name}…</h1>
      <p class="lede">Loading every trainer, wild area, shop and item of this game. This takes a moment.</p>
      <div class="meter" aria-hidden="true"><div class="meterload"></div></div>
    </section>
  {:else}
    <div class="head">
      <div class="intro">
        <h1 class="disp">{sec.label}</h1>
        <p class="lede">{sec.desc}</p>
      </div>
      <div class="actions">
        <button class="btn ghost" disabled={!app.edits.countOf(sec.kinds)} onclick={() => app.revertEditSection()}>Revert section</button>
        <button class="btn" aria-label="Changes: {app.edits.count}" aria-expanded={app.changesOpen} onclick={() => (app.changesOpen = !app.changesOpen)}>
          <Icon d="M12 20h9M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4z" />
          <span class="chlabel">Changes</span> <span class="badge" class:hot={app.edits.count > 0}>{app.edits.count}</span>
        </button>
      </div>
    </div>

    {#if errors.length}
      <div class="ewarn" role="status">
        <Icon d="M12 3l9.5 17h-19zM12 10v4M12 17h.01" size={16} />
        <div class="issues">
          <strong>Fix these before saving</strong>
          {#each errors as x (x.key)}
            <span class="issue"><span>{x.text}</span>{#if x.section}{@const to = x.section}<button class="linkbtn" onclick={() => app.goToEdit(to, x.target)}>{x.goLabel}</button>{/if}</span>
          {/each}
        </div>
      </div>
    {/if}
    {#if warnings.length}
      <div class="note" role="status">
        <Icon d="M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 11v5M12 8h.01" size={16} />
        <div class="issues">
          <strong>Worth a second look</strong>
          {#each warnings as x (x.key)}
            <span class="issue"><span>{x.text}</span>{#if x.section}{@const to = x.section}<button class="linkbtn" onclick={() => app.goToEdit(to, x.target)}>{x.goLabel}</button>{/if}</span>
          {/each}
        </div>
      </div>
    {/if}

    {#if sectionSize(app.editView, sec.id) === 0}
      <section class="card state">
        <h2 class="disp">Nothing to edit here</h2>
        <p class="lede">{app.rom.name} has no {sec.label.toLowerCase()} the editor can change.</p>
      </section>
    {:else if sec.id === 'starters'}
      <StartersSection view={app.editView} />
    {:else if sec.id === 'mons'}
      <MonsSection view={app.editView} />
    {:else if sec.id === 'trainers'}
      <TrainersSection view={app.editView} />
    {:else if sec.id === 'wild'}
      <WildSection view={app.editView} />
    {:else if sec.id === 'statics'}
      <StaticsSection view={app.editView} />
    {:else if sec.id === 'trades'}
      <TradesSection view={app.editView} />
    {:else if sec.id === 'shops'}
      <ShopsSection view={app.editView} />
    {:else if sec.id === 'items'}
      <ItemsSection view={app.editView} />
    {:else}
      <FieldSection view={app.editView} />
    {/if}
  {/if}
</main>

<style>
  /* The view's one scroll area, and the size container the list sections measure (see app.css). */
  main { flex: 1; min-width: 0; overflow-y: auto; padding: var(--padtop) var(--padx) 40px; display: flex; flex-direction: column; gap: 20px; container: pane / size; scrollbar-width: thin; }
  h1 { margin: 0; font-size: var(--h1); }
  .state { display: flex; flex-direction: column; gap: 12px; max-width: 640px; }
  .state h1 { font-size: 24px; }
  .state h2 { margin: 0; font-size: 20px; }
  /* The head stays one row while it fits: the intro shrinks before the actions wrap. */
  .head { display: flex; align-items: flex-end; justify-content: space-between; gap: 12px 16px; flex-wrap: wrap; }
  .intro { flex: 1 1 240px; display: flex; flex-direction: column; gap: 6px; max-width: 760px; min-width: 0; }
  .intro .lede { font-size: 15px; }
  .actions { display: flex; gap: 8px; flex-wrap: wrap; }
  .actions .badge { margin-left: 0; }
  .issues { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 4px; }
  .issue { display: flex; gap: 8px; align-items: baseline; flex-wrap: wrap; }
  @media (max-height: 720px) {
    .intro .lede { font-size: 14px; }
  }
  /* Narrow panes: the Changes button keeps its icon and count. */
  @container pane (max-width: 679px) {
    .chlabel { display: none; }
  }
</style>
