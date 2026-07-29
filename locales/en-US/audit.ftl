audit-view-project = Project audit
audit-view-runtime = Runtime
audit-category-build = Build
audit-category-references = References
audit-category-accessibility = Accessibility
audit-category-seo = SEO
audit-category-assets = Assets
audit-category-workspace = Workspace
audit-zola-valid = Zola validation passed
audit-zola-invalid = Invalid Zola project
audit-zola-unavailable = Validation unavailable
audit-zola-queued = Validation scheduled
audit-zola-running = Validation in progress
audit-zola-none = Zola not validated
audit-full-failed = Full audit failed: { $error }
audit-project-location = Project
audit-eyebrow = Quality workspace
audit-title = Project audit
audit-description = Structural problems are derived from the current Rust session; Zola validation separately confirms the real build.
audit-refresh = Reanalyze
audit-run-full = Run full audit
audit-tabs-label = Audit views
audit-summary-label = Audit summary
audit-errors-count =
    { $count ->
        [one] { $count } error
       *[other] { $count } errors
    }
audit-warnings-count =
    { $count ->
        [one] { $count } warning
       *[other] { $count } warnings
    }
audit-info-count =
    { $count ->
        [one] { $count } informational diagnostic
       *[other] { $count } informational diagnostics
    }
audit-files-count =
    { $count ->
        [one] { $count } affected file
       *[other] { $count } affected files
    }
audit-errors = Errors
audit-warnings = Warnings
audit-informational = Informational
audit-affected-files = Affected files
audit-build = Build
audit-build-label = Build: { $status }. { $message }
audit-diagnostics = Diagnostics
audit-visible-count = { $visible } of { $total }
audit-search-label = Search diagnostics
audit-search-placeholder = Search message, code, or file
audit-severity = Severity
audit-all = All
audit-category = Category
audit-rust-failed = The Rust audit could not be built
audit-retry = Retry
audit-building = Building the audit projection from the project session…
audit-no-filter-results = No results for the current filters
audit-reset-filters = Reset filters
audit-no-known-problems = No known structural problems
audit-run-full-help = Run the full audit to also confirm the Zola build.
audit-severity-label = Severity: { $severity }
audit-open = Open
audit-title-project-model = Project model
audit-title-project-reference = Project reference
audit-title-workspace-file = File omitted from workspace
audit-image-missing-alt-title = Image without alternative text
audit-image-missing-alt-message = The <img> element does not declare the alt attribute. Use an empty alt only for decorative images.
audit-html-missing-lang-title = Document language is missing
audit-html-missing-lang-message = The <html> element must declare lang for screen readers and search engines.
audit-document-missing-title-title = Document title is missing
audit-document-missing-title-message = The template contains <head>, but does not declare a <title> element.
audit-content-missing-title-title = Page has no title
audit-content-missing-title-message = The page front matter does not declare an explicit title.
audit-content-missing-description-title = Meta description is missing
audit-content-missing-description-message = Add description to front matter for a controlled summary in search results.
audit-asset-without-usage-title = Asset without known usage
audit-asset-without-usage-message = Source Graph found no reference to { $path }.
