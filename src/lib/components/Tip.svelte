<script lang="ts">
  import { tooltip } from '../tooltip.svelte';

  let { text, label }: { text: string; label: string } = $props();
  let el = $state<HTMLSpanElement>();

  const show = () => el && tooltip.show(el, text);
  const showOnKeyboardFocus = () => {
    if (el?.matches(':focus-visible')) show();
  };
  // A click must neither pin the tooltip nor toggle the switch of the surrounding <label>.
  const swallow = (e: MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    tooltip.hide();
  };
</script>

{#if text}
  <span
    bind:this={el}
    class="tipbtn"
    role="button"
    tabindex="0"
    aria-label={'About ' + label}
    onmouseenter={show}
    onmouseleave={() => tooltip.hide()}
    onfocus={showOnKeyboardFocus}
    onblur={() => tooltip.hide()}
    onclick={swallow}
    onkeydown={(e) => e.key === 'Escape' && tooltip.hide()}>?</span
  >
{/if}
