<script lang="ts">
  import {
    IconDeviceDesktop,
    IconDeviceMobile,
    IconDeviceTablet,
  } from "@tabler/icons-svelte";
  import ToolbarButton from "$lib/components/topbar/ToolbarButton.svelte";

  type PreviewDevice = "desktop" | "tablet" | "mobile";

  export let previewDevice: PreviewDevice = "desktop";
  export let setPreviewDevice: (device: PreviewDevice) => void;
</script>

<ToolbarButton
  title="Desktop"
  active={previewDevice === "desktop"}
  segmented
  onclick={() => setPreviewDevice("desktop")}
>
  <IconDeviceDesktop size={17} stroke={1.8} />
</ToolbarButton>
<ToolbarButton
  title="Tablet"
  active={previewDevice === "tablet"}
  segmented
  onclick={() => setPreviewDevice("tablet")}
>
  <IconDeviceTablet size={17} stroke={1.8} />
</ToolbarButton>
<ToolbarButton
  title="Mobile"
  active={previewDevice === "mobile"}
  segmented
  onclick={() => setPreviewDevice("mobile")}
>
  <IconDeviceMobile size={17} stroke={1.8} />
</ToolbarButton>
