taxonomies-eyebrow = Catalog semantic Rust
taxonomies-title = Taxonomii
taxonomies-description = Definițiile, termenii, rutele și impactul sunt proiectate după semantica Zola.
taxonomies-stat-definitions = Definiții
taxonomies-stat-terms = Termeni
taxonomies-stat-pages = Pagini
taxonomies-stat-problems = Probleme
taxonomies-root-url = Rădăcină URL
taxonomies-root-placeholder = implicit Zola
taxonomies-apply = Aplică
taxonomies-search-label = Caută taxonomii sau termeni
taxonomies-search-placeholder = Caută taxonomie sau termen
taxonomies-add = Adaugă taxonomie
taxonomies-catalog-label = Catalog taxonomii
taxonomies-load-error-title = Catalogul nu poate fi proiectat
taxonomies-retry = Reîncearcă
taxonomies-loading = Se proiectează catalogul Rust…
taxonomies-empty-title = Proiectul nu declară nicio taxonomie
taxonomies-empty-description = Taxonomiile grupează paginile după termeni și generează rute Zola pentru liste și termeni.
taxonomies-add-first = Adaugă prima taxonomie
taxonomies-terms-count =
    { $count ->
        [one] { $count } termen
       *[other] { $count } termeni
    }
taxonomies-declared = Declarată
taxonomies-undeclared = Nedeclarată
taxonomies-terms-label = Termeni { $name }
taxonomies-no-results = Niciun rezultat
taxonomies-change-search = Schimbă termenul de căutare.
taxonomies-detail-label = Panou contextual taxonomii
taxonomies-new-definition = Definiție Zola nouă
taxonomies-semantic-edit = Editare semantică
taxonomies-add-title = Adaugă taxonomie
taxonomies-edit-title = Editează { $name }
taxonomies-form-description = Rust validează definiția și actualizează atomic configurația și atribuirile afectate.
taxonomies-cancel = Renunță
taxonomies-name = Nume
taxonomies-language = Limbă
taxonomies-render-pages = Generează pagini
taxonomies-render-feed = Generează feed
taxonomies-items-per-page = Elemente / pagină
taxonomies-no-pagination = fără paginare
taxonomies-pagination-path = Cale paginare
taxonomies-required-error = Numele și limba taxonomiei sunt obligatorii.
taxonomies-applying = Se aplică prin Rust…
taxonomies-create-session = Creează în sesiune
taxonomies-apply-changes = Aplică modificările
taxonomies-term-kicker = Termen · { $taxonomy } · { $language }
taxonomies-close-term = Închide termenul
taxonomies-rename-label = Redenumește în toate paginile afectate
taxonomies-rename-atomic = Redenumește atomic
taxonomies-zola-slug = Slug Zola
taxonomies-pages = Pagini
taxonomies-route = Rută
taxonomies-same-slug = Variante cu același slug
taxonomies-associated-pages = Pagini asociate
taxonomies-no-associated-pages = Nicio pagină asociată.
taxonomies-definition = Definiție Zola
taxonomies-undeclared-use = Utilizare nedeclarată
taxonomies-edit = Editează
taxonomies-declare = Declară
taxonomies-slug = Slug
taxonomies-terms = Termeni
taxonomies-affected-pages = Pagini afectate
taxonomies-rendering = Randare
taxonomies-active-feminine = Activă
taxonomies-disabled-feminine = Dezactivată
taxonomies-feed = Feed
taxonomies-active-masculine = Activ
taxonomies-disabled-masculine = Dezactivat
taxonomies-pagination = Paginare
taxonomies-effective-templates = Șabloane efective
taxonomies-list-template = Listă taxonomie
taxonomies-term-template = Pagină termen
taxonomies-open-templates = Deschide în Șabloane
taxonomies-no-pages-use = Nicio pagină nu folosește această taxonomie.
taxonomies-rust-diagnostics = Diagnostice Rust
taxonomies-edit-definition = Editează definiția
taxonomies-declare-taxonomy = Declară taxonomia
taxonomies-remove = Elimină
taxonomies-delete-label = Confirmare eliminare taxonomie
taxonomies-delete-title = Elimini „{ $name }”?
taxonomies-delete-impact =
    Definiția afectează { $pageCount ->
        [one] { $pageCount } pagină
       *[other] { $pageCount } pagini
    } și { $termCount ->
        [one] { $termCount } termen
       *[other] { $termCount } termeni
    }.
taxonomies-remove-assignments = Elimină și atribuirile din toate frontmatter-ele afectate
taxonomies-diagnostic-config-invalid = Configurația Zola nu poate proiecta taxonomiile: { $details }
taxonomies-diagnostic-definition-slug-collision = Taxonomiile { $names } pentru limba { $language } generează aceeași rută „{ $slug }”.
taxonomies-diagnostic-undeclared = Pagina { $path } folosește taxonomia nedeclarată „{ $name }” pentru limba { $language }.
taxonomies-diagnostic-term-slug-collision = Termenii { $aliases } sunt reuniți de Zola la ruta „{ $slug }”.
taxonomies-diagnostic-template-missing =
    Taxonomia „{ $name }” este randată, dar nu are șablon efectiv pentru { $kind ->
        [list] listă
       *[term] termen
    }.
taxonomies-confirm-impact = Confirmă impactul
taxonomies-select-title = Selectează o taxonomie
taxonomies-select-description = Inspectorul va afișa rutele, șabloanele și paginile afectate.
taxonomies-template-missing = Lipsește
taxonomies-template-theme = Temă
taxonomies-template-theme-named = Temă · { $name }
taxonomies-template-local = Local
taxonomies-template-fallback = fallback
taxonomies-operation-label = Operația pe taxonomii
taxonomies-operation-session-warning = { $message } Modificarea este în ProjectWorkspace; interfața necesită resincronizare.
taxonomies-operation-session-success = { $message } — modificarea este în ProjectWorkspace; Ctrl+S persistă pe disc.
taxonomies-operation-failed = Operația pe taxonomii a eșuat: { $message }
taxonomies-created = Taxonomia a fost creată
taxonomies-updated = Taxonomia a fost actualizată
taxonomies-root-updated = Rădăcina rutelor taxonomice a fost actualizată
taxonomies-term-renamed = Termenul „{ $name }” a fost redenumit
taxonomies-removed = Taxonomia „{ $name }” a fost eliminată
