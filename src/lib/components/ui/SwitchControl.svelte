<script lang="ts">
  import { t } from "$lib/i18n/runtime.svelte";

  let {
    label,
    description = undefined,
    checked = false,
    disabled = false,
    compact = false,
    onchange = undefined,
  }: {
    label: string;
    description?: string;
    checked?: boolean;
    disabled?: boolean;
    compact?: boolean;
    onchange?: (checked: boolean) => void;
  } = $props();
</script>

<label class="ui-switch" class:checked class:disabled class:compact>
  <span class="ui-switch-copy">
    <strong>{label}</strong>
    {#if description}<small>{description}</small>{/if}
  </span>
  <span class="ui-switch-action">
    <span class="ui-switch-state" aria-hidden="true">
      {checked ? t("common-enabled") : t("common-disabled")}
    </span>
    <input
      type="checkbox"
      role="switch"
      {checked}
      {disabled}
      onchange={(event) => onchange?.(event.currentTarget.checked)}
    />
    <span class="ui-switch-track" aria-hidden="true">
      <span class="ui-switch-thumb"></span>
    </span>
  </span>
</label>
