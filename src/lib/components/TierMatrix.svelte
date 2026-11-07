<script lang="ts">
  import type { MatrixRow } from '../options/engine';
  import { app } from '../app.svelte';
  import Tip from './Tip.svelte';

  let { rows }: { rows: MatrixRow[] } = $props();
</script>

<div class="sub">Per trainer tier</div>
<div class="scroll">
  <table class="mx">
    <thead><tr><th scope="col" class="left">Option</th><th scope="col">Boss</th><th scope="col">Important</th><th scope="col">Regular</th></tr></thead>
    <tbody>
      {#each rows as r (r.label)}
        <tr>
          <th scope="row" class="left rowhead"><span class="lab"><span>{r.label}</span><Tip text={r.tip} label={r.label} /></span></th>
          {#each r.cells as c (c.aria)}
            <td>
              <span class="cell">
                <input type="checkbox" class="sw" checked={c.on} disabled={c.disabled} onchange={() => app.set(c.key + '.on', !c.on)} aria-label={c.aria} />
                {#if c.num}
                  <input type="number" class="num" min="1" max="5" value={c.value} disabled={c.disabled || !c.on} aria-label={c.aria + ', amount'}
                    onchange={(e) => app.set(c.key + '.v', Math.max(1, Math.min(5, Number(e.currentTarget.value))))} />
                {/if}
              </span>
            </td>
          {/each}
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .sub {
    margin-top: 20px;
  }
  .scroll {
    overflow-x: auto;
    container: tiers / inline-size;
  }
  .left {
    text-align: left;
  }
  .rowhead {
    font: 500 14px Inter, sans-serif;
    color: var(--text);
    text-transform: none;
    letter-spacing: 0;
    border-top: 1px solid var(--line);
  }
  .cell {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }
  .num {
    width: 60px;
  }
  /* Narrow cards: each cell stacks its switch over its amount, so the table fits without scrolling sideways. */
  @container tiers (max-width: 479px) {
    .cell {
      flex-direction: column;
      gap: 6px;
    }
    .num {
      width: 52px;
    }
    th {
      letter-spacing: 0.02em;
    }
  }
</style>
