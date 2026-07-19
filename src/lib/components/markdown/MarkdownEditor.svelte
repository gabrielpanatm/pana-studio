<script lang="ts">
  import { onMount } from "svelte";
  import type TipTapMarkdownEditorComponent from "$lib/components/markdown/TipTapMarkdownEditor.svelte";
  import { splitMarkdownFrontmatter, joinMarkdownFrontmatter } from "$lib/markdown/frontmatter";
  import { queueFileBufferDraftFlushSnapshotForPath } from "$lib/session/file-buffer-draft-sync";

  export let source = "";
  export let path = "";
  export let refreshToken = 0;
  export let readOnly = false;
  export let onChange: (nextSource: string, path: string) => void;

  let TipTapMarkdownEditor: typeof TipTapMarkdownEditorComponent | null = null;
  let frontmatter = "";
  let marker: "---" | "+++" | "" = "";
  let body = "";
  let lastSource = "";

  onMount(() => {
    let mounted = true;

    import("$lib/components/markdown/TipTapMarkdownEditor.svelte").then((component) => {
      if (mounted) TipTapMarkdownEditor = component.default;
    });

    return () => {
      mounted = false;
    };
  });

  $: if (source !== lastSource) {
    const parts = splitMarkdownFrontmatter(source);
    marker = parts.marker;
    frontmatter = parts.frontmatter;
    body = parts.body;
    lastSource = source;
  }

  function emit(nextBody = body) {
    const nextSource = joinMarkdownFrontmatter({ marker, frontmatter, body: nextBody });
    lastSource = nextSource;
    onChange(nextSource, path);
  }

  function handleBodyChange(nextBody: string) {
    body = nextBody;
    emit(body);
  }

  function handleFlushSnapshot(nextBody: string) {
    body = nextBody;
    const nextSource = joinMarkdownFrontmatter({ marker, frontmatter, body: nextBody });
    if (nextSource !== lastSource) {
      lastSource = nextSource;
      onChange(nextSource, path);
    }
    queueFileBufferDraftFlushSnapshotForPath(
      path,
      nextSource,
      "markdown.editor.flush",
    );
  }
</script>

<section class="markdown-editor" aria-label="Editor Markdown">
  {#key `${path}:${refreshToken}`}
    {#if TipTapMarkdownEditor}
      <svelte:component
        this={TipTapMarkdownEditor}
        markdown={body}
        onChange={handleBodyChange}
        flushId={path}
        {readOnly}
        onFlushSnapshot={handleFlushSnapshot}
      />
    {:else}
      <div class="markdown-loading" aria-label="Se încarcă editorul Markdown"></div>
    {/if}
  {/key}
</section>

<style>
  .markdown-editor {
    position: relative;
    display: grid;
    grid-template-rows: minmax(0, 1fr);
    min-height: 0;
    height: 100%;
    background: var(--surface);
  }

  .markdown-loading {
    min-height: 0;
    height: 100%;
    background: var(--surface);
  }
</style>
