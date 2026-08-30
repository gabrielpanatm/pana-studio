diagnostic-application-settings-stale = The application settings expected revision { $expected }, but Rust owns revision { $actual }.
diagnostic-application-settings-load-failed = The application settings could not be loaded.
diagnostic-application-settings-save-failed = The application settings could not be saved.
diagnostic-application-settings-invalid-language = The interface language “{ $locale }” is not included in this version of Pană Studio.
diagnostic-application-settings-invalid-accent = A custom accent must be an sRGB color in #RRGGBB format.
diagnostic-application-settings-system-refresh-failed = System preferences could not be reapplied.
diagnostic-application-settings-layout-save-failed = The Inspector layout could not be saved.
diagnostic-system-preferences-live-unavailable = System preferences will not update live.
diagnostic-disk-conflict-file-missing = The tracked file is missing from disk relative to the session baseline.
diagnostic-disk-conflict-file-not-file = The tracked path is no longer a text file.
diagnostic-disk-conflict-file-oversized = The disk file has { $size } bytes, above the FileBufferStore limit of { $limit } bytes.
diagnostic-disk-conflict-file-invalid-path = The tracked path cannot be read inside the project boundary: { $detail }
diagnostic-disk-conflict-file-unreadable = The disk file cannot be read as text for conflict checking: { $detail }
diagnostic-disk-conflict-file-readonly = The disk file is read-only; Save Engine would block writing.
diagnostic-disk-conflict-file-changed = The disk content differs from the FileBufferStore baseline.
diagnostic-disk-conflict-file-metadata = Disk metadata differs, but the text hash matches the baseline.
diagnostic-disk-conflict-file-draft = An in-memory draft exists and the disk is still at the session baseline.
diagnostic-disk-conflict-file-clean = The disk matches the FileBufferStore baseline.
diagnostic-disk-conflict-summary-empty = FileBufferStore is not tracking files for conflict checking yet.
diagnostic-disk-conflict-summary-error =
    { $count ->
        [one] { $count } file cannot be checked safely against disk.
       *[other] { $count } files cannot be checked safely against disk.
    }
diagnostic-disk-conflict-summary-warning =
    { $count ->
        [one] { $count } file differs from the baseline or would block Save Engine.
       *[other] { $count } files differ from the baseline or would block Save Engine.
    }
diagnostic-disk-conflict-summary-info = { $drafts } local drafts and { $metadata } metadata changes without a hash conflict.
diagnostic-disk-conflict-summary-clean =
    { $count ->
        [one] { $count } tracked file is aligned with disk.
       *[other] { $count } tracked files are aligned with disk.
    }
source-graph-not-zola-project = The current project does not appear to be a valid Zola project.
source-graph-conventional-data-invalid = Source Graph could not catalog a conventional data file: { $details }
source-graph-load-data-missing = Local Zola file referenced by load_data was not found: { $path }
source-graph-load-data-unresolved = load_data(path={ $path }) cannot be cataloged: { $details }
source-graph-data-toml-invalid = Invalid TOML data document: { $details }
source-graph-data-format-invalid = Invalid { $format } data document: { $details }
source-graph-config-toml-invalid = Invalid TOML configuration: { $details }
source-graph-content-target-missing = Referenced Zola content was not found: { $target }
source-graph-template-target-missing = Referenced template was not found: { $target }
source-graph-content-tera-syntax-invalid = The Markdown content contains invalid Tera 2 syntax: { $details }
source-graph-legacy-tera-incompatible = This template uses Tera 1 macro/import syntax, which is incompatible with Zola 0.23.4 and Tera 2.
source-graph-legacy-shortcode-template-incompatible = Legacy shortcode template directories are incompatible with Zola 0.23.4; define a Tera 2 component instead.
source-graph-legacy-shortcode-incompatible = Legacy shortcode call “{ $name }” is incompatible with Zola 0.23.4; use a Tera 2 component call.
source-graph-zola-runtime-argument-deprecated = { $function } argument “{ $argument }” is deprecated; use “{ $replacement }” instead.
source-graph-page-template-missing = The page template was not found: { $template }
source-graph-section-page-template-missing = The section page_template was not found: { $template }
source-graph-frontmatter-invalid = Invalid { $format } front matter: { $details }
source-graph-projection-source-missing = The exact ProjectWorkspace projection did not contain the source text for this indexed file; Audit did not fall back to disk.
source-graph-tera-syntax-invalid = The template does not follow the Tera grammar used by Zola: { $details }
source-graph-partial-extends-invalid = Partials must not use extends. Create a page or layout template for Tera inheritance.
source-graph-partial-block-invalid = Partial { $name } contains Tera block “{ $block }”. Partials must be included fragments without block/endblock.
source-graph-multiple-extends = The template has multiple extends directives; Zola/Tera expects exactly one.
source-graph-duplicate-tera-block = Duplicate Tera block in the same template: { $block }
source-graph-dynamic-load-data =
    { $count ->
        [one] One load_data call in { $file } uses a dynamic path and cannot be resolved statically.
       *[other] { $count } load_data calls in { $file } use dynamic paths and cannot be resolved statically.
    }
