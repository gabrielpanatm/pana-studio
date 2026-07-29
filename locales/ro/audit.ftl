audit-view-project = Audit proiect
audit-view-runtime = Execuție
audit-category-build = Construire
audit-category-references = Referințe
audit-category-accessibility = Accesibilitate
audit-category-seo = SEO
audit-category-assets = Resurse
audit-category-workspace = Spațiu de lucru
audit-zola-valid = Validare Zola reușită
audit-zola-invalid = Proiect Zola invalid
audit-zola-unavailable = Validare indisponibilă
audit-zola-queued = Validare programată
audit-zola-running = Validare în curs
audit-zola-none = Zola nevalidat
audit-full-failed = Auditul complet a eșuat: { $error }
audit-project-location = Proiect
audit-eyebrow = Spațiu pentru calitate
audit-title = Audit proiect
audit-description = Problemele structurale sunt derivate din sesiunea Rust curentă; validarea Zola confirmă separat construirea reală.
audit-refresh = Reanalizează
audit-run-full = Rulează audit complet
audit-tabs-label = Vizualizări Audit
audit-summary-label = Rezumat audit
audit-errors-count =
    { $count ->
        [one] { $count } eroare
        [few] { $count } erori
       *[other] { $count } de erori
    }
audit-warnings-count =
    { $count ->
        [one] { $count } avertisment
        [few] { $count } avertismente
       *[other] { $count } de avertismente
    }
audit-info-count =
    { $count ->
        [one] { $count } diagnostic informativ
        [few] { $count } diagnostice informative
       *[other] { $count } de diagnostice informative
    }
audit-files-count =
    { $count ->
        [one] { $count } fișier afectat
        [few] { $count } fișiere afectate
       *[other] { $count } de fișiere afectate
    }
audit-errors = Erori
audit-warnings = Avertismente
audit-informational = Informative
audit-affected-files = Fișiere afectate
audit-build = Construire
audit-build-label = Construire: { $status }. { $message }
audit-diagnostics = Diagnostice
audit-visible-count = { $visible } din { $total }
audit-search-label = Caută în diagnostice
audit-search-placeholder = Caută mesaj, cod sau fișier
audit-severity = Severitate
audit-all = Toate
audit-category = Categorie
audit-rust-failed = Auditul Rust nu a putut fi construit
audit-retry = Reîncearcă
audit-building = Se construiește proiecția auditului din sesiunea proiectului…
audit-no-filter-results = Niciun rezultat pentru filtrele curente
audit-reset-filters = Resetează filtrele
audit-no-known-problems = Nu există probleme structurale cunoscute
audit-run-full-help = Rulează auditul complet pentru a confirma și build-ul Zola.
audit-severity-label = Severitate: { $severity }
audit-open = Deschide
audit-title-project-model = Modelul proiectului
audit-title-project-reference = Referință de proiect
audit-title-workspace-file = Fișier omis din workspace
audit-image-missing-alt-title = Imagine fără text alternativ
audit-image-missing-alt-message = Elementul <img> nu declară atributul alt. Folosește alt gol numai pentru imagini decorative.
audit-html-missing-lang-title = Limba documentului lipsește
audit-html-missing-lang-message = Elementul <html> trebuie să declare lang pentru cititoare de ecran și motoare de căutare.
audit-document-missing-title-title = Titlul documentului lipsește
audit-document-missing-title-message = Template-ul conține <head>, dar nu declară un element <title>.
audit-content-missing-title-title = Pagina nu are title
audit-content-missing-title-message = Frontmatter-ul paginii nu declară un titlu explicit.
audit-content-missing-description-title = Meta description lipsește
audit-content-missing-description-message = Adaugă description în frontmatter pentru un rezumat controlat în rezultatele de căutare.
audit-asset-without-usage-title = Resursă fără utilizare cunoscută
audit-asset-without-usage-message = Source Graph nu a găsit nicio referință către { $path }.
