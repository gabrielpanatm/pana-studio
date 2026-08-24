<script lang="ts">
  let {
    title,
    description = undefined,
    tone = "neutral",
    compact = false,
  }: {
    title: string;
    description?: string;
    tone?: "neutral" | "warning" | "error";
    compact?: boolean;
  } = $props();
</script>

<div class="ui-empty-state" class:compact class:warning={tone === "warning"} class:error={tone === "error"} role={tone === "error" ? "alert" : "status"}>
  <strong>{title}</strong>
  {#if description}<p>{description}</p>{/if}
</div>
