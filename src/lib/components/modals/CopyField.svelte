<script lang="ts">
  import Field from '../Field.svelte';

  let { label, value }: { label: string; value: string } = $props();
  let copied = $state(false);

  const copy = async () => {
    await navigator.clipboard?.writeText(value).catch(() => undefined);
    copied = true;
  };
</script>

<Field {label} {value}><button class="btn" onclick={copy}>{copied ? 'Copied' : 'Copy'}</button></Field>
