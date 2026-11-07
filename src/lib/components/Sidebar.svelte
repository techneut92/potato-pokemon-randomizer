<script lang="ts">
  import { app } from '../app.svelte';
  import Icon from './Icon.svelte';
  import RomCard from './RomCard.svelte';
  import EditNav from './edits/EditNav.svelte';
</script>

<aside class:open={app.navOpen}>
  <RomCard />

  {#if app.mode === 'edits'}
    <EditNav />
  {:else}
    <label class="search">
      <Icon d="M11 4a7 7 0 1 0 0 14 7 7 0 1 0 0-14zM20 20l-3.5-3.5" size={16} />
      <input type="search" placeholder="Search all options" aria-label="Search all options" bind:value={app.query} onkeydown={(e) => e.key === 'Enter' && (app.navOpen = false)} />
    </label>

    <nav aria-label="Option categories">
      {#each app.view.sections as s (s.id)}
        {@const cur = s.id === app.sec && !app.query}
        <button class="nav" class:cur class:dim={s.unsupported} aria-current={cur ? 'page' : undefined} onclick={() => app.goTo(s.id)}>
          <Icon d={s.icon} />
          {s.label}
          <span class="badge" class:hot={s.count > 0}>{s.count}</span>
        </button>
      {/each}
    </nav>
  {/if}
</aside>

<style>
  aside { flex: none; width: clamp(232px, 24vw, 300px); overflow-y: auto; padding: 20px 16px; display: flex; flex-direction: column; gap: 18px; border-right: 1px solid var(--line); scrollbar-width: thin; }
  nav { display: flex; flex-direction: column; gap: 2px; }
  /* Narrow windows: the sidebar becomes a drawer, opened from the header's menu button. */
  @media (max-width: 719px) {
    aside { position: absolute; inset: 0 auto 0 0; z-index: 35; width: min(300px, 85vw); background: var(--bg); box-shadow: 0 18px 50px rgba(0, 0, 0, 0.35); transform: translateX(-100%); visibility: hidden; transition: transform 0.2s, visibility 0.2s; }
    aside.open { transform: none; visibility: visible; }
  }
  @media (max-width: 719px) and (prefers-reduced-motion: reduce) {
    aside { transition: none; }
  }
  @media (max-height: 760px) {
    aside { gap: 12px; padding-block: 14px; }
    aside :global(.nav) { min-height: 38px; }
  }
</style>