preview-projection-unsupported-intent = The preview message type “{ $type }” is not supported.
preview-projection-project-session-required = Open a project session before changing the preview.
preview-projection-required-field-missing = The preview action is missing the required field “{ $field }”.
preview-projection-position-invalid = The preview drop position must be before, after, or inside.
preview-projection-wrong-executor-intent = The preview action reached the wrong executor ({ $executor }).
preview-projection-structural-plan-blocked = The requested structural change is not safe for this source.
preview-projection-structural-plan-blocked-with-details = The requested structural change was refused: { $details }
preview-projection-intent-accepted = The preview action is ready to run.
preview-projection-intent-blocked = The preview action cannot run.
preview-projection-intent-unsupported = The preview action is not supported.
preview-projection-execution-blocked = The preview change could not be applied.
preview-projection-execution-committed = The preview change was applied in { $file }.
recovery-project-workspace-save-incomplete = ProjectWorkspace save transaction { $transaction } is incomplete and requires recovery.
recovery-project-transition-retention-incomplete = Project transition retention { $retention } is incomplete and requires recovery.
recovery-project-workspace-journal-unreadable = The ProjectWorkspace recovery journal could not be read safely.
recovery-project-transition-journal-unreadable = The project transition recovery journal could not be read safely.
recovery-journal-unreadable = A recovery journal could not be read safely.
project-transition-confirmation-required = This project transition requires explicit confirmation.
project-transition-blocked = This project transition is blocked by the authoritative project state.
project-transition-allowed = This project transition is allowed.
file-buffer-diagnostic-not-text = This file is not relevant text for FileBufferStore.
file-buffer-diagnostic-open-failed = { $path } disappeared before FileBufferStore could load it.
file-buffer-diagnostic-not-file = { $path } is no longer a regular file.
file-buffer-diagnostic-file-too-large = { $path } exceeds the safe per-file FileBufferStore limit.
file-buffer-diagnostic-invalid-path = { $path } is not a valid project-relative path.
file-buffer-diagnostic-unsafe-path = { $path } cannot be followed safely inside the project.
file-buffer-diagnostic-unstable = { $path } changed while Rust was reading it.
file-buffer-diagnostic-read-failed = { $path } could not be read as text.
file-buffer-diagnostic-max-files = FileBufferStore reached its safe file-count limit.
file-buffer-diagnostic-max-total-bytes = FileBufferStore reached its safe total-memory limit.
file-buffer-diagnostic-saved-file-too-large = The saved file { $path } exceeds the safe FileBufferStore limit and was removed from the in-memory text index.
file-buffer-diagnostic-generic = FileBufferStore reported a workspace diagnostic.
