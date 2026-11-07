<script lang="ts" module>
  /** One object in a section's list. `tags` are the filters it falls under. */
  export interface ListRow {
    id: number;
    label: string;
    sub: string;
    edited: boolean;
    tags?: readonly string[];
  }
</script>

<script lang="ts">
  import Icon from '../Icon.svelte';

  /**
   * The list of a list section: search, optional filter pills, and the objects, the shown one marked.
   * On a wide editor pane it stands beside the detail, sticky and as tall as the pane. On a narrow one (see app.css)
   * a picker bar takes its place: the shown object, previous / next through the filtered list, and the list itself
   * as a popover. Both are this one list, laid out by a container query.
   */
  let { label, hint, plural, rows, selected, onpick, filters = [], filterLabel = 'Filter', searchSub = true }: {
    label: string;
    hint: string;
    /** What the count counts: "trainers". */
    plural: string;
    rows: ListRow[];
    selected: number;
    onpick: (id: number) => void;
    /** Filter pills, the first ("all") showing every row. */
    filters?: readonly (readonly [string, string])[];
    filterLabel?: string;
    /** Whether the search also looks at each row's second line. */
    searchSub?: boolean;
  } = $props();

  /** A long list shows this many rows; the search narrows it. */
  const SHOWN = 250;

  let query = $state('');
  let filter = $state('all');

  const pool = $derived(filter === 'all' ? rows : rows.filter((r) => r.tags?.includes(filter)));
  const found = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return q ? pool.filter((r) => r.label.toLowerCase().includes(q) || (searchSub && r.sub.toLowerCase().includes(q))) : pool;
  });
  const countText = $derived((query.trim() ? found.length + ' of ' + pool.length : pool.length) + ' ' + plural + (found.length > SHOWN ? ' · showing the first ' + SHOWN + ', search to narrow' : ''));

  // The picker bar: where the shown object is in the filtered list, and stepping through it.
  const at = $derived(found.findIndex((r) => r.id === selected));
  const current = $derived(at >= 0 ? found[at] : rows.find((r) => r.id === selected));
  const step = (d: number) => {
    if (!found.length) return;
    onpick(found[at < 0 ? 0 : (at + d + found.length) % found.length].id);
  };

  let open = $state(false);
  let host: HTMLElement;
  let toggle: HTMLButtonElement;
  const close = (refocus = false) => {
    open = false;
    if (refocus) toggle.focus();
  };
  const pick = (id: number) => {
    onpick(id);
    if (open) close(true);
  };
  // Opening scrolls the bar to the pane's top, where it sticks, so the popover gets the pane's height.
  $effect(() => {
    if (!open) return;
    const pane = host.closest('main');
    const below = pane ? host.getBoundingClientRect().top - pane.getBoundingClientRect().top : 0;
    if (pane && below > 0) pane.scrollTop += below;
    host.querySelector<HTMLInputElement>('input[type=search]')?.focus({ preventScroll: true });
  });

  // The shown object stays in view, also after a jump from the Changes list. Only the list scrolls, not the page.
  let list: HTMLElement;
  $effect(() => {
    void open;
    const el = list.querySelector<HTMLElement>(`[data-id="${selected}"]`);
    if (!el) return;
    const top = el.offsetTop - list.offsetTop;
    if (top < list.scrollTop) list.scrollTop = top;
    else if (top + el.offsetHeight > list.scrollTop + list.clientHeight) list.scrollTop = top + el.offsetHeight - list.clientHeight;
  });
</script>

<svelte:window
  onkeydown={(e) => open && e.key === 'Escape' && close(true)}
  onpointerdown={(e) => open && !host.contains(e.target as Node) && close()}
/>

<div class="host" class:open bind:this={host}>
  <div class="bar">
    <button class="lcur" bind:this={toggle} aria-haspopup="true" aria-expanded={open} aria-label="{label}: {current?.label ?? 'none'}. Choose another" onclick={() => (open = !open)}>
      <span class="dot" class:off={!current?.edited} aria-hidden="true"></span>
      <span class="min0"><span class="ell">{current?.label ?? 'Nothing selected'}</span><small class="ell">{current?.sub ?? ''}</small></span>
      <span class="pos mono">{at >= 0 ? at + 1 + ' / ' + found.length : found.length}</span>
      <Icon d="M6 9l6 6 6-6" size={16} />
    </button>
    <button class="btn ico" aria-label="Previous" disabled={!found.length} onclick={() => step(-1)}><Icon d="M15 6l-6 6 6 6" size={16} /></button>
    <button class="btn ico" aria-label="Next" disabled={!found.length} onclick={() => step(1)}><Icon d="M9 6l6 6-6 6" size={16} /></button>
  </div>
