<script lang="ts">
  import { app } from './lib/app.svelte';
  import Header from './lib/components/Header.svelte';
  import Welcome from './lib/components/Welcome.svelte';
  import Sidebar from './lib/components/Sidebar.svelte';
  import SectionPanel from './lib/components/SectionPanel.svelte';
  import EditorPanel from './lib/components/edits/EditorPanel.svelte';
  import ChangesPanel from './lib/components/edits/ChangesPanel.svelte';
  import Footer from './lib/components/Footer.svelte';
  import Toast from './lib/components/Toast.svelte';
  import TooltipHost from './lib/components/TooltipHost.svelte';
  import ModalHost from './lib/components/modals/ModalHost.svelte';
</script>

<div class="app" data-theme={app.theme} style:--accent={app.accent}>
  <Header />
  {#if app.screen === 'welcome'}
    <div class="page"><Welcome /></div>
  {:else}
    <div class="workspace">
      {#if app.navOpen}<button class="scrim" aria-label="Close categories" onclick={() => (app.navOpen = false)}></button>{/if}
      <Sidebar />
      {#if app.mode === 'edits'}
        <EditorPanel />
        {#if app.changesOpen && app.editStatus === 'ready'}
          <button class="scrim changes-scrim" aria-label="Close changes" tabindex="-1" onclick={() => (app.changesOpen = false)}></button>
          <ChangesPanel />
        {/if}
      {:else}
        <SectionPanel />
      {/if}
    </div>
    <Footer />
  {/if}
  <ModalHost />
  <Toast />
  <TooltipHost />
</div>

<style>
  /* Header and footer stay put; the panes between them scroll on their own. */
  .page {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
  .workspace {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .scrim {
    display: none;
    position: absolute;
    inset: 0;
    z-index: 34;
    border: 0;
    background: rgba(0, 0, 0, 0.45);
  }
  /* The category drawer below 720 px, the Changes drawer below 1440 px (see app.css). */
  @media (max-width: 719px) {
    .scrim {
      display: block;
    }
  }
  @media (max-width: 1439px) {
    .changes-scrim {
      display: block;
      z-index: 32;
    }
  }
</style>
