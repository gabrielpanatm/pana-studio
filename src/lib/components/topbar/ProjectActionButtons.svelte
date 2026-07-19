<script lang="ts">
  import {
    IconCircleCheck,
    IconExternalLink,
    IconFolderOpen,
    IconRefresh,
    IconRotateClockwise,
    IconX,
  } from "@tabler/icons-svelte";
  import ToolbarButton from "$lib/components/topbar/ToolbarButton.svelte";

  export let noProject = false;
  export let canOpenInBrowser = false;
  export let openProjectFolder: () => void;
  export let closeCurrentProject: () => void | Promise<void>;
  export let openCurrentProjectInBrowser: () => void | Promise<void>;
  export let rescanCurrentProject: () => void;
  export let refreshCurrentSession: () => void | Promise<void>;
  export let validateZola: () => void | Promise<void>;
</script>

<ToolbarButton title="Deschide dosar proiect" cta={noProject} onclick={openProjectFolder}>
  <IconFolderOpen size={17} stroke={1.8} />
</ToolbarButton>
<ToolbarButton
  title="Închide proiectul curent"
  disabled={noProject}
  onclick={closeCurrentProject}
>
  <IconX size={17} stroke={1.9} />
</ToolbarButton>
<ToolbarButton
  title="Purge complet: șterge sesiunea și reîncarcă proiectul din disk"
  onclick={rescanCurrentProject}
>
  <IconRefresh size={17} stroke={1.8} />
</ToolbarButton>
<ToolbarButton
  title="Reaplică sesiunea curentă peste editor, inspector și preview"
  disabled={noProject}
  onclick={refreshCurrentSession}
>
  <IconRotateClockwise size={17} stroke={1.8} />
</ToolbarButton>
<ToolbarButton
  title="Validează proiectul cu zola check"
  disabled={noProject}
  onclick={validateZola}
>
  <IconCircleCheck size={17} stroke={1.8} />
</ToolbarButton>
<ToolbarButton
  title="Deschide în browser"
  disabled={!canOpenInBrowser}
  onclick={openCurrentProjectInBrowser}
>
  <IconExternalLink size={17} stroke={1.8} />
</ToolbarButton>
