publish-view-release = Pregătire publicare
publish-view-configuration = Configurare
publish-validation-valid = Construire Zola validă
publish-validation-invalid = Construire Zola invalidă
publish-validation-error = Validare indisponibilă
publish-validation-queued = Validare programată
publish-validation-running = Validare în curs
publish-validation-none = Construire nevalidată
publish-preflight-updated = Verificarea înainte de publicare a fost actualizată.
publish-preflight-failed = Verificarea nu a putut fi finalizată: { $error }
publish-eyebrow = Spațiu de publicare
publish-title = Publicare
publish-description = Verificare, construire și publicare într-un singur flux. Sursele sunt salvate înainte ca rezultatul Zola să fie publicat.
publish-state = Stare publicare
publish-ready = Pregătit pentru construire
publish-needs-check = Necesită verificare
publish-tabs-label = Vizualizări Publicare
publish-quality-gates = Praguri de calitate
publish-preflight-title = Verificare înainte de publicare
publish-checking = Se verifică…
publish-run-preflight = Rulează preflight
publish-sources-saved = Surse salvate
publish-sources-synced = Sesiunea proiectului și discul sunt sincronizate.
publish-unsaved-areas =
    { $count ->
        [one] { $count } zonă conține modificări nepersistate.
        [few] { $count } zone conțin modificări nepersistate.
       *[other] { $count } de zone conțin modificări nepersistate.
    }
publish-save = Salvează
publish-project-audit = Audit proiect
publish-audit-summary = { $errors } erori · { $warnings } avertismente
publish-audit-stale = Auditul nu corespunde reviziei curente.
publish-open-audit = Deschide auditul
publish-validation-help = Rulează preflight pentru validarea proiectului cu Zola.
publish-check = Verifică
publish-bunny-target = Țintă Bunny CDN
publish-bunny-description = Credentialele rămân în .env și sunt folosite numai de comanda Rust de deploy.
publish-configure = Configurează
publish-build-and-release = Construire și publicare
publish-release-current = Livrează versiunea curentă
publish-release-description = Construirea generează rezultatul local. Publicarea trimite rezultatul configurat către Bunny CDN și nu pornește automat.
publish-gates-warning = Rezolvă pragurile înainte de publicare. Acțiunile rămân disponibile pentru verificări controlate.
publish-open-log = Deschide jurnalul
publish-config-sources = config.toml · .env · setări Pană
publish-config-title = Configurare build și destinație
publish-config-description = Setările sunt citite și scrise prin comenzile proiectului, fără o configurație paralelă în interfață.
