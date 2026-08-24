<script lang="ts">
  import type { HTMLInputAttributes } from "svelte/elements";

  let {
    label,
    description = undefined,
    value = $bindable(""),
    type = "text",
    name = undefined,
    placeholder = "",
    autocomplete = undefined,
    disabled = false,
    required = false,
    compact = false,
    min = undefined,
    max = undefined,
    pattern = undefined,
    oninput = undefined,
  }: {
    label: string;
    description?: string;
    value?: string;
    type?: "text" | "search" | "url" | "email" | "password" | "number";
    name?: string;
    placeholder?: string;
    autocomplete?: HTMLInputAttributes["autocomplete"];
    disabled?: boolean;
    required?: boolean;
    compact?: boolean;
    min?: string | number;
    max?: string | number;
    pattern?: string;
    oninput?: (value: string) => void;
  } = $props();

  const controlId = $props.id();
  const descriptionId = `${controlId}-description`;

  function updateValue(event: Event) {
    value = (event.currentTarget as HTMLInputElement).value;
    oninput?.(value);
  }
</script>

<label class="ui-form-field" for={controlId}>
  <span class="ui-form-label">{label}</span>
  <input
    id={controlId}
    class="ui-input"
    class:compact
    {type}
    {name}
    {value}
    {placeholder}
    {autocomplete}
    {disabled}
    {required}
    {min}
    {max}
    {pattern}
    aria-describedby={description ? descriptionId : undefined}
    oninput={updateValue}
  />
  {#if description}
    <small id={descriptionId} class="ui-form-help">{description}</small>
  {/if}
</label>
