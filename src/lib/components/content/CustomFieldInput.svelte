<script lang="ts">
  import { IconPhoto, IconPlus, IconTrash } from "@tabler/icons-svelte";
  import type { ContentFieldDefinition } from "$lib/types";
  import CustomFieldInput from "./CustomFieldInput.svelte";

  let {
    field,
    value,
    path,
    onValueChange,
  }: {
    field: ContentFieldDefinition;
    value: unknown;
    path: string;
    onValueChange: (value: unknown) => void;
  } = $props();

  const objectValue = $derived(
    typeof value === "object" && value !== null && !Array.isArray(value)
      ? value as Record<string, unknown>
      : {},
  );
  const arrayValue = $derived(Array.isArray(value) ? value as unknown[] : []);

  function textValue() {
    return typeof value === "string" ? value : "";
  }

  function numberValue() {
    return typeof value === "number" ? String(value) : "";
  }

  function updateObjectField(key: string, nextValue: unknown) {
    const next = { ...objectValue };
    if (nextValue === undefined) delete next[key];
    else next[key] = nextValue;
    onValueChange(next);
  }

  function updateRepeaterItem(index: number, key: string, nextValue: unknown) {
    const next = arrayValue.map((item, itemIndex) => {
      if (itemIndex !== index) return item;
      const object = typeof item === "object" && item !== null && !Array.isArray(item)
        ? item as Record<string, unknown>
        : {};
      const nextItem = { ...object };
      if (nextValue === undefined) delete nextItem[key];
      else nextItem[key] = nextValue;
      return nextItem;
    });
    onValueChange(next);
  }

  function addRepeaterItem() {
    const item = Object.fromEntries(
      field.fields
        .filter((child) => child.defaultValue !== undefined)
        .map((child) => [child.key, child.defaultValue]),
    );
    onValueChange([...arrayValue, item]);
  }

  function removeRepeaterItem(index: number) {
    onValueChange(arrayValue.filter((_, itemIndex) => itemIndex !== index));
  }
</script>

<div class="field" class:required={field.required} class:structured={field.kind === "group" || field.kind === "repeater"}>
  <div class="field-heading">
    <span>{field.label}{#if field.required}<i>*</i>{/if}</span>
    <code>{path}</code>
  </div>

  {#if field.kind === "group"}
    <div class="nested-fields">
      {#each field.fields as child (child.id)}
        <CustomFieldInput
          field={child}
          value={objectValue[child.key] ?? child.defaultValue}
          path={`${path}.${child.key}`}
          onValueChange={(nextValue: unknown) => updateObjectField(child.key, nextValue)}
        />
      {:else}
        <p class="empty">Grupul nu definește încă subcâmpuri.</p>
      {/each}
    </div>
  {:else if field.kind === "repeater"}
    <div class="repeater">
      {#each arrayValue as item, index (`${field.id}:${index}`)}
        <section class="repeater-item">
          <header><strong>Element {index + 1}</strong><button type="button" aria-label="Șterge elementul" onclick={() => removeRepeaterItem(index)}><IconTrash size={13} /></button></header>
          <div class="nested-fields">
            {#each field.fields as child (child.id)}
              <CustomFieldInput
                field={child}
                value={(typeof item === "object" && item !== null && !Array.isArray(item) ? (item as Record<string, unknown>)[child.key] : undefined) ?? child.defaultValue}
                path={`${path}[${index}].${child.key}`}
                onValueChange={(nextValue: unknown) => updateRepeaterItem(index, child.key, nextValue)}
              />
            {/each}
          </div>
        </section>
      {:else}
        <p class="empty">Lista este goală.</p>
      {/each}
      <button class="add-item" type="button" onclick={addRepeaterItem}><IconPlus size={13} /> Adaugă element</button>
    </div>
  {:else if field.kind === "textarea" || field.kind === "markdown"}
    <textarea value={textValue()} rows={field.kind === "markdown" ? 7 : 4} oninput={(event) => onValueChange(event.currentTarget.value)}></textarea>
  {:else if field.kind === "number"}
    <input type="number" value={numberValue()} min={field.minimum} max={field.maximum} oninput={(event) => onValueChange(event.currentTarget.value === "" ? undefined : Number(event.currentTarget.value))} />
  {:else if field.kind === "boolean"}
    <span class="toggle"><input type="checkbox" checked={value === true} onchange={(event) => onValueChange(event.currentTarget.checked)} /> Activ</span>
  {:else if field.kind === "select"}
    <select value={textValue()} onchange={(event) => onValueChange(event.currentTarget.value)}><option value="">Alege…</option>{#each field.choices as choice (choice.value)}<option value={choice.value}>{choice.label}</option>{/each}</select>
  {:else if field.kind === "color"}
    <span class="color-control"><input type="color" value={/^#[0-9a-f]{6}$/i.test(textValue()) ? textValue() : "#000000"} oninput={(event) => onValueChange(event.currentTarget.value)} /><input value={textValue()} placeholder="#000000" oninput={(event) => onValueChange(event.currentTarget.value)} /></span>
  {:else if field.kind === "image"}
    <span class="image-control"><IconPhoto size={15} /><input value={textValue()} placeholder="/imagini/exemplu.jpg" oninput={(event) => onValueChange(event.currentTarget.value)} /></span>
  {:else}
    <input type={field.kind === "date" ? "date" : field.kind === "url" ? "url" : "text"} value={textValue()} pattern={field.pattern} oninput={(event) => onValueChange(event.currentTarget.value)} />
  {/if}

  {#if field.help}<small>{field.help}</small>{/if}
</div>

<style>
  .field { display: grid; gap: 4px; padding: 8px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; color: var(--wb-text-muted); background: var(--wb-surface-document); font-size: 11px; font-weight: 700; }
  .field.structured { background: var(--wb-surface-chrome); }
  .field-heading { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
  .field-heading > span { color: var(--text-strong); font-size: 12px; }
  .field-heading i { margin-left: 2px; color: var(--danger); font-style: normal; }
  .field-heading code { color: var(--wb-text-muted); font-size: 11px; font-weight: 500; }
  input, select, textarea { width: 100%; min-width: 0; min-height: 31px; padding: 6px 7px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--material-inset); font: inherit; font-weight: 500; }
  textarea { resize: vertical; line-height: 1.45; }
  small { font-weight: 500; line-height: 1.4; }
  .toggle { display: flex; align-items: center; gap: 7px; min-height: 31px; text-transform: none; }
  .toggle input { width: 16px; min-height: auto; }
  .color-control, .image-control { display: flex; align-items: center; gap: 5px; }
  .color-control input[type="color"] { width: 36px; padding: 3px; }
  .nested-fields, .repeater { display: grid; gap: 7px; }
  .repeater-item { display: grid; gap: 6px; padding: 7px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); }
  .repeater-item > header { display: flex; align-items: center; justify-content: space-between; }
  .repeater-item strong { color: var(--text-strong); font-size: 11px; }
  button { display: inline-flex; min-height: 28px; align-items: center; justify-content: center; gap: 4px; padding: 0 8px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); }
  .repeater-item header button { width: 27px; padding: 0; color: var(--danger); }
  .add-item { justify-self: start; }
  .empty { margin: 0; padding: 8px; color: var(--wb-text-muted); font-size: 11px; font-weight: 500; text-align: center; }
</style>
