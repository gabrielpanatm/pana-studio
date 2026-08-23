# Reconstruire incrementală `ProjectModel`

Data: 2026-07-31
Actualizat: 2026-08-21

## Rezultat

Mutațiile HTML structurale eligibile asupra unui singur template local nu mai
apelează rebuild-ul complet al `ProjectModel`. Rust primește modelul publicat,
snapshot-ul exact al candidatului și lista exactă de fișiere din receipt-ul
tranzacției, reparsează template-ul schimbat și publică numai un model complet
și verificat.

Pe fixture-ul determinist extins `40 pagini / 20 componente / 200 noduri`, în
build release warm:

| Cale | Mostre | Înainte p95 | După p95 | Diferență |
| --- | ---: | ---: | ---: | ---: |
| editare HTML incrementală | 25 | 40,902 ms | 14,728 ms | −64,0% |
| oracle complet | 5 | 56,301 ms | 28,416 ms | −49,5% |
| deschidere proiect | 20 | 54,526 ms | 30,619 ms | −43,8% |
| external reconcile | 25 | 7,039 ms | 6,871 ms | −2,4% |

Fast-path-ul a avut zero fallback și fiecare rezultat a fost semantic identic
cu snapshot-ul produs de builder-ul complet. CSS, cale neafectată de această
schimbare, a rămas la 0,783 ms mediană față de 0,773 ms înainte; p95-ul său
submilisecundă variază între rulări cu zgomotul scheduler-ului.

| Fază HTML | Înainte p95 | După p95 |
| --- | ---: | ---: |
| parse + reconciliere identități | 35,819 ms | 12,137 ms |
| `ComponentGraph` | 3,371 ms | 0,793 ms |
| `BlockGraph` | 0,085 ms | 0,094 ms |
| content usages | 0,582 ms | 0,283 ms |
| listing items | 0,063 ms | 0 ms, reutilizat |
| dynamic widgets | 0,176 ms | 0,083 ms |
| markdown | 0,001 ms | 0,001 ms |
| node index | 0,263 ms | 0,238 ms |

## Contract Rust

API-ul `rebuild_project_model_after_workspace_change` primește:

- `ProjectModel`-ul publicat anterior și revision-ul workspace din care provine;
- `WorkspaceProjectionSnapshot` pentru candidatul exact;
- `changed_paths` din receipt-ul mutației curente, nu delta cumulată față de
  disk;
- un intent tipat. Numai operațiile HTML structurale declarate de executor sunt
  eligibile.

Înainte de fast-path sunt verificate root-ul canonic, revision-uri adiacente,
transaction ID-ul, un singur path normalizat, existența anterioară și curentă a
sursei și originea locală a template-ului.

Operațiile eligibile inițial sunt `move`, `insert`, `delete`, `setText`,
`setAttributes`, `replaceTag` și duplicate-ul HTML, care folosește aceeași cale
de insert. Operațiile Tera și orice apelant fără intent structural folosesc
builder-ul complet.

## Invalidare și înlocuire

`SourceGraph` existent este contractul de dependențe. Pentru template-ul
schimbat, mecanismul:

1. calculează consumatorii inversi tranzitivi prin `Extends`, `Includes` și
   `Imports`, apoi paginile afectate prin `PageTemplate` și
   `SectionPageTemplate`;
2. parsează numai sursa schimbată cu scannerul canonic;
3. compară vechiul și noul contract semantic: origine, nume, block-uri,
   macro-uri, extends/include/import, lookup-uri content, `load_data`, asset-uri
   și transformări de imagine;
4. validează identitățile nodurilor, contiguitatea segmentului și toate
   endpoint-urile relațiilor existente;
5. înlocuiește numai nodurile și sumarul template-ului, păstrând relațiile și
   restul grafului neschimbate.

După confirmarea contractului stabil, un plan explicit de invalidare aplică:

- upsert pe fișier pentru definițiile și invocările `ComponentGraph`, apoi
  reconciliază determinist shadowing-ul, consumatorii și parametrii;
- upsert pe fișier pentru instanțele `BlockGraph`;
- înlocuirea exclusivă a `template_usages` pentru content models;
- reutilizarea integrală a `ListingItemCatalog`, ale cărui intrări nu s-au
  schimbat;
