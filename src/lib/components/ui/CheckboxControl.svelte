<script lang="ts">
  let {
    label,
    description = undefined,
    checked = $bindable(false),
    name = undefined,
    disabled = false,
    compact = false,
    labelHidden = false,
    onchange = undefined,
  }: {
    label: string;
    description?: string;
    checked?: boolean;
    name?: string;
    disabled?: boolean;
    compact?: boolean;
    labelHidden?: boolean;
    onchange?: (checked: boolean) => void;
  } = $props();

  function updateChecked(event: Event) {
    checked = (event.currentTarget as HTMLInputElement).checked;
    onchange?.(checked);
  }
</script>

<label class="ui-checkbox" class:checked class:disabled class:compact class:label-hidden={labelHidden} aria-label={labelHidden ? label : undefined}>
  <input type="checkbox" {name} {checked} {disabled} onchange={updateChecked} />
  <span class="ui-checkbox-box" aria-hidden="true"><span></span></span>
  {#if !labelHidden}
    <span class="ui-checkbox-copy">
      <strong>{label}</strong>
      {#if description}<small>{description}</small>{/if}
    </span>
  {/if}
</label>
