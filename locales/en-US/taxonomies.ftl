taxonomies-eyebrow = Rust semantic catalog
taxonomies-title = Taxonomies
taxonomies-description = Definitions, terms, routes, and impact are projected according to Zola semantics.
taxonomies-stat-definitions = Definitions
taxonomies-stat-terms = Terms
taxonomies-stat-pages = Pages
taxonomies-stat-problems = Problems
taxonomies-root-url = URL root
taxonomies-root-placeholder = Zola default
taxonomies-apply = Apply
taxonomies-search-label = Search taxonomies or terms
taxonomies-search-placeholder = Search taxonomy or term
taxonomies-add = Add taxonomy
taxonomies-catalog-label = Taxonomy catalog
taxonomies-load-error-title = The catalog cannot be projected
taxonomies-retry = Retry
taxonomies-loading = Projecting the Rust catalog…
taxonomies-empty-title = The project does not declare any taxonomy
taxonomies-empty-description = Taxonomies group pages by terms and generate Zola list and term routes.
taxonomies-add-first = Add the first taxonomy
taxonomies-terms-count =
    { $count ->
        [one] { $count } term
       *[other] { $count } terms
    }
taxonomies-declared = Declared
taxonomies-undeclared = Undeclared
taxonomies-terms-label = { $name } terms
taxonomies-no-results = No results
taxonomies-change-search = Change the search term.
taxonomies-detail-label = Taxonomy context panel
taxonomies-new-definition = New Zola definition
taxonomies-semantic-edit = Semantic edit
taxonomies-add-title = Add taxonomy
taxonomies-edit-title = Edit { $name }
taxonomies-form-description = Rust validates the definition and atomically updates the configuration and affected assignments.
taxonomies-cancel = Cancel
taxonomies-name = Name
taxonomies-language = Language
taxonomies-render-pages = Generate pages
taxonomies-render-feed = Generate feed
taxonomies-items-per-page = Items / page
taxonomies-no-pagination = no pagination
taxonomies-pagination-path = Pagination path
taxonomies-required-error = Taxonomy name and language are required.
taxonomies-applying = Applying through Rust…
taxonomies-create-session = Create in session
taxonomies-apply-changes = Apply changes
taxonomies-term-kicker = Term · { $taxonomy } · { $language }
taxonomies-close-term = Close term
taxonomies-rename-label = Rename in all affected pages
taxonomies-rename-atomic = Rename atomically
taxonomies-zola-slug = Zola slug
taxonomies-pages = Pages
taxonomies-route = Route
taxonomies-same-slug = Variants with the same slug
taxonomies-associated-pages = Associated pages
taxonomies-no-associated-pages = No associated page.
taxonomies-definition = Zola definition
taxonomies-undeclared-use = Undeclared use
taxonomies-edit = Edit
taxonomies-declare = Declare
taxonomies-slug = Slug
taxonomies-terms = Terms
taxonomies-affected-pages = Affected pages
taxonomies-rendering = Rendering
taxonomies-active-feminine = Active
taxonomies-disabled-feminine = Disabled
taxonomies-feed = Feed
taxonomies-active-masculine = Active
taxonomies-disabled-masculine = Disabled
taxonomies-pagination = Pagination
taxonomies-effective-templates = Effective templates
taxonomies-list-template = Taxonomy list
taxonomies-term-template = Term page
taxonomies-open-templates = Open in Templates
taxonomies-no-pages-use = No page uses this taxonomy.
taxonomies-rust-diagnostics = Rust diagnostics
taxonomies-edit-definition = Edit definition
taxonomies-declare-taxonomy = Declare taxonomy
taxonomies-remove = Remove
taxonomies-delete-label = Confirm taxonomy removal
taxonomies-delete-title = Remove “{ $name }”?
taxonomies-delete-impact =
    The definition affects { $pageCount ->
        [one] { $pageCount } page
       *[other] { $pageCount } pages
    } and { $termCount ->
        [one] { $termCount } term
       *[other] { $termCount } terms
    }.
taxonomies-remove-assignments = Also remove assignments from all affected front matter
taxonomies-diagnostic-config-invalid = Zola configuration cannot project taxonomies: { $details }
taxonomies-diagnostic-definition-slug-collision = Taxonomies { $names } for language { $language } generate the same “{ $slug }” route.
taxonomies-diagnostic-undeclared = Page { $path } uses undeclared taxonomy “{ $name }” for language { $language }.
taxonomies-diagnostic-term-slug-collision = Zola merges terms { $aliases } at the “{ $slug }” route.
taxonomies-diagnostic-template-missing =
    Taxonomy “{ $name }” is rendered, but has no effective template for { $kind ->
        [list] its list
       *[term] its terms
    }.
taxonomies-confirm-impact = Confirm impact
taxonomies-select-title = Select a taxonomy
taxonomies-select-description = The inspector will show its routes, templates, and affected pages.
taxonomies-template-missing = Missing
taxonomies-template-theme = Theme
taxonomies-template-theme-named = Theme · { $name }
taxonomies-template-local = Local
taxonomies-template-fallback = fallback
taxonomies-operation-label = Taxonomy operation
taxonomies-operation-session-warning = { $message } The change is in ProjectWorkspace; the interface needs to resynchronize.
taxonomies-operation-session-success = { $message } — the change is in ProjectWorkspace; Ctrl+S persists it to disk.
taxonomies-operation-failed = The taxonomy operation failed: { $message }
taxonomies-created = Taxonomy created
taxonomies-updated = Taxonomy updated
taxonomies-root-updated = The taxonomy route root was updated
taxonomies-term-renamed = Term “{ $name }” was renamed
taxonomies-removed = Taxonomy “{ $name }” was removed
