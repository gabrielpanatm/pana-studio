# Audit și implementare — latența mutațiilor Preview

Data: 2026-07-31

## Verdict

Latența percepută nu era o limită inevitabilă a Rust sau a DOM-ului. Cauza
principală era arhitecturală: confirmarea vizuală a unei mutații era cuplată
prea strâns de clonarea ProjectWorkspace, persistența recovery, reconstruirea
generației Preview și verificarea canonică Zola.

Randarea Zola completă rămâne un cost real pentru schimbări globale, dar nu
trebuie să blocheze feedback-ul vizual. Fluxul implementat păstrează Rust ca
unică autoritate și separă explicit planificarea gestului de mutarea vizibilă:

1. DragOver: plan Rust tipat + indicator, fără mutarea DOM-ului;
2. Drop: proiecție DOM instantă, reversibilă, din ultimul plan Rust permis;
3. commit Rust durabil + CanvasPatch de confirmare;
4. verificare Zola canonică asincronă, cu fallback sau rollback controlat.

Nu a fost introdusă o autoritate paralelă. Ordonarea nouă folosește identitatea
sesiunii, revision, generation, transaction ID și sequence.

```text
DragOver
        │
        ▼
plan semantic Rust tipat ──► indicator vizual; DOM nemodificat
        │
        ▼ Drop
proiecție DOM reversibilă (≤50 ms de la pointer-up)
        │
        ▼
commit ProjectWorkspace + recovery
        │
        ▼
CanvasPatch exact ──► confirmare fără a doua mutare vizibilă
        │
        └───────────► Zola canonic în fundal
                         │
                         ├─ succes: confirmare canonicalVerified
                         └─ eșec: rollback/fallback diagnosticat
```

## Cauzele identificate

- DragOver folosea mai multe rezolvări seriale și putea procesa poziții deja
  depășite.
- După prima optimizare, proiecția DOM provizorie era declanșată încă din
  DragOver. Latența era mică, dar semantica gestului era greșită: elementul se
  deplasa înainte ca utilizatorul să dea drumul butonului mouse-ului.
- Undo/Redo știa starea sursei, dar nu păstra delta vizuală semantică
  forward/inverse.
- Recovery serializa frecvent întregul workspace, inclusiv istoricul.
- O nouă generație Preview rematerializa surse și artefacte neschimbate.
- Reconcilierea frontend derivată putea întârzia pregătirea Preview, deși
  ambele consumau aceeași revizie Rust imuabilă.
- Ancorele bazate numai pe source ID erau insuficiente pentru aceeași sursă
  randată de mai multe ori.

## Corecții implementate

### Drag și DragOver

- O singură comandă Rust, `resolve_canvas_drag_over_intent`, rezolvă ținta,
  permisiunea și planul din același snapshot și ProjectModel.
- Lane-ul frontend este latest-wins: maximum o operație activă și numai ultimul
  DragOver pending. Sequence, document epoch, agent instance și generation sunt
  reverificate după fiecare `await`.
- `EditorMovePlan` schema 3 conține `liveProjection` schema 1, emisă exclusiv
  de Rust din execuția efectivă. Contractul include identitatea Canvas, tokenul
  planului, instanțele render source/target, poziția și ancorele rollback.
- Numele de schemă `liveProjection` descrie proiecția rapidă față de commitul
  canonic, nu o mutare în timpul DragOver. Frontend-ul păstrează planul și
  afișează numai indicatorul până la Drop.
- Proiecția este disponibilă pentru orice `EditorMoveExecution::Html` sigur:
  `HtmlSourceMove`, `ComponentMove` și `BlockMove` cu origine HTML. Tera,
  proveniența ambiguă, identitățile lipsă și sursele randate multiplu rămân
  fail-closed, cu motiv tipat.
- La Drop, iframe-ul repoziționează provizoriu nodul HTML exact identificat.
  Păstrează părintele, următorul sibling real și `pointer-events`, iar Cancel
  restaurează exact DOM-ul anterior.
- Registry-ul Rust păstrează împreună planul și execuția care l-a produs.
  Commitul consumă aceeași decizie tokenizată când revizia este neschimbată;
  nu reconstruiește semantic planul în frontend.
- Drop așteaptă lane-ul DragOver latest-wins, folosește ultima țintă permisă de
  Rust și abia apoi cere proiecția DOM. Bridge-ul acceptă proiecția numai dacă
  `dragSessionId` și `gestureSequence` corespund exact unui Drop real pending;
  orice comandă primită înainte de Drop este refuzată fail-closed.

### Drop și CanvasPatch

- Patch-ul este legat de project root, runtime session, base revision, result
  revision, workspace transaction ID și model revisions.
- Ancorele includ `renderInstanceId`, plus identități alternative controlate
  de Rust.
- Executorul browser validează unicitatea ancorei și înregistrează operațiile
  inverse înainte să modifice DOM-ul.
- Aplicarea, refuzul, fallback-ul și rollback-ul emit evenimente de
  observabilitate cu diagnostic.

