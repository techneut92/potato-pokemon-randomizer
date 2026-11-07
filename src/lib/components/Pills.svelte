<script lang="ts">
  import type { ChoiceView } from '../options/engine';

  let { choices, label, onpick }: { choices: ChoiceView[]; label: string; onpick: (i: number) => void } = $props();
</script>

<div class="pills" role="radiogroup" aria-label={label}>
  {#each choices as c (c.i)}
    <button class="pill" class:cur={c.cur} role="radio" aria-checked={c.cur} disabled={c.disabled} onclick={() => onpick(c.i)}>{c.label}</button>
  {/each}
</div>
