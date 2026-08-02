# Reconstruire incrementală `ProjectModel`

Data: 2026-07-31

## Rezultat

Mutațiile HTML structurale eligibile asupra unui singur template local nu mai
apelează rebuild-ul complet al `ProjectModel`. Rust primește modelul publicat,
snapshot-ul exact al candidatului și lista exactă de fișiere din receipt-ul
tranzacției, reparsează template-ul schimbat și publică numai un model complet
și verificat.

Pe `/home/gabriel/Documente/studio.pana.tm.ro/sursa`, în build release warm:

| Cale | Mostre | p95 |
| --- | ---: | ---: |
| builder complet | 10 | 74 ms |
| builder incremental | 25 | 37 ms |

Fast-path-ul a avut zero fallback și fiecare rezultat a fost identic cu
snapshot-ul produs de builder-ul complet. Ultima mostră incrementală s-a
descompus în 1 ms clonare model, 29 ms parsare template, 1 ms
`ComponentGraph`, 0 ms `BlockGraph` și 1 ms `TeraGraph`.

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

`ComponentGraph`, `BlockGraph` și `TeraGraph` sunt regenerate determinist din
`SourceGraph`-ul deja actualizat. Pe fixture-ul real acest cost cumulat este de
aproximativ 2 ms în release. Alegerea păstrează oracle-ul exact și evită o a
doua autoritate de indexare; fragmentarea acestor derivate nu este necesară
pentru bugetul de 50 ms.

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
și cer egalitate exactă pentru files/revision, `SourceGraph`, `TeraGraph`,
`ComponentGraph`, `BlockGraph`, capabilities și diagnostics.

Matricea include operațiile HTML, mutații consecutive, forward/Undo/Redo,
extends/include/import/macro/block, inheritance de pagină, content/data/
assets/styles/scripts, `load_data` static și dinamic, override local peste o
temă activă, editarea temei, config/taxonomii, create/delete/rename, revision
stale și rollback la sursă invalidă.

Evenimentele Rust pentru editor move și Undo/Redo raportează:

- `incremental` sau `fullFallback` și motivul fallback-ului;
- changed paths, template-uri și pagini invalidate;
- noduri înlocuite/reutilizate și relații reutilizate;
- clone, parse, `ComponentGraph`, `BlockGraph`, `TeraGraph` și durata totală.

Testul release pe proiectul real este ignorat implicit și se activează cu
`PANA_INCREMENTAL_BENCH_PROJECT`; testele CI obișnuite păstrează protecția
algoritmică prin fast-path obligatoriu, contoare de reutilizare și egalitate
exactă cu oracle-ul. Pragul temporal p95 rămâne separat pentru a nu transforma
variația hardware a runnerului într-un test instabil.

## Limită cunoscută

Parserul Tera folosit de scanner este mult mai lent în build debug: pe
template-ul real de aproximativ 20 KiB parsarea poate dura circa 600 ms. Acesta
este cost de build neoptimizat; acceptance-ul release este 37 ms p95. Dacă
template-ul își schimbă dependențele sau diagnosticele, rebuild-ul complet este
intenționat și rămâne în afara fast-path-ului.
