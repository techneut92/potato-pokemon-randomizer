<script lang="ts">
  import { tooltip, placeTooltip } from '../tooltip.svelte';

  let box = $state<HTMLDivElement>();
  let pos = $state({ x: -9999, y: -9999 });

  const place = () => {
    if (!tooltip.anchor || !box) return;
    if (!tooltip.anchor.isConnected) return tooltip.hide();
    pos = placeTooltip(tooltip.anchor.getBoundingClientRect(), box.offsetWidth, box.offsetHeight, window.innerWidth, window.innerHeight);
  };

  $effect(() => {
    void tooltip.text;
    pos = { x: -9999, y: -9999 };
    queueMicrotask(place);
  });

  // Follow the anchor while any container scrolls.
  $effect(() => {
    document.addEventListener('scroll', place, true);
    return () => document.removeEventListener('scroll', place, true);
  });
</script>

<svelte:window onresize={place} onkeydown={(e) => e.key === 'Escape' && tooltip.hide()} />

{#if tooltip.text}
  <div bind:this={box} class="tooltip" role="tooltip" style:left="{pos.x}px" style:top="{pos.y}px">{tooltip.text}</div>
{/if}