<section class="card pane" aria-label="{label} list">
  <label class="search">
    <Icon d="M11 4a7 7 0 1 0 0 14 7 7 0 1 0 0-14zM20 20l-3.5-3.5" size={16} />
    <input type="search" placeholder={hint} aria-label={hint} bind:value={query} />
  </label>
  {#if filters.length > 1}
    <!-- Pills while the list is wide; a narrow list (the column beside the detail) gets a dropdown, so the rows keep their room. -->
    <div class="pills" role="radiogroup" aria-label={filterLabel}>
      {#each filters as [id, text] (id)}
        <button class="pill" class:cur={filter === id} role="radio" aria-checked={filter === id} onclick={() => (filter = id)}>{text}</button>
      {/each}
    </div>
    <select class="sel fsel" aria-label={filterLabel} bind:value={filter}>
      {#each filters as [id, text] (id)}<option value={id}>{text}</option>{/each}
    </select>
  {/if}
  <span class="count">{countText}</span>
  <div class="scroll" role="list" bind:this={list}>
    {#each found.slice(0, SHOWN) as r (r.id)}
      <div role="listitem" class="item" data-id={r.id}>
        <button class="li" class:cur={r.id === selected} aria-current={r.id === selected} onclick={() => pick(r.id)}>
          <span class="dot" class:off={!r.edited} aria-hidden="true"></span>
          <span class="min0"><span class="ell">{r.label}</span><small>{r.sub}</small></span>
          {#if r.edited}<span class="sr">Edited</span>{/if}
        </button>
      </div>
    {/each}
  </div>
</section>
</div>

<style>
  /* Beside the detail: sticky 12 px below the pane's top, and as tall as the visible pane (cqh: the pane's content box). */
  .host { flex: 0 0 clamp(260px, 34%, 340px); min-width: 0; position: sticky; top: calc(12px - var(--padtop)); z-index: 5; }
  .pane { min-width: 0; padding: 14px; display: flex; flex-direction: column; gap: 10px; max-height: calc(100cqh + var(--padtop) - 12px); container: list / inline-size; }
  .fsel { display: none; width: 100%; min-height: 40px; }
  @container list (max-width: 419px) {
    .pills { display: none; }
    .fsel { display: block; }
  }
  .count { font-size: 12px; color: var(--faint); }
  .scroll { flex: 1 1 auto; min-height: 0; }
  .bar { display: none; }
  /* Above the detail: a sticky picker bar; the list opens under it as a popover. */
  @container pane (max-width: 679px) {
    .host { flex: none; top: calc(-1 * var(--padtop)); }
    .bar { display: flex; gap: 6px; align-items: stretch; padding: 8px 0; background: var(--bg); }
    .lcur { flex: 1; min-width: 0; display: flex; align-items: center; gap: 10px; min-height: 48px; padding: 6px 12px; border-radius: 12px; border: 1px solid var(--line2); background: var(--panel); color: var(--text); font: 500 14px Inter, sans-serif; text-align: left; cursor: pointer; }
    .lcur:hover, .open .lcur { border-color: var(--accent); }
    .lcur:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
    .lcur small { color: var(--faint); font-size: 12px; font-weight: 400; }
    .lcur :global(svg) { flex: none; color: var(--faint); }
    .pos { flex: none; font-size: 12px; color: var(--faint); }
    .bar .btn { min-height: 48px; min-width: 44px; }
    .pane { display: none; position: absolute; left: 0; right: 0; top: calc(100% - 2px); max-height: min(480px, calc(100cqh + var(--padtop) - 30px)); border-color: var(--line2); box-shadow: 0 18px 50px rgba(0, 0, 0, 0.35); }
    .open .pane { display: flex; }
  }
  .item { flex: none; display: flex; }
  .min0 { min-width: 0; flex: 1; }
  .sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
</style>