### Undo/Redo

- Fiecare intrare eligibilă din istoricul Rust poate păstra o deltă Canvas
  semantică forward și inverse.
- Sunt acoperite move, insert, delete, duplicate, setText, setAttributes și
  replaceTag.
- Undo primește patch-ul invers, Redo patch-ul direct; ambele folosesc aceeași
  validare de identitate ca Drop.
- Intrările coalesced păstrează primul inverse și ultimul forward.
- Tera, macro-urile, frontmatter-ul și ancorele ambigue nu sunt ghicite:
  folosesc reproiecția canonică sigură.

### Costul autorității și al Preview-ului canonic

- Commitul ProjectWorkspace măsoară separat clone, mutation, recovery,
  authority publish și total.
- Recovery este acum checkpoint + jurnal incremental checksummed. Checkpoint-ul
  se compactează periodic la 32 de revizii și există limite stricte pentru
  record și jurnal.
- Istoricul recovery serializează numai prefixul/sufixul schimbat, inclusiv
  cazul de trim/coalescing.
- Generația sursă este seed-uită din generația publicată: FICLONE copy-on-write
  când filesystem-ul îl suportă, apoi `copy_file_range`, apoi copiere
  descriptor-bound ca fallback sigur.
- O schimbare numai de template reutilizează prin referință rădăcina imuabilă
  de artefacte publicată anterior; nu mai copiază fiecare CSS/static.
- Impactul canonic este clasificat `Full`, `Templates` sau `AssetsOnly`.
  Schimbările globale refac site-ul, template-urile folosesc reload-ul Zola,
  iar resursele refac numai artefactele necesare.
- Reconcilierea topology/SourceGraph/SCSS și candidatul Zola pornesc în paralel
  din aceeași revizie Rust. Un test cu barieră produce deadlock dacă fluxul
  redevine serial.

Ruta activă apare prima prin CanvasPatch. Verificarea completă a rutelor
dependente rămâne în fundal; API-ul embedded Zola actual nu oferă o publicație
canonică independentă și sigură pentru o singură rută după orice tip de
schimbare globală.

## Măsurători

Toate valorile sunt din build de dezvoltare și sunt potrivite pentru comparații
relative. Testul Firefox măsoară runtime-ul iframe real; nu include latența
unui pachet release Tauri pe un proiect de producție.

### Runtime browser real, warm

| Metrică | Țintă | Rezultat |
| --- | ---: | ---: |
| Drop→DOM round-trip | ≤50 ms | 18 ms |
| CanvasPatch p95, 100 mostre | ≤50 ms | 2 ms |
| Bridge p95 | — | 1 ms |
| Undo/Redo patch maxim | ≤100 ms | 1 ms |
| Undo/Redo bridge maxim | ≤100 ms | 1 ms |
| Proiecții stale acceptate | 0 | 0 |
| Document iframe păstrat | da | da |

Testul încearcă deliberat o proiecție înainte de Drop și confirmă că DOM-ul și
`pointer-events` rămân neschimbate. După Drop, aceeași proiecție tipată este
acceptată și aplicată în 18 ms. DragOver Rust expune separat
`inputToPlanDurationMs` și `rustDurationMs`; distribuția p95 completă IPC pe
aplicația împachetată trebuie măsurată separat.

### WebKit/Tauri real — măsurătoare istorică a proiecției

Testul a folosit o copie temporară a `studio.pana.tm.ro/sursa`, deschisă prin
UI-ul Tauri. Fișierul real și copia au păstrat același SHA-256 pentru
`templates/index.html`: `73b7e9915a914fecfe9b7eba43b6a9c52968d22cdf9ef80275f1bd81dbada7e5`.

| Metrică | Înainte | După |
| --- | ---: | ---: |
| Prima proiecție DOM din DragOver | 1 457 ms | 14 ms |
| Proiecții live, mostre | — | 17 |
| Proiecție live p50 | — | 16 ms |
| Proiecție live p95 / maxim | — | 24 / 24 ms |
| Buget cerut | ≤50 ms | trecut |
| `fontFallbackFrames` | — | 0 în 7/7 promovări |
| `fontInvalidationCount` / `maxTextMetricDelta` | — | 0 / 0 |

Aceste valori au fost capturate înainte de corectarea semanticii gestului și
demonstrează numai că proiecția DOM încape în bugetul de 50 ms. Ele nu sunt
acceptance pentru fluxul actual, deoarece proiecția pornea din DragOver.
Contractul actual mută exact aceeași operație după Drop; validarea automată în
browser măsoară 18 ms. O recaptură WebKit/Tauri va raporta separat Drop→DOM.

Măsurătoarea istorică, dinaintea builder-ului incremental, nu participa la
timpul până la feedback, dar făcea commitul autoritativ lent. Cele trei
commituri `ComponentMove` măsurate atunci au avut
`pointerUpToCommitReceiptMs` 1 370–1 421 ms. Descompunerea arată clar limita
de atunci:

