<script lang="ts" module>
  let nextInputId = 0;
</script>

<script lang="ts">
  import type { ScssVariable } from "$lib/types";
  import { IconBolt } from "@tabler/icons-svelte";
  import VariablePopover from "./VariablePopover.svelte";

  let {
    property,
    value = "",
    suggestions = [],
    oninput,
    oncommit,
    oncancel,
  }: {
    property: string;
    value?: string;
    suggestions?: ScssVariable[];
    oninput?: (value: string) => void;
    oncommit?: (value: string) => void;
    oncancel?: () => void;
  } = $props();

  const instanceId = nextInputId++;
  const uid        = $derived(`${property.replace(/[^a-z0-9]/g, "-")}-${instanceId}`);
  const inputId    = $derived(`ci-input-${uid}`);

  let draftValue   = $state("");
  let focused      = $state(false);
  let alphaFocused = $state(false);
  let alphaStr     = $state("1");

  let root             = $state<HTMLDivElement | null>(null);
  let showSuggestions  = $state(false);
  let skipNextCommit = false;

  // ── Color helpers ────────────────────────────────────────────────────────

  function parseColor(v: string): { hex: string; alpha: number } | null {
    const s = v.trim();

    // #rrggbb / #rgb
    const hexM = s.match(/^#([0-9a-fA-F]{6}|[0-9a-fA-F]{3})$/);
    if (hexM) {
      const h = hexM[1];
      const hex = h.length === 3
        ? `#${h[0]}${h[0]}${h[1]}${h[1]}${h[2]}${h[2]}`
        : `#${h}`;
      return { hex, alpha: 1 };
    }

    // rgb() / rgba()
    const rgbaM = s.match(/^rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)(?:\s*,\s*([\d.]+))?\s*\)$/);
    if (rgbaM) {
      const r = parseInt(rgbaM[1]);
      const g = parseInt(rgbaM[2]);
      const b = parseInt(rgbaM[3]);
      const a = rgbaM[4] !== undefined ? parseFloat(rgbaM[4]) : 1;
      const hex = `#${r.toString(16).padStart(2,"0")}${g.toString(16).padStart(2,"0")}${b.toString(16).padStart(2,"0")}`;
      return { hex, alpha: a };
    }

    // hsl() / hsla()
    const hslaM = s.match(/^hsla?\(\s*([\d.]+)\s*,\s*([\d.]+)%\s*,\s*([\d.]+)%(?:\s*,\s*([\d.]+))?\s*\)$/);
    if (hslaM) {
      const h   = parseFloat(hslaM[1]) / 360;
      const sat = parseFloat(hslaM[2]) / 100;
      const l   = parseFloat(hslaM[3]) / 100;
      const a   = hslaM[4] !== undefined ? parseFloat(hslaM[4]) : 1;
      let r: number, g: number, b: number;
      if (sat === 0) {
        r = g = b = l;
      } else {
        const hue2rgb = (p: number, q: number, t: number) => {
          if (t < 0) t += 1;
          if (t > 1) t -= 1;
          if (t < 1/6) return p + (q - p) * 6 * t;
          if (t < 1/2) return q;
          if (t < 2/3) return p + (q - p) * (2/3 - t) * 6;
          return p;
        };
        const q = l < 0.5 ? l * (1 + sat) : l + sat - l * sat;
        const p = 2 * l - q;
        r = hue2rgb(p, q, h + 1/3);
        g = hue2rgb(p, q, h);
        b = hue2rgb(p, q, h - 1/3);
      }
      const hex = `#${Math.round(r*255).toString(16).padStart(2,"0")}${Math.round(g*255).toString(16).padStart(2,"0")}${Math.round(b*255).toString(16).padStart(2,"0")}`;
      return { hex, alpha: a };
    }

    return null;
  }

  function serializeColor(hex: string, alpha: number): string {
    const a = parseFloat(alpha.toFixed(2));
    if (a >= 1) return hex;
    const r = parseInt(hex.slice(1, 3), 16);
    const g = parseInt(hex.slice(3, 5), 16);
    const b = parseInt(hex.slice(5, 7), 16);
    return `rgba(${r}, ${g}, ${b}, ${a})`;
  }

  const parsedColor = $derived(parseColor(draftValue));

  // Resolve SCSS variable to its actual color for display
  const resolvedColor = $derived.by(() => {
    if (parsedColor) return parsedColor;
    const raw = draftValue.trim();
    if (!raw.startsWith("$")) return null;
    const varName = raw.slice(1);
    const found = suggestions.find((s) => s.name === varName);
    return found ? parseColor(found.value) : null;
  });

  // ── Sync effects ─────────────────────────────────────────────────────────

  $effect(() => {
    if (!focused && value !== draftValue) {
      draftValue = value;
    }
  });

  $effect(() => {
    if (!alphaFocused && parsedColor) {
      alphaStr = String(parsedColor.alpha);
    }
  });

  // ── Handlers ─────────────────────────────────────────────────────────────

  function handleHexChange(hex: string) {
    const alpha = parsedColor?.alpha ?? 1;
    const next  = serializeColor(hex, alpha);
    draftValue  = next;
    oninput?.(next);
  }

  function handleAlphaInput(raw: string) {
    alphaStr = raw;
    if (raw.endsWith(".")) return;
    const a = parseFloat(raw);
    if (!isNaN(a) && parsedColor) {
      const next = serializeColor(parsedColor.hex, Math.min(1, Math.max(0, a)));
      draftValue = next;
      oninput?.(next);
    }
  }

  // ── Variable suggestions ─────────────────────────────────────────────────

  const filteredSuggestions = $derived.by(() => {
    const query = draftValue.trim().replace(/^\$/, "").toLowerCase();
    if (!query) return suggestions;
    return suggestions.filter((s) =>
      s.name.toLowerCase().includes(query) || s.value.toLowerCase().includes(query)
    );
  });

  function selectSuggestion(variable: ScssVariable) {
    const next = `$${variable.name}`;
    draftValue  = next;
    oninput?.(next);
    oncommit?.(next);
    showSuggestions = false;
    document.getElementById(inputId)?.focus();
  }

  function handleFocusOut(event: FocusEvent) {
    const next = event.relatedTarget;
    if (next instanceof Node && root?.contains(next)) return;
    showSuggestions = false;
    if (skipNextCommit) {
      skipNextCommit = false;
      return;
    }
    oncommit?.(draftValue);
  }

  function cancelEdit(input: HTMLInputElement) {
    skipNextCommit = true;
    draftValue = value;
    showSuggestions = false;
    oncancel?.();
    input.blur();
  }
