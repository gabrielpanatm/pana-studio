data-title = Data
data-add = Add data
data-search = Search data
data-all = All
data-toml = TOML
data-other-formats = Other formats
data-eyebrow = Structured Zola data
data-description = Local sources used by load_data are resolved throughout the project; date/ remains the default convention.
data-files = Files
data-tables = Tables
data-lists = Lists
data-values = Values
data-links = Links
data-formats-label = Data formats
data-search-files = Search data files
data-files-label = Data files
data-values-count =
    { $count ->
        [one] One value
       *[other] { $count } values
    }
data-empty-title = No data source
data-empty-description = Add a TOML file or reference a local source with load_data.

data-node-document = Document
data-node-comment = Comment
data-node-element = Element
data-node-element-index = Element { $index }
data-node-value = Value
data-kind-document = TOML document
data-kind-table = Table
data-kind-inline-table = Inline table
data-kind-table-collection = Table collection
data-kind-row = Row
data-kind-list = List
data-kind-text = Text
data-kind-integer = Integer
data-kind-decimal = Decimal number
data-kind-boolean = Boolean
data-kind-datetime = Date / time
data-kind-new-row = New row

data-location-date = date/ convention
data-location-static = Local static
data-location-content = Local content
data-location-output = Generated output
data-location-theme = Active theme
data-location-project-root = Project root
data-origin-theme = Theme: { $theme }
data-origin-active-theme = Active theme
data-origin-local = Local

data-mutation-label = Data change
data-mutation-needs-resync = { $success } The change is in the project session; the interface must be resynchronized.
data-mutation-session-only = { $success } The change is in the project session — Ctrl+S persists it to disk.
data-file-path-required = Add a TOML file path.
data-file-created = The file { $path } was created.
data-node-updated = The node { $node } was updated.
data-node-inserted = Data was added to { $node }.
data-node-deleted = The node { $node } was deleted.

data-new-file = New file
data-toml-data = TOML data
data-new-file-description = The empty file is created in the session; then you can add its structure visually.
data-close = Close
data-project-relative-path = Project-relative path
data-new-file-path-help = date/ is the default. A file created elsewhere is cataloged when load_data references it.
data-cancel = Cancel
data-validating = Validating…
data-create-file = Create file
data-visual-editing = Visual TOML editing
data-visual-editing-description = Every validated save produces one Undo action.
data-close-editor = Close editor
data-structure-label = Structure of { $file }
data-root = root
data-loading-exact-value = Reading the exact value from Rust…
data-key = Key
data-type = Type
data-active-value = Active value
data-value-with-kind = { $kind } value
data-save-node = Save node
data-comments-code-only = Comments are preserved losslessly and can only be changed in the code editor.
data-add-to-selection = Add to selection
data-new-element = New element
data-value = Value
data-add-action = Add
data-delete-confirmation = Delete “{ $node }” and all its children?
data-checking = Checking…
data-delete = Delete
data-delete-node = Delete node

data-origin-label = Origin: { $origin }
data-visually-editable = Visually editable
data-read-only = Read-only
data-open-in-editor = Open in Editor
data-load-data-paths = load_data paths
data-semantic-structure = Semantic structure
data-more-nodes =
    { $count ->
        [one] One more node is available for editing.
       *[other] { $count } more nodes are available for editing.
    }
data-edit-visually = Edit visually
data-fix-syntax-before-visual = Correct the syntax in the code editor before visual editing.
data-read-only-reason = This source is read-only in the Data activity.
data-select-or-create = Select or create a data file.
data-new-file-placeholder = data/menu.toml
data-new-key-placeholder = new_key
