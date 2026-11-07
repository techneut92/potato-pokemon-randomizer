<script lang="ts">
  import type { OptView } from '../options/engine';
  import { app } from '../app.svelte';
  import Tip from './Tip.svelte';
  import Pills from './Pills.svelte';
  import Icon from './Icon.svelte';

  let { o }: { o: OptView } = $props();

  const flip = () => app.set(o.key + '.on', !o.on);
  const setValue = (v: unknown) => app.set(o.key + '.v', v);
  const num = (e: Event) => setValue(Number((e.currentTarget as HTMLInputElement | HTMLSelectElement).value));
  const valueOff = $derived(o.disabled || (o.hasToggle && !o.on));
</script>

{#snippet head()}
  <label class="lab">
    {#if o.hasToggle}<input type="checkbox" class="sw" checked={o.on} disabled={o.disabled} onchange={flip} />{/if}
    <span>{o.label}</span>
  </label>
  <Tip text={o.tip} label={o.label} />
{/snippet}

{#if o.kind === 'sub'}
  <div class="sub">{o.label}</div>
{:else if o.kind === 'info'}
  <p class="info">{o.text}</p>
{:else if o.kind === 'update'}
  <div class="actions">
    <button class="btn" onclick={() => (app.features.gameUpdates ? (app.modal = 'update') : app.notYet('Game updates'))}>Load Game Update…</button>
    <button class="btn ghost" disabled={!app.update} onclick={() => app.unloadUpdate()}>Unload Game Update</button>
  </div>
{:else if o.kind === 'toggle'}
  <label class="row" class:dim={o.disabled}>
    <span class="lab"><span>{o.label}</span><Tip text={o.tip} label={o.label} /></span>
    <input type="checkbox" class="sw" checked={o.on} disabled={o.disabled} onchange={flip} />
  </label>
{:else if o.kind === 'limit'}
  <div class="row">
    <label class="lab"><input type="checkbox" class="sw" checked={o.on} onchange={flip} /><span>{o.label}</span></label>
    <Tip text={o.tip} label={o.label} />
    <span class="grow"></span>
    <button class="btn compact" disabled={!o.on} onclick={() => (app.modal = 'limit')}>{app.limitText}</button>
  </div>
{:else if o.kind === 'slider'}
  <div class="row slider" class:dim={o.disabled}>
    <span class="lab">{@render head()}</span>
    <output class="mono">{o.valueText}</output>
    <input type="range" class="range" min={o.min} max={o.max} value={o.value} disabled={valueOff} oninput={num} aria-label={o.label} />
  </div>
{:else if o.kind === 'select'}
  <div class="row" class:dim={o.disabled}>
    <span class="lab">{@render head()}</span>
    <select class="sel" value={String(o.value)} disabled={valueOff} onchange={num} aria-label={o.label}>
      {#each o.choices as c (c.i)}<option value={String(c.i)}>{c.label}</option>{/each}
    </select>
  </div>
{:else if o.kind === 'number'}
  <div class="row" class:dim={o.disabled}>
    <span class="lab">{@render head()}</span>
    <input type="number" class="num" min={o.min} max={o.max} value={o.value} disabled={valueOff} onchange={num} aria-label={o.label} />
  </div>
{:else if o.kind === 'radio'}
  <div class="radio" class:dim={o.disabled}>
    <div class="radio-label">{o.label}</div>
    <Pills choices={o.choices} label={o.label} onpick={setValue} />
    {#if o.choiceTip}<p class="mdesc">{o.choiceTip}</p>{/if}
  </div>
{:else if o.kind === 'species'}
  <div class="row static" class:dim={o.disabled}>
    <span class="lab">{o.label}</span>
    <button class="pickbtn" disabled={o.disabled} aria-label={o.label + ': ' + o.value + '. Choose another Pokémon'} onclick={() => app.openSpecies(o.key, o.label)}>
      <span class="mono dexnum">{o.dexNum}</span>
      <span class="grow">{o.value}</span>
      <Icon d="M11 4a7 7 0 1 0 0 14 7 7 0 1 0 0-14zM21 21l-4.3-4.3" size={16} />
    </button>
  </div>
{/if}

<style>
  .info {
    margin: 6px 0 10px;
    font-size: 13px;
    color: var(--muted);
    break-inside: avoid;
  }
  .actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    padding-top: 4px;
  }
  .grow {
    flex: 1;
    text-align: left;
  }
  .compact {
    min-height: 34px;
  }
  .slider {
    flex-wrap: wrap;
    padding: 8px 0;
    gap: 8px 16px;
  }
  .slider .lab {
    flex: 1 1 auto;
  }
  output {
    font-size: 13px;
    color: var(--muted);
  }
  .radio {
    padding: 10px 0 12px;
    border-top: 1px solid var(--line);
    break-inside: avoid;
  }
  .radio-label {
    font-size: 13px;
    color: var(--muted);
    margin-bottom: 8px;
  }
  .radio .mdesc {
    margin: 10px 0 0;
  }
  .static {
    cursor: default;
  }
  .dexnum {
    color: var(--faint);
    font-size: 12px;
  }
</style>
