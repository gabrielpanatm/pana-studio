<script lang="ts">
  let {
    label,
    description = undefined,
    value = $bindable(""),
    name = undefined,
    placeholder = "",
    disabled = false,
    required = false,
    rows = 4,
    code = false,
    spellcheck = true,
    oninput = undefined,
  }: {
    label: string;
    description?: string;
    value?: string;
    name?: string;
    placeholder?: string;
    disabled?: boolean;
    required?: boolean;
    rows?: number;
    code?: boolean;
    spellcheck?: boolean;
    oninput?: (value: string) => void;
  } = $props();

  const controlId = $props.id();
  const descriptionId = `${controlId}-description`;

  function updateValue(event: Event) {
    value = (event.currentTarget as HTMLTextAreaElement).value;
    oninput?.(value);
  }
</script>

<label class="ui-form-field" for={controlId}>
  <span class="ui-form-label">{label}</span>
  <textarea
    id={controlId}
    class="ui-textarea"
    class:code
    {name}
    {value}
    {placeholder}
    {disabled}
    {required}
    {rows}
    {spellcheck}
    aria-describedby={description ? descriptionId : undefined}
    oninput={updateValue}
  ></textarea>
  {#if description}
    <small id={descriptionId} class="ui-form-help">{description}</small>
  {/if}
</label>
