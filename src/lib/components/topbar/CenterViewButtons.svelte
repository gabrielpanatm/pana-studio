<script lang="ts">
  import { IconCode, IconEye, IconMarkdown } from "@tabler/icons-svelte";
  import ToolbarButton from "$lib/components/topbar/ToolbarButton.svelte";

  type CenterView = "preview" | "code" | "markdown" | "canvas" | "site" | "kernel";

  export let centerView: CenterView = "preview";
  export let canPreviewCurrentSource = true;
  export let isMarkdownSource = false;
  export let disabled = false;
  export let setCenterView: (view: CenterView) => void;
</script>

<ToolbarButton
  title="Preview"
  active={!disabled && centerView === "preview"}
  disabled={disabled || !canPreviewCurrentSource}
  segmented
  onclick={() => setCenterView("preview")}
>
  <IconEye size={17} stroke={1.8} />
</ToolbarButton>
<ToolbarButton
  title="Code"
  active={!disabled && centerView === "code"}
  disabled={disabled}
  segmented
  onclick={() => setCenterView("code")}
>
  <IconCode size={17} stroke={1.8} />
</ToolbarButton>
<ToolbarButton
  title="Markdown"
  active={!disabled && centerView === "markdown"}
  disabled={disabled || !isMarkdownSource}
  segmented
  onclick={() => setCenterView("markdown")}
>
  <IconMarkdown size={17} stroke={1.8} />
</ToolbarButton>
