diagnostic-application-settings-stale = Setările aplicației așteptau revizia { $expected }, dar Rust deține revizia { $actual }.
diagnostic-application-settings-load-failed = Setările aplicației nu au putut fi încărcate.
diagnostic-application-settings-save-failed = Setările aplicației nu au putut fi salvate.
diagnostic-application-settings-invalid-language = Limba interfeței „{ $locale }” nu este inclusă în această versiune Pană Studio.
diagnostic-application-settings-invalid-accent = Un accent personalizat trebuie să fie o culoare sRGB în format #RRGGBB.
diagnostic-application-settings-system-refresh-failed = Preferințele sistemului nu au putut fi reaplicate.
diagnostic-application-settings-layout-save-failed = Aspectul Inspectorului nu a putut fi salvat.
diagnostic-system-preferences-live-unavailable = Preferințele sistemului nu se vor actualiza live.
diagnostic-disk-conflict-file-missing = Fișierul urmărit lipsește de pe disc față de referința sesiunii.
diagnostic-disk-conflict-file-not-file = Calea urmărită nu mai este un fișier text.
diagnostic-disk-conflict-file-oversized = Fișierul de pe disc are { $size } octeți, peste limita FileBufferStore de { $limit } octeți.
diagnostic-disk-conflict-file-invalid-path = Calea urmărită nu poate fi citită în limita proiectului: { $detail }
diagnostic-disk-conflict-file-unreadable = Fișierul de pe disc nu poate fi citit ca text pentru verificarea conflictelor: { $detail }
diagnostic-disk-conflict-file-readonly = Fișierul de pe disc este doar în citire; Save Engine ar bloca scrierea.
diagnostic-disk-conflict-file-changed = Conținutul de pe disc diferă de referința FileBufferStore.
diagnostic-disk-conflict-file-metadata = Metadatele discului diferă, dar hash-ul text corespunde referinței.
diagnostic-disk-conflict-file-draft = Există o ciornă în memorie, iar discul este încă la referința sesiunii.
diagnostic-disk-conflict-file-clean = Discul corespunde referinței FileBufferStore.
diagnostic-disk-conflict-summary-empty = FileBufferStore nu urmărește încă fișiere pentru verificarea conflictelor.
diagnostic-disk-conflict-summary-error =
    { $count ->
        [one] { $count } fișier nu poate fi verificat sigur față de disc.
        [few] { $count } fișiere nu pot fi verificate sigur față de disc.
       *[other] { $count } de fișiere nu pot fi verificate sigur față de disc.
    }
diagnostic-disk-conflict-summary-warning =
    { $count ->
        [one] { $count } fișier diferă de referință sau ar bloca Save Engine.
        [few] { $count } fișiere diferă de referință sau ar bloca Save Engine.
       *[other] { $count } de fișiere diferă de referință sau ar bloca Save Engine.
    }
diagnostic-disk-conflict-summary-info = { $drafts } ciorne locale și { $metadata } schimbări de metadate fără conflict de hash.
diagnostic-disk-conflict-summary-clean =
    { $count ->
        [one] { $count } fișier urmărit este aliniat cu discul.
        [few] { $count } fișiere urmărite sunt aliniate cu discul.
       *[other] { $count } de fișiere urmărite sunt aliniate cu discul.
    }
source-graph-not-zola-project = Proiectul curent nu pare să fie un proiect Zola valid.
source-graph-conventional-data-invalid = Source Graph nu a putut cataloga un fișier de date convențional: { $details }
source-graph-load-data-missing = Fișierul local Zola referențiat de load_data nu a fost găsit: { $path }
source-graph-load-data-unresolved = load_data(path={ $path }) nu poate fi catalogat: { $details }
source-graph-data-toml-invalid = Document de date TOML invalid: { $details }
source-graph-data-format-invalid = Document de date { $format } invalid: { $details }
source-graph-config-toml-invalid = Configurație TOML invalidă: { $details }
source-graph-content-target-missing = Conținutul Zola referențiat nu a fost găsit: { $target }
source-graph-template-target-missing = Template-ul referențiat nu a fost găsit: { $target }
source-graph-content-tera-syntax-invalid = Conținutul Markdown conține sintaxă Tera 2 invalidă: { $details }
source-graph-legacy-tera-incompatible = Acest template folosește sintaxa macro/import din Tera 1, incompatibilă cu Zola 0.23.4 și Tera 2.
source-graph-legacy-shortcode-template-incompatible = Directoarele legacy de shortcode-uri sunt incompatibile cu Zola 0.23.4; definește o componentă Tera 2.
source-graph-legacy-shortcode-incompatible = Apelul legacy „{ $name }” este incompatibil cu Zola 0.23.4; folosește un apel de componentă Tera 2.
source-graph-zola-runtime-argument-deprecated = Argumentul „{ $argument }” al funcției { $function } este depreciat; folosește „{ $replacement }”.
source-graph-page-template-missing = Template-ul paginii nu a fost găsit: { $template }
source-graph-section-page-template-missing = Template-ul page_template al secțiunii nu a fost găsit: { $template }
source-graph-frontmatter-invalid = Frontmatter { $format } invalid: { $details }
source-graph-projection-source-missing = Proiecția exactă ProjectWorkspace nu conține textul-sursă pentru acest fișier indexat; Audit nu a folosit discul ca alternativă.
source-graph-tera-syntax-invalid = Template-ul nu respectă gramatica Tera folosită de Zola: { $details }
source-graph-partial-extends-invalid = Parțialele nu trebuie să folosească extends. Creează un template de pagină sau layout pentru moștenire Tera.
source-graph-partial-block-invalid = Parțialul { $name } conține blocul Tera „{ $block }”. Parțialele trebuie să fie fragmente incluse, fără block/endblock.
source-graph-multiple-extends = Template-ul are mai multe directive extends; Zola/Tera așteaptă una singură.
source-graph-duplicate-tera-block = Bloc Tera duplicat în același template: { $block }
source-graph-dynamic-load-data =
    { $count ->
        [one] Un apel load_data din { $file } folosește o cale dinamică și nu poate fi rezolvat static.
        [few] { $count } apeluri load_data din { $file } folosesc căi dinamice și nu pot fi rezolvate static.
       *[other] { $count } de apeluri load_data din { $file } folosesc căi dinamice și nu pot fi rezolvate static.
    }
