data-title = Date
data-add = Adaugă date
data-search = Caută date
data-all = Toate
data-toml = TOML
data-other-formats = Alte formate
data-eyebrow = Date structurate Zola
data-description = Sursele locale folosite de load_data sunt rezolvate în întregul proiect; date/ rămâne convenția implicită.
data-files = Fișiere
data-tables = Tabele
data-lists = Liste
data-values = Valori
data-links = Legături
data-formats-label = Formate de date
data-search-files = Caută fișiere de date
data-files-label = Fișiere de date
data-values-count =
    { $count ->
        [one] O valoare
        [few] { $count } valori
       *[other] { $count } de valori
    }
data-empty-title = Nicio sursă de date
data-empty-description = Adaugă un fișier TOML sau referențiază o sursă locală prin load_data.

data-node-document = Document
data-node-comment = Comentariu
data-node-element = Element
data-node-element-index = Element { $index }
data-node-value = Valoare
data-kind-document = Document TOML
data-kind-table = Tabel
data-kind-inline-table = Tabel inline
data-kind-table-collection = Colecție de tabele
data-kind-row = Rând
data-kind-list = Listă
data-kind-text = Text
data-kind-integer = Număr întreg
data-kind-decimal = Număr zecimal
data-kind-boolean = Boolean
data-kind-datetime = Dată / oră
data-kind-new-row = Rând nou

data-location-date = Convenție date/
data-location-static = Static local
data-location-content = Conținut local
data-location-output = Output generat
data-location-theme = Temă activă
data-location-project-root = Rădăcină proiect
data-origin-theme = Temă: { $theme }
data-origin-active-theme = Temă activă
data-origin-local = Local

data-mutation-label = Modificarea datelor
data-mutation-needs-resync = { $success } Modificarea este în sesiunea proiectului; interfața necesită resincronizare.
data-mutation-session-only = { $success } Modificarea este în sesiunea proiectului — Ctrl+S persistă pe disc.
data-file-path-required = Adaugă o cale de fișier TOML.
data-file-created = Fișierul { $path } a fost creat.
data-node-updated = Nodul { $node } a fost actualizat.
data-node-inserted = Datele au fost adăugate în { $node }.
data-node-deleted = Nodul { $node } a fost șters.

data-new-file = Fișier nou
data-toml-data = Date TOML
data-new-file-description = Fișierul gol este creat în sesiune; apoi îi adaugi structura vizual.
data-close = Închide
data-project-relative-path = Cale relativă în proiect
data-new-file-path-help = date/ este implicit. Un fișier creat în altă locație este catalogat când este referențiat prin load_data.
data-cancel = Renunță
data-validating = Se validează…
data-create-file = Creează fișierul
data-visual-editing = Editare vizuală TOML
data-visual-editing-description = Fiecare salvare validată produce o singură acțiune Undo.
data-close-editor = Închide editorul
data-structure-label = Structura { $file }
data-root = rădăcină
data-loading-exact-value = Se citește valoarea exactă din Rust…
data-key = Cheie
data-type = Tip
data-active-value = Valoare activă
data-value-with-kind = Valoare { $kind }
data-save-node = Salvează nodul
data-comments-code-only = Comentariile sunt păstrate lossless și se modifică numai în editorul de cod.
data-add-to-selection = Adaugă în selecție
data-new-element = Element nou
data-value = Valoare
data-add-action = Adaugă
data-delete-confirmation = Ștergi „{ $node }” și toți copiii săi?
data-checking = Se verifică…
data-delete = Șterge
data-delete-node = Șterge nodul

data-origin-label = Origine: { $origin }
data-visually-editable = Editabil vizual
data-read-only = Read-only
data-open-in-editor = Deschide în Editor
data-load-data-paths = Căi load_data
data-semantic-structure = Structură semantică
data-more-nodes =
    { $count ->
        [one] Încă un nod este disponibil în editare.
        [few] Încă { $count } noduri sunt disponibile în editare.
       *[other] Încă { $count } de noduri sunt disponibile în editare.
    }
data-edit-visually = Editează vizual
data-fix-syntax-before-visual = Corectează sintaxa în editorul de cod înaintea editării vizuale.
data-read-only-reason = Sursa este read-only în activitatea Date.
data-select-or-create = Selectează sau creează un fișier de date.
data-new-file-placeholder = date/meniu.toml
data-new-key-placeholder = cheie_nouă
