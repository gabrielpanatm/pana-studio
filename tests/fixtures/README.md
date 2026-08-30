# Fixture-uri Pană Studio

Acest director este catalogul central pentru datele și proiectele de test care
trebuie păstrate împreună cu aplicația.

## Proiecte complete

Proiectele Zola care pot fi deschise în Pană Studio stau în `projects/`:

- `index-zero/` — proiectul canonic de stres și performanță; include generatorul
  Rust și sursa Zola folosită pentru materializarea profilelor benchmarkului;
- `design-safe-zola/` — fixture vizual controlat pentru verificări de design;
- `empty-active-zola/` — proiect minim pentru stările fără conținut activ.
- `zola-upgrade-baseline/` — matricea compactă pentru comparația motorului
  embedded înainte și după upgrade; acoperă pagini, secțiuni, taxonomii,
  paginare, Sass, imagini procesate, search, feed, i18n și asset colocat.

Catalogul conține fixture-urile persistente și reutilizabile. Proiectele create
exclusiv pentru un singur test Rust sau pentru microbenchmarkul legacy de kernel
sunt materializate în directorul temporar al sistemului, au marker de ownership
și sunt eliminate după test; ele nu reprezintă copii persistente ale acestui
catalog. Proiectele din `src-tauri/resources/project-starters/` sunt produse
livrate utilizatorului, nu fixture-uri de test.

Fixture-urile mari sunt surse canonice. Testele care pot modifica fișiere trebuie
să lucreze pe copii temporare și să verifice integritatea sursei după execuție.

## Ce nu intră aici

- `src-tauri/resources/project-starters/` conține șabloane livrate utilizatorilor
  împreună cu aplicația, nu fixture-uri de test;
- fixture-urile foarte mici, folosite de un singur modul Rust, pot rămâne lângă
  testele modulului;
- copiile generate în timpul benchmarkului rămân în `benchmark-results/` și sunt
  eliminate implicit după rulare.