- upsert local pentru dynamic widgets, urmat numai de reconcilierea globală a
  ID-urilor duplicate;
- înlocuirea proiecțiilor Markdown deja calculate în sumarul template-ului.

Full builders rămân unica implementare pentru scanarea completă, oracle și full
fallback. Un gard arhitectural respinge apelarea lor din hot-path-ul unui
template. Nu există al doilea graf semantic sau cache paralel.

Scannerul folosește acum un index al începuturilor de linie pentru `SourceRange`.
Anterior, fiecare nod recalcula linia și coloana de la începutul sursei, ceea ce
făcea parsarea cvadratică pe template-uri cu multe noduri.

## Fallback fail-closed

Orice invariant neverificabil intră în builder-ul complet și emite un motiv
stabil. Sunt acoperite explicit:

- config, content/taxonomii, style, script, data și surse de temă;
- creare, ștergere și redenumire;
- mai multe fișiere schimbate;
- revision stale, transaction ID absent, root/path nesigur sau model anterior
  absent;
- template de temă ori ambiguu, diagnostic vechi/nou, coliziune de identitate;
- schimbarea contractului de dependențe;
- `load_data(path=variabilă)` și alte dependențe dinamice.

Fallback-ul nu este o eroare și produce modelul canonic complet. Dacă nici
builder-ul complet nu poate construi candidatul, apelul eșuează și candidatul
workspace este abandonat integral.

## Publicare și Undo/Redo

`structural_write` construiește modelul în candidatul detașat. Publicarea
workspace-ului rămâne atomică și verifică project root, runtime session,
workspace revision și transaction ID. Un rezultat stale sau o sursă invalidă
nu poate avansa documentele, istoricul ori revision-ul modelului.

Undo și Redo capturează modelul/revision-ul anterior, aplică tranziția în
același candidat, folosesc path-urile exacte ale intrării de istoric și publică
modelul înainte de commitul tranzacției. Intrările cu `canvas_delta` HTML sunt
eligibile; celelalte folosesc fallback-ul complet.

Ruta vizuală post-Drop nu s-a schimbat: CanvasPatch-ul continuă să mute DOM-ul
imediat, separat de commitul autoritativ.

## Oracle și observabilitate

Testele serializează snapshot-ul incremental și pe cel al builder-ului complet
și compară toate derivatele: `ComponentGraph`, `BlockGraph`, content models,
listing items, dynamic widgets și Markdown. Sunt normalizate exclusiv
identitățile runtime; ID-urile semantice rămân în comparație.

Matricea include operațiile HTML, mutații consecutive, forward/Undo/Redo,
extends/include/import/macro/repeat/native block, inheritance de pagină,
content-field usage, listing item, dynamic widgets inclusiv ID duplicat,
Markdown, content/data/assets/styles/scripts, `load_data` static și dinamic,
override local peste o temă activă, editarea temei, config/taxonomii,
create/delete/rename, revision stale, Undo/Redo și rollback la sursă invalidă.

Evenimentele Rust pentru editor move și Undo/Redo raportează:

- `incremental` sau `fullFallback` și motivul fallback-ului;
- changed paths, template-uri și pagini invalidate;
- noduri înlocuite/reutilizate și relații reutilizate;
- clone, parse, `ComponentGraph`, `BlockGraph`, content usages, listing
  reuse/update, dynamic widgets, Markdown, node index și durata totală, în
  microsecunde.

Benchmark-ul release se rulează prin `npm run performance:baseline`. Runner-ul
generează fixture-ul controlat, setează `PANA_PERFORMANCE_BENCH_PROJECT` și
aplică bugete executabile: HTML/model 20 ms, oracle complet 50 ms, project open
40 ms, CSS 1,5 ms, external reconcile 10 ms și clone 1,5 ms. Raportul final are
`budgetViolations: []`.

## Limită cunoscută

Dacă template-ul își schimbă dependențele, topologia sau diagnosticele,
rebuild-ul complet este intenționat și rămâne în afara fast-path-ului. Calea
incrementală nu este extinsă la content/config/style topology.
