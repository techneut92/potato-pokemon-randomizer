<script lang="ts">
  import { app, settingsRomName } from '../../app.svelte';
  import type { RomInfo } from '../../data/roms';
  import type { PremadeSeed } from '../../generated/PremadeSeed';
  import Dialog from './Dialog.svelte';
  import Field from '../Field.svelte';

  let source = $state<'seed' | 'file'>('seed');
  let seed = $state('');
  let str = $state('');
  /** What the seed and string fields hold, once both are filled: a premade seed or why it isn't one. */
  let typed = $state<PremadeSeed | string | null>(null);
  let file = $state<{ path: string; premade: PremadeSeed } | null>(null);
  let rom = $state<RomInfo | null>(null);
  let romError = $state('');

  let asked = 0;
  $effect(() => {
    const [s, t] = [seed, str];
    const n = ++asked;
    if (!s.trim() || !t.trim()) {
      typed = null;
      return;
    }
    void app.premadeFromString(s, t).then((r) => n === asked && (typed = r));
  });

  const premade = $derived(source === 'file' ? (file?.premade ?? null) : typeof typed === 'object' ? typed : null);
  const required = $derived(premade?.shared.extras.romName ?? '');
  const matches = $derived(!!rom && !!premade && settingsRomName(rom) === required);
  const hint = $derived.by(() => {
    if (source === 'seed' && typeof typed === 'string') return typed;
    if (!premade) return 'ROM required: enter the settings above first.';
    if (romError) return romError;
    if (!rom) return 'ROM required: ' + required + '.';
    if (!matches) return `This isn’t the required ROM. Required: ${required}. This ROM: ${settingsRomName(rom)}.`;
    return 'Ready. The ROM matches the one these settings were made for.';
  });
  const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

  const chooseFile = async () => {
    const r = await app.premadeFromFile();
    if (typeof r === 'string') app.say(r, 'err');
    else if (r) file = r;
  };
  const chooseRom = async () => {
    const r = await app.premadeRom();
    if (r === null) return;
    romError = typeof r === 'string' ? r : '';
    rom = typeof r === 'string' ? null : r;
  };
  const apply = () => {
    if (premade && rom && matches) void app.usePremade(premade, rom);
  };
</script>

<Dialog id="d-preset" title="Premade Seed" lede="Recreate a randomization someone shared, from a seed and settings string or from a seed file. The same seed and settings make the same game in this app; games made by other randomizers from the same settings come out differently.">
  <div class="pills" role="radiogroup" aria-label="Source">
    <button class="pill" class:cur={source === 'seed'} role="radio" aria-checked={source === 'seed'} onclick={() => (source = 'seed')}>Seed &amp; settings string</button>
    <button class="pill" class:cur={source === 'file'} role="radio" aria-checked={source === 'file'} onclick={() => (source = 'file')}>Seed file</button>
  </div>
  {#if source === 'seed'}
    <label class="stack"><span class="lbl">Random seed</span><input class="inp mono" placeholder="e.g. 123456789012" bind:value={seed} /></label>
    <label class="stack"><span class="lbl">Settings string</span><input class="inp mono" placeholder="Paste a settings string" bind:value={str} /></label>
  {:else}
    <Field label="Seed file" value={file ? fileName(file.path) : 'No file chosen'}><button class="btn" onclick={chooseFile}>Browse…</button></Field>
  {/if}
  <Field label="ROM file" value={rom ? rom.name + ' — ' + rom.fileName : 'No ROM chosen'}><button class="btn" disabled={!premade} onclick={chooseRom}>Browse…</button></Field>
  <div class="warn"><span>{hint}</span></div>
  {#snippet footer()}
    <button class="btn ghost" onclick={() => app.closeModal()}>Cancel</button>
    <button class="btn primary lg" disabled={!matches} onclick={apply}>Apply Randomization Settings</button>
  {/snippet}
</Dialog>

<style>
  .stack { display: flex; flex-direction: column; gap: 8px; }
</style>