preview-projection-unsupported-intent = Tipul de mesaj preview „{ $type }” nu este acceptat.
preview-projection-project-session-required = Deschide o sesiune de proiect înainte de a modifica preview-ul.
preview-projection-required-field-missing = Acțiunii din preview îi lipsește câmpul obligatoriu „{ $field }”.
preview-projection-position-invalid = Poziția din preview trebuie să fie înainte, după sau în interior.
preview-projection-wrong-executor-intent = Acțiunea din preview a ajuns la executorul greșit ({ $executor }).
preview-projection-structural-plan-blocked = Modificarea structurală cerută nu este sigură pentru această sursă.
preview-projection-structural-plan-blocked-with-details = Modificarea structurală cerută a fost refuzată: { $details }
preview-projection-intent-accepted = Acțiunea din preview este pregătită pentru execuție.
preview-projection-intent-blocked = Acțiunea din preview nu poate fi executată.
preview-projection-intent-unsupported = Acțiunea din preview nu este acceptată.
preview-projection-execution-blocked = Modificarea din preview nu a putut fi aplicată.
preview-projection-execution-committed = Modificarea din preview a fost aplicată în { $file }.
recovery-project-workspace-save-incomplete = Tranzacția de salvare ProjectWorkspace { $transaction } este incompletă și necesită recuperare.
recovery-project-transition-retention-incomplete = Retenția tranziției de proiect { $retention } este incompletă și necesită recuperare.
recovery-project-workspace-journal-unreadable = Jurnalul de recuperare ProjectWorkspace nu a putut fi citit în siguranță.
recovery-project-transition-journal-unreadable = Jurnalul de recuperare pentru tranziția proiectului nu a putut fi citit în siguranță.
recovery-journal-unreadable = Un jurnal de recuperare nu a putut fi citit în siguranță.
project-transition-confirmation-required = Această tranziție de proiect necesită confirmare explicită.
project-transition-blocked = Această tranziție este blocată de starea autoritativă a proiectului.
project-transition-allowed = Această tranziție de proiect este permisă.
file-buffer-diagnostic-not-text = Acest fișier nu este text relevant pentru FileBufferStore.
file-buffer-diagnostic-open-failed = { $path } a dispărut înainte ca FileBufferStore să îl poată încărca.
file-buffer-diagnostic-not-file = { $path } nu mai este un fișier obișnuit.
file-buffer-diagnostic-file-too-large = { $path } depășește limita sigură per fișier a FileBufferStore.
file-buffer-diagnostic-invalid-path = { $path } nu este o cale relativă validă în proiect.
file-buffer-diagnostic-unsafe-path = { $path } nu poate fi urmărit în siguranță în interiorul proiectului.
file-buffer-diagnostic-unstable = { $path } s-a modificat în timp ce Rust îl citea.
file-buffer-diagnostic-read-failed = { $path } nu a putut fi citit ca text.
file-buffer-diagnostic-max-files = FileBufferStore a atins limita sigură de fișiere.
file-buffer-diagnostic-max-total-bytes = FileBufferStore a atins limita sigură totală de memorie.
file-buffer-diagnostic-saved-file-too-large = Fișierul salvat { $path } depășește limita sigură FileBufferStore și a fost eliminat din indexul text din memorie.
file-buffer-diagnostic-generic = FileBufferStore a raportat un diagnostic pentru spațiul de lucru.
