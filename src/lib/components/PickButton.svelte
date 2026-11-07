<script lang="ts">
  import Icon from './Icon.svelte';

  /** A field whose value is chosen in the picker dialog: the value's tag (dex number, …), its name, and an icon. */
  let { num = '', name, label, size, disabled = false, onclick }: {
    num?: string;
    name: string;
    /** What the field is, for screen readers: "Starter 1". */
    label: string;
    size?: 'lg' | 'sm';
    disabled?: boolean;
    onclick: () => void;
  } = $props();
</script>

<button class="pickbtn" class:lg={size === 'lg'} class:sm={size === 'sm'} {disabled} aria-label={label + ': ' + name + '. Choose another'} {onclick}>
  {#if num}<span class="mono pn">{num}</span>{/if}
  <span class="pv">{name}</span>
  <Icon d="M11 4a7 7 0 1 0 0 14 7 7 0 1 0 0-14zM21 21l-4.3-4.3" size={16} />
</button>

<style>
  .pickbtn { width: 100%; max-width: 280px; min-width: 0; min-height: 40px; border-radius: 10px; }
  .pickbtn :global(svg) { flex: none; color: var(--faint); }
  .pv { flex: 1; min-width: 0; text-align: left; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pn { flex: none; color: var(--faint); font-size: 12px; }
  .lg { max-width: none; min-height: 48px; font-size: 16px; }
  .sm { max-width: none; min-height: 36px; font-size: 13px; }
</style>