</script>

<div class="color-input" class:has-value={!!draftValue} bind:this={root} onfocusout={handleFocusOut}>
  {#if resolvedColor}
    <input
      type="color"
      class="color-swatch"
      value={resolvedColor.hex}
      title="Alege culoare"
      oninput={(e) => handleHexChange(e.currentTarget.value)}
      onchange={() => oncommit?.(draftValue)}
    />
    {#if parsedColor && parsedColor.alpha < 1}
      <div class="alpha-wrap">
        <span class="alpha-label">A</span>
        <input
          type="text"
          class="alpha-field"
          value={alphaStr}
          title="Opacitate (0–1)"
          autocomplete="off"
          onfocus={() => { alphaFocused = true; }}
          onblur={() => { alphaFocused = false; }}
          onkeydown={(event) => {
            if (event.key === "Escape") {
              event.preventDefault();
              cancelEdit(event.currentTarget);
            } else if (event.key === "Enter") {
              event.preventDefault();
              event.currentTarget.blur();
            }
          }}
          oninput={(e) => handleAlphaInput(e.currentTarget.value)}
        />
      </div>
    {/if}
  {:else}
    <label class="color-swatch-placeholder" title="Alege culoare">
      <input
        type="color"
        class="color-picker-hidden"
        value="#000000"
        oninput={(e) => {
          draftValue = e.currentTarget.value;
          oninput?.(draftValue);
        }}
        onchange={() => oncommit?.(draftValue)}
      />
    </label>
  {/if}

  {#if suggestions.length}
    <button
      type="button"
      class="var-btn"
      title="Inserează variabilă SCSS"
      onclick={() => {
        showSuggestions = !showSuggestions;
        document.getElementById(inputId)?.focus();
      }}
    ><IconBolt size={11} stroke={2} /></button>
  {/if}

  <input
    id={inputId}
    type="text"
    class="color-field"
    value={draftValue}
    placeholder="—"
    autocomplete="off"
    onfocus={() => { focused = true; if (filteredSuggestions.length) showSuggestions = true; }}
    onblur={() => { focused = false; }}
    onkeydown={(e) => {
      if (e.key === "Escape") {
        e.preventDefault();
        cancelEdit(e.currentTarget);
      } else if (e.key === "Enter") {
        e.preventDefault();
        e.currentTarget.blur();
      }
    }}
    oninput={(e) => {
      draftValue = e.currentTarget.value;
      oninput?.(draftValue);
    }}
  />

  {#if showSuggestions && filteredSuggestions.length}
    <VariablePopover anchor={root} suggestions={filteredSuggestions} onselect={selectSuggestion} />
  {/if}
</div>

<style>
  .color-input {
    position: relative;
    display: flex;
    align-items: center;
    border: 1px solid var(--border-4);
    border-radius: 6px;
    background: var(--surface-8);
    overflow: visible;
    min-width: 0;
  }

  .color-input:focus-within {
    border-color: var(--brand);
  }

  /* ── Swatch ─────────────────────────────────────────────────────────── */

  .color-swatch {
    width: 26px;
    height: 24px;
    flex-shrink: 0;
    border: none;
    border-right: 1px solid var(--border-4);
    padding: 2px;
    background: var(--surface-4);
    cursor: pointer;
    border-radius: 5px 0 0 5px;
  }

  .color-swatch-placeholder {
    position: relative;
    width: 26px;
    height: 24px;
    flex-shrink: 0;
    border-right: 1px solid var(--border-4);
    border-radius: 5px 0 0 5px;
    background: repeating-linear-gradient(
      45deg,
      var(--border-4) 0px,
      var(--border-4) 3px,
      transparent 3px,
      transparent 7px
    );
    cursor: pointer;
    overflow: hidden;
  }

  .color-picker-hidden {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    opacity: 0;
    cursor: pointer;
  }

  /* ── Alpha ──────────────────────────────────────────────────────────── */

  .alpha-wrap {
    display: flex;
    align-items: center;
    height: 24px;
    border-right: 1px solid var(--border-4);
    flex-shrink: 0;
  }

  .alpha-label {
    padding: 0 4px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-muted);
    font-family: "JetBrains Mono", monospace;
    user-select: none;
    background: var(--surface-4);
    height: 100%;
    display: flex;
    align-items: center;
    border-right: 1px solid var(--border-4);
  }

  .alpha-field {
    width: 34px;
    height: 100%;
    padding: 0 4px;
    border: none;
    background: transparent;
    color: var(--text);
    font-size: 12px;
    font-family: "JetBrains Mono", monospace;
    outline: none;
    text-align: center;
  }

  /* ── Variable button ────────────────────────────────────────────────── */

  .var-btn {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 24px;
    padding: 0;
    border: none;
    border-right: 1px solid var(--border-4);
    font-size: 12px;
    line-height: 1;
    background: var(--surface-4);
    cursor: pointer;
    color: var(--text-muted);
  }

  .var-btn:hover {
    background: var(--brand-soft);
    color: var(--brand-strong);
  }

  /* ── Text field ─────────────────────────────────────────────────────── */

  .color-field {
    flex: 1;
    min-width: 0;
    height: 24px;
    padding: 0 6px;
    border: none;
    color: var(--text);
    font-size: 12px;
    background: transparent;
    font-family: "JetBrains Mono", "SFMono-Regular", Consolas, monospace;
    outline: none;
  }
</style>
