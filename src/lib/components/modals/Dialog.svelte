<script lang="ts">
  import type { Snippet } from 'svelte';
  import { app } from '../../app.svelte';

  /** A dialog: a fixed head (title, `aside`, `toolbar`), a body that scrolls on its own, and fixed actions (`footer`). */
  let { id, title, lede, width = 620, children, footer, aside, toolbar }: {
    id: string;
    title: string;
    lede?: string;
    width?: number;
    children?: Snippet;
    footer?: Snippet;
    aside?: Snippet;
    /** Stays in view above the body, such as a search field and its filters. */
    toolbar?: Snippet;
  } = $props();

  let sheet: HTMLDivElement;
  $effect(() => {
    sheet.querySelector<HTMLElement>('[data-autofocus]')?.focus();
  });
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && app.closeModal()} />

<div class="overlay">
  <button class="backdrop" aria-label="Close dialog" tabindex="-1" onclick={() => app.closeModal()}></button>
  <div class="sheet" style:max-width="{width}px" bind:this={sheet}>
    <div class="dlg" role="dialog" aria-modal="true" aria-labelledby={id}>
      <div class="dlg-head">
        <div class="head">
          <div class="titles">
            <h2 {id} class="disp">{title}</h2>
            {#if lede}<p class="lede">{lede}</p>{/if}
          </div>
          {@render aside?.()}
        </div>
        {@render toolbar?.()}
      </div>
      {#if children}<div class="dlg-body">{@render children()}</div>{/if}
      {#if footer}<div class="foot">{@render footer()}</div>{/if}
    </div>
  </div>
</div>

<style>
  .head { display: flex; justify-content: space-between; align-items: flex-start; gap: 16px; flex-wrap: wrap; }
  .titles { display: flex; flex-direction: column; gap: 6px; }
</style>