| Sub-timp commit | Interval măsurat |
| --- | ---: |
| `planRevalidationMs` | 0 ms |
| `nativeBlockContractMs` | 4–5 ms |
| `workspaceStageMs` | 2 ms |
| `afterProjectModelBuildMs` | 1 131–1 167 ms |
| `aliasCalculationMs` | 45–51 ms |
| `recoveryPersistMs` | 106–171 ms |

Auditul ulterior în cod a confirmat că relațiile tranzitive necesare există în
`SourceGraph`. A fost implementat un fast-path Rust fail-closed pentru un
singur template HTML local. Pe copia reală, în release warm, builder-ul complet
a avut 74 ms p95, iar builder-ul incremental 37 ms p95 în 25 de mostre, cu
zero fallback și egalitate exactă față de oracle. Detaliile contractului,
fallback-urilor și matricei de teste sunt în
[project-model-incremental-rebuild-2026-07-31.md](project-model-incremental-rebuild-2026-07-31.md).

### Recovery incremental

| Fixture | Snapshot complet | Jurnal delta | Timp snapshot | Timp jurnal |
| --- | ---: | ---: | ---: | ---: |
| mic | 41.677 B | 9.320 B | 4,153 ms | 0,994 ms |
| mare, 96 × 32 KiB | 3.227.334 B | 66.666 B | 281,817 ms | 5,985 ms |

Pe fixture-ul mare, delta este de aproximativ 48× mai mică și pregătirea ei de
aproximativ 47× mai rapidă decât serializarea conservatoare a checkpoint-ului.

### Reutilizarea generației Preview

Pentru a doua revizie, cu un singur template schimbat:

- intrări materializate explicit: 9 → 1;
- intrări reutilizate: 8;
- artefacte CSS/static: reutilizate prin referință;
- fișiere FICLONE: 0 pe filesystem-ul fixture-ului;
- fallback kernel/userspace: 4 fișiere;
- candidatul canonic de test: 78 ms total.

Absența FICLONE este o proprietate a filesystem-ului testat, nu o eroare.
Corectitudinea rămâne aceeași, dar un object store content-addressed ar elimina
și copierea fallback.

## Verificare

- Rust: 1.255 teste trecute, 0 eșuate, 3 ignorate intenționat;
- kernel frontend: 66/66 suite trecute;
- Svelte/TypeScript: 0 erori, 0 avertismente;
- test integrare Preview cu server loopback: trecut;
- test browser real: trecut, inclusiv refuzul mutării înainte de Drop,
  DOM neschimbat la pointer-up, proiecție Drop→DOM în 18 ms, restore exact,
  rollback, insert/Undo/Redo și fazele canonice;
- WebKit/Tauri real, măsurătoare istorică: 17 proiecții în 11–24 ms, commit Rust
  autoritativ, Undo/Redo CanvasPatch aplicat și zero variație metrică a
  fontului; recaptura semanticii post-Drop rămâne de făcut;
- `cargo check`, `cargo fmt --check` și `git diff --check`: trecute.

## Limite reale și pașii următori

1. **Măsurare release pe proiect mare.** Capturarea p50/p95/p99 pentru
   input→plan, plan→commit, recovery, commit→patch și patch→canonical trebuie
   rulată într-un AppImage, pe fixture-ul real cu 35+ rute.
2. **Object store content-addressed.** Sursele neschimbate pot fi păstrate ca
   obiecte immutable după hash și legate în generație, eliminând cei patru
   fallback copies observați când reflink nu există.
3. **Randare Zola per rută.** `ProjectModel` folosește acum relațiile existente
   pentru invalidarea semantică și rebuild-ul incremental al grafului. Pentru
   verificare Zola realmente parțială rămâne necesară o extensie a motorului
   embedded; config, taxonomiile și dependențele dinamice continuă corect să
   folosească rebuild-ul canonic complet.
4. **Patch fallback pentru destinații patologice.** O mutare deliberată către
   o țintă descendentă a produs la Undo un `CanvasPatchRefused` ciclic și a
   revenit corect la reproiecția canonică. Nu există corupție, dar acel caz nu
   este instant; planificatorul Rust poate bloca mai devreme această topologie.
5. **Bugete CI de performanță.** Adăugarea fixture-urilor 1k/10k pagini și a
   pragurilor warm p95 ar detecta regresii de coadă, jurnal și materializare,
   nu doar regresii funcționale.

Concluzia operațională: elementul nu se mai mișcă în timpul drag-ului; numai
indicatorul urmărește poziția planificată de Rust. La Drop, feedback-ul vizual
instant este realizabil și decuplat de costul Zola. Limita rămasă aparține
verificării canonice site-wide, nu mutației vizuale și nu autorității Rust.

Remedierea ulterioară a flash-ului de HTML fără CSS este documentată separat
în [preview-fouc-remediation-2026-07-31.md](preview-fouc-remediation-2026-07-31.md).
