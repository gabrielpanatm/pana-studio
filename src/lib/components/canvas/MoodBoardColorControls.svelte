<script lang="ts">
  import { IconCodeVariable } from "@tabler/icons-svelte";
  import { moodBoardColorWithValue } from "$lib/mood-board/context-actions";
  import { cloneMoodBoardItem } from "$lib/mood-board/item-view";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import type { MoodBoardColorItem, MoodBoardItem } from "$lib/mood-board/model";
  import type { ScssVariable } from "$lib/types";

  export let colorItem: MoodBoardColorItem;
  export let scssVariables: ScssVariable[] = [];
  export let previewItem: (item: MoodBoardItem) => void;
  export let commitItemEdit: (beforeItem: MoodBoardItem, nextItem: MoodBoardItem) => void;
  export let applyColorToScssVariable: ((color: string, label: string, variableName?: string) => void | Promise<void>) | undefined = undefined;

  let editBeforeItem: MoodBoardColorItem | null = null;
  let variablePickerOpen = false;

  $: colorVariables = scssVariables.filter((variable) => {
    const name = variable.name.toLowerCase();
    const value = variable.value.trim().toLowerCase();
    return name.includes("color")
      || name.startsWith("bg-")
      || name.startsWith("text-")
      || name.startsWith("border-")
      || value.startsWith("#")
      || value.startsWith("rgb")
      || value.startsWith("hsl");
  });
  $: variableOptions = [
    { value: "", label: "Variabilă..." },
    ...colorVariables.map((variable) => ({
      value: variable.name,
      label: `$${variable.name}`,
      detail: variable.value,
    })),
  ];

  function cloneItem(value: MoodBoardColorItem): MoodBoardColorItem {
    return cloneMoodBoardItem(value);
  }

  function beginEdit() {
    editBeforeItem ??= cloneItem(colorItem);
  }

  function commitEdit(nextItem: MoodBoardColorItem) {
    if (!editBeforeItem) return;
    const before = editBeforeItem;
    editBeforeItem = null;
    commitItemEdit(before, nextItem);
  }

  function updateColorValue(value: string) {
    beginEdit();
    previewItem(moodBoardColorWithValue(colorItem, value));
  }

  function commitColorValue(value: string) {
    commitEdit(moodBoardColorWithValue(colorItem, value));
  }

  function chooseVariable(name: string) {
    if (!applyColorToScssVariable || !name) return;
    variablePickerOpen = false;
    void applyColorToScssVariable(colorItem.color, colorItem.label || "culoare", name);
  }
</script>

<span class="separator"></span>
<label class="color-control" style={`--control-color:${colorItem.color};`} title="Culoare">
  <input
    type="color"
    value={colorItem.color}
    aria-label="Culoare"
    onfocus={beginEdit}
    oninput={(event) => updateColorValue(event.currentTarget.value)}
    onchange={(event) => commitColorValue(event.currentTarget.value)}
    onblur={(event) => commitColorValue(event.currentTarget.value)}
  />
</label>
<span class="muted">{colorItem.color}</span>
{#if applyColorToScssVariable}
  <div class="variable-wrap">
    <button type="button" class:active={variablePickerOpen} title="Trimite către variabilă SCSS" onclick={() => (variablePickerOpen = !variablePickerOpen)}>
      <IconCodeVariable size={15} stroke={2} />
    </button>
    {#if variablePickerOpen}
      <div class="variable-select">
        <SelectControl value="" options={variableOptions} ariaLabel="Alege variabilă SCSS" onchange={chooseVariable} />
      </div>
    {/if}
  </div>
{/if}
