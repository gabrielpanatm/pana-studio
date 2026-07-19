<script lang="ts">
  import { IconCpu, IconHierarchy2, IconLayoutBoard } from "@tabler/icons-svelte";
  import ToolbarButton from "$lib/components/topbar/ToolbarButton.svelte";

  type CenterView = "preview" | "code" | "markdown" | "canvas" | "site" | "kernel";

  export let centerView: CenterView = "preview";
  export let disabled = false;
  export let setCenterView: (view: CenterView) => void;
</script>

<ToolbarButton
  title="Nucleu"
  active={!disabled && centerView === "kernel"}
  {disabled}
  onclick={() => setCenterView("kernel")}
>
  <IconCpu size={17} stroke={1.8} />
</ToolbarButton>
<ToolbarButton
  title="Site Editor"
  active={!disabled && centerView === "site"}
  {disabled}
  onclick={() => setCenterView("site")}
>
  <IconHierarchy2 size={17} stroke={1.8} />
</ToolbarButton>
<ToolbarButton
  title="Canvas"
  active={!disabled && centerView === "canvas"}
  {disabled}
  onclick={() => setCenterView("canvas")}
>
  <IconLayoutBoard size={17} stroke={1.8} />
</ToolbarButton>
