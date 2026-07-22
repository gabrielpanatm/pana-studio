<script lang="ts">
  import type { ScssVariable } from "$lib/types";
  import type { ScssVariableEditController } from "$lib/inspector/css-property-edit";

  type VarGroup = { label: string; vars: ScssVariable[] };

  let {
    scssVariables = [],
    pendingVarValues = {},
    edit,
  }: {
    scssVariables?: ScssVariable[];
    pendingVarValues?: Record<string, string>;
    edit: ScssVariableEditController;
  } = $props();

  function groupVariables(vars: ScssVariable[]): VarGroup[] {
    const isColor = (value: string) =>
      value.startsWith("#") || value.startsWith("rgb") || value.startsWith("hsl") || value.includes("color-mix");
    const groups: Record<string, ScssVariable[]> = {
      culori: [],
      tipografie: [],
      spatiere: [],
      radius: [],
      umbre: [],
      tranzitii: [],
      layout: [],
      altele: [],
    };
    for (const variable of vars) {
      const name = variable.name;
      if (
        name.startsWith("color-") ||
        name.startsWith("bg-") ||
        name.startsWith("border-color") ||
        name.startsWith("border-strong") ||
        (name.startsWith("text-") && isColor(variable.value))
      ) {
        groups.culori.push(variable);
      } else if (
        name.startsWith("text-") ||
        name.startsWith("font-") ||
        name.startsWith("leading-") ||
        name.startsWith("tracking-")
      ) {
        groups.tipografie.push(variable);
      } else if (name.startsWith("space-")) {
        groups.spatiere.push(variable);
      } else if (name.startsWith("radius-")) {
        groups.radius.push(variable);
      } else if (name.startsWith("shadow-")) {
        groups.umbre.push(variable);
      } else if (name.startsWith("transition-")) {
        groups.tranzitii.push(variable);
      } else if (name.startsWith("bp-") || name.startsWith("container-") || name.startsWith("z-")) {
        groups.layout.push(variable);
      } else {
        groups.altele.push(variable);
      }
    }
    const labels: Record<string, string> = {
      culori: "Culori",
      tipografie: "Tipografie",
      spatiere: "Spațiere",
      radius: "Border Radius",
      umbre: "Umbre",
      tranzitii: "Tranziții",
      layout: "Layout",
      altele: "Altele",
    };
    return Object.entries(groups)
      .filter(([, variables]) => variables.length > 0)
      .map(([key, variables]) => ({ label: labels[key], vars: variables }));
  }

  function isLikelyColor(value: string): boolean {
    return /^#[0-9a-fA-F]{3,8}$/.test(value.trim());
  }
</script>

{#if scssVariables.length === 0}
  <p class="hint">Deschide un proiect Zola cu fișiere SCSS pentru a vedea variabilele.</p>
{:else}
  {@const groups = groupVariables(scssVariables)}

  {#each groups as group}
    <section class="inspector-group">
      <div class="group-header"><h3>{group.label}</h3></div>
      <div class="vars-list">
        {#each group.vars as variable}
          {@const key = variable.file + "|" + variable.name}
          {@const currentValue = pendingVarValues[key] ?? variable.value}
          {@const isDirty = pendingVarValues[key] !== undefined}
          {@const isColor = isLikelyColor(currentValue)}
          <div class="var-row" class:dirty={isDirty}>
            <span class="var-name">${variable.name}</span>
            <div class="var-edit">
              {#if isColor}
                <input
                  class="color-swatch"
                  type="color"
                  value={currentValue}
                  oninput={(event) => edit.draft(variable, event.currentTarget.value)}
                  onchange={(event) => edit.commit(variable, event.currentTarget.value)}
                />
              {/if}
              <input
                class="var-input"
                type="text"
                value={currentValue}
                oninput={(event) => edit.draft(variable, event.currentTarget.value)}
                onblur={() => edit.commit(variable)}
                onkeydown={(event) => {
                  if (event.key === "Escape") {
                    event.preventDefault();
                    edit.cancel(variable);
                    event.currentTarget.blur();
                  } else if (event.key === "Enter") {
                    event.preventDefault();
                    event.currentTarget.blur();
                  }
                }}
              />
            </div>
          </div>
        {/each}
      </div>
    </section>
  {/each}
{/if}

<style>
  .hint {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
    line-height: 1.45;
  }

  .inspector-group {
    display: grid;
    gap: 9px;
    padding: 10px;
    border: 1px solid var(--border-2);
    border-radius: 9px;
    background: var(--surface-2);
  }

  .group-header {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .group-header h3 {
    margin: 0;
    font-size: 12px;
    font-weight: 900;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .vars-list {
    display: grid;
    gap: 5px;
  }

  .var-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 160px;
    gap: 6px;
    align-items: center;
    min-height: 28px;
    padding: 0 8px;
    border: 1px solid var(--border-4);
    border-radius: 8px;
    background: var(--surface-5);
  }

  .var-row.dirty {
    border-color: var(--brand);
    background: var(--brand-soft);
  }

  .var-name {
    color: var(--text-muted);
    font-size: 12px;
    font-family: "JetBrains Mono", "SFMono-Regular", Consolas, monospace;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .var-edit {
    display: flex;
    align-items: center;
    gap: 5px;
  }

  .color-swatch {
    flex-shrink: 0;
    width: 22px;
    height: 22px;
    padding: 2px;
    border: 1px solid var(--border-4);
    border-radius: 5px;
    background: transparent;
    cursor: pointer;
  }

  .var-input {
    flex: 1;
    min-width: 0;
    min-height: 26px;
    padding: 0 7px;
    border: 1px solid var(--border-4);
    border-radius: 6px;
    color: var(--text);
    font-family: "JetBrains Mono", "SFMono-Regular", Consolas, monospace;
    font-size: 12px;
    background: var(--surface-8);
  }
</style>
