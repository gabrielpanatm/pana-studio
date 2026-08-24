<script lang="ts">
  let {
    message,
    tone = "info",
  }: {
    message: string;
    tone?: "info" | "success" | "warning" | "error";
  } = $props();
</script>

<p class="ui-message" class:success={tone === "success"} class:warning={tone === "warning"} class:error={tone === "error"} role={tone === "error" ? "alert" : "status"}>
  {message}
</p>
