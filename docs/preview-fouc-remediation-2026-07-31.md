# Remediere FOUC în Preview

Data: 2026-07-31

## Rezultat

Flash-ul de HTML fără CSS observat la mutare și la Undo/Redo a fost eliminat
fără să fie încetinită proiecția live. Soluția păstrează Rust drept sursă de
adevăr, iar patch-ul DOM optimist rămâne separat de verificarea canonică
Rust/Zola.

Remedierea acoperă și „soft refresh”-ul ulterior: în WebKit, fonturile locale
nu mai sunt invalidate când mutația schimbă numai structura din `<body>`.

Pe proiectul real
`/home/gabriel/Documente/studio.pana.tm.ro/sursa`, WebKit/Tauri a fost
eșantionat la fiecare `requestAnimationFrame`:

| Scenariu | Cadre eșantionate | Cadre fără stiluri |
| --- | ---: | ---: |
| Undo + Redo | 3.417 | 0 |
| mutare fizică în canvas | 4.931 | 0 |
| total | 8.348 | 0 |

În toate stările distincte au rămas active exact două stylesheet-uri, fontul
calculat a rămas `Geist, system-ui, sans-serif`, iar fundalul a rămas
`rgb(237, 241, 238)`.

## Cauza

Problema avea două componente care se amplificau reciproc:

1. Rust adăuga `__pana_preview_revision` fiecărei resurse locale. O modificare
   exclusiv structurală schimba astfel URL-ul CSS, deși bytes erau identici.
2. Bridge-ul încărca foaia nouă cu `media="not all"`, apoi o activa și elimina
   foaia veche înainte de bariera vizuală `styledReady`. WebKit putea picta un
   cadru între aceste operații.

CSS era într-adevăr injectat/reconciliat live, dar identitatea globală per
revizie îl făcea să arate ca o resursă nouă la fiecare mutație.

Auditul ulterior al layout shift-ului fonturilor a găsit încă două cauze:

3. Proveniența Tera din documentul canonic putea adăuga sau elimina comentarii
   din `<head>`. Reconcilierea reașeza apoi toate nodurile pentru a reproduce
   ordinea canonică, inclusiv exact aceleași instanțe `<link>`.
4. WebKit invalidează CSSOM și `FontFaceSet` chiar dacă un `<link
   rel="stylesheet">` nemodificat este doar mutat în același `<head>`.

În reproducerea reală de dinaintea ultimei corecții, o mutare a produs două
tranziții ale `document.fonts`, un cadru de fallback și 60 de adăugări plus 60
de eliminări observate în `<head>`, fără request nou de font și fără înlocuirea
instanțelor `<link>`. Aceasta a separat cauza de rețea: era invalidare WebKit
provocată de mutarea DOM a resursei existente.

Mai exista și un defect structural independent: placeholderul vizual pentru
un bloc Tera gol era decis după numele blocului. Blocuri localizate sau cu nume
neprevăzute puteau primi un `<div>` în `<head>`, lăsând parserul HTML să repare
documentul implicit.

## Schimbări

### Identitate de resursă în Rust

- URL-urile resurselor locale folosesc acum
  `__pana_resource_hash=sha256-<digest>`.
- Același conținut păstrează același URL între revizii; conținutul schimbat
  primește alt URL.
- Query-ul și fragmentul autorului sunt păstrate.
- URL-urile externe, protocol-relative, `data:`, `blob:`, `mailto:` și `tel:`
  nu sunt rescrise.
- Dacă manifestul nu conține o resursă, fallback-ul rămâne revizia exactă,
  fail-closed.
- Serverul Preview rezolvă hash-ul exact în generațiile active, staged sau
  retired și livrează resursa ca immutable cu ETag.
- Pentru o schimbare numai de template/HTML, manifestul de resurse al
  generației anterioare este reutilizat. În testul real,
  `resourceManifestMs` a rămas `0` pe aceste generații.

Resource tree-ul WebKit real a confirmat URL-uri stabile precum:

```text
/css-framework/framework.css?...&__pana_resource_hash=sha256-...
/pagini/index.css?...&__pana_resource_hash=sha256-...
```

### Promovare CSS atomică

Bridge-ul aplică acum tranzacția în ordinea:

```text
staging CSS nou cu media="not all"
  → toate foile noi sunt încărcate
  → activare și ordine canonică a cascadei
  → reconciliere DOM
  → fonts.ready + două cadre
  → styledReady
  → retragere CSS vechi
```

Foile vechi rămân montate și restaurabile până după `styledReady`; schimbarea
de stare a foii vechi și activarea celei noi au loc în aceeași operație
JavaScript, fără un punct de `await` între ele. La eroare, bridge-ul elimină
numai foile staged și restaurează atributele foilor reutilizate; documentul
stilizat anterior rămâne intact.

Testul browser real pentru CSS schimbat a raportat:

- promovare: `reused=0`, `staged=1`, `retired=1`,
  `activationToStyledMs=30`;
- reconciliere identică: `reused=1`, `staged=0`, `retired=0`,
  `activationToStyledMs=31`;
- stylesheet 404: candidatul a fost respins, iar documentul și foaia veche au
  rămas active.

### Reconciliere diferențială a `<head>` și fonturilor

- Atributele stylesheet/preload sunt scrise numai când valoarea canonică este
  realmente diferită. Reconcilierea identică produce zero scrieri.
- `title`, `base`, `meta`, `link`, `script`, ID-urile și nodurile text/comentariu
  au chei semantice; nodurile compatibile sunt actualizate în loc.
- Ordinea relativă a resurselor reutilizate este comparată înainte de commit.
  Dacă este aceeași, resursele sunt ancore imobile, iar comentariile și
  metadatele sunt reconciliate în jurul lor.
- Dacă documentul canonic schimbă realmente ordinea resurselor, schimbarea nu
  este ascunsă: nodurile sunt mutate și metrica `headNodesReordered` o
  înregistrează.
- Preload-urile noi sunt staged și așteptate înainte de commit. Preload-urile
  stabile își păstrează identitatea DOM.
- Bariera `styledReady` verifică `document.fonts.ready`, starea fețelor și două
  cadre. Un timeout sau o față nouă eșuată refuză candidatul.

În Rust, adnotarea Tera folosește acum contextul structural rezultat din CST:

- bloc gol demonstrat în `<body>`: poate primi placeholder vizual;
- bloc în `<head>`: păstrează numai comentariile de proveniență;
- context necunoscut: fail-closed, fără element vizual inventat.

Testul include explicit un bloc localizat `{% block preincarcare %}` în
`<head>` și un bloc gol în `<body>`.

### Navigation fallback

Calea rară de navigare completă este acoperită de un guard vizual:

- documentul progresiv din iframe nu devine vizibil înainte de receipt-ul
  `styledReady`;
- utilizatorul vede un strat neutru în tema aplicației, nu un flash alb;
- la timeout/eșec se revine la URL-ul ultimei generații stilizate;
- guard-ul se retrage numai când identitatea exactă a documentului recuperat
  ajunge la `styledReady`.

### Observabilitate

Evenimentul `kernel.preview.canvas.stylesheets_promoted` înregistrează:

- `projectionMode`;
- `stylesheetsReused`;
- `stylesheetsStaged`;
- `stylesheetsRetired`;
- preload-uri reutilizate, staged și retrase;
- noduri de head reutilizate, create, retrase și reordonate;
- scrieri de atribute stylesheet/preload;
- invalidări de font, cadre de fallback și delta maximă a metricii textului;
- timpul până la `document.fonts.ready`;
- `stylesheetActivationToStyledMs`.

Evenimentele canonice existente păstrează timpii
`resourcesReady → committed → styledReady`, identitatea tranzacției și motivul
unui fallback. Logurile testului real confirmă reconciliere `in_place`,
`resourceManifestMs=0` și zero resurse CSS staged/retrase pentru mutațiile
exclusiv structurale.

## Latență

În testul Firefox real, după remediere:

| Metrică | Rezultat |
| --- | ---: |
| CanvasPatch p95 | 2 ms |
| patch Undo/Redo maxim | 2 ms |
| drag preview round-trip | 17 ms |
| document iframe păstrat | da |

Reconcilierea identică cu font WOFF2 real a raportat:

| Metrică | Rezultat |
| --- | ---: |
| stylesheet/preload reutilizate | 1 / 1 |
| atribute stylesheet/preload scrise | 0 / 0 |
| noduri head create/retrase/reordonate | 0 / 0 / 0 |
| invalidări font | 0 |
| cadre fallback | 0 |
| delta metrică text | 0 |
| request-uri noi pentru font | 0 |

Testul separat de ordine inversează și restaurează aceleași resurse și confirmă
că o schimbare canonică reală nu este suprimată.

Costul canonic Zola continuă în fundal și nu blochează feedback-ul live.

## Verificare

- Rust: 1.248 teste trecute, 0 eșuate, 2 ignorate intenționat;
- frontend/kernel: 66/66 teste trecute;
- Svelte/TypeScript: 0 erori, 0 avertismente;
- `cargo fmt --check`: trecut;
- Firefox real: promovare, reutilizare, reordonare semantică, rollback CSS,
  drag, Undo și Redo trecute;
- WebKit/Tauri real: mutare fizică, Undo și Redo, 8.348 cadre fără FOUC;
- auditul WebKit/Tauri pentru fonturi, cumulat peste drag, Undo și Redo:
  `FontFaceSet transitions=0`, `FontFace transitions=0`,
  `fallback frames=0`, `max text metric delta=0`,
  `stable resource moves=0`, `stable resource attribute writes=0`;
- la final, Geist, Bricolage Grotesque și Geist Mono au rămas `loaded`, iar
  ordinea elementelor a fost restaurată;
- sursa proiectului real nu a fost salvată sau modificată pe disc;
- nu au fost folosite MCP sau mecanisme noi de lease.

## Riscuri reziduale

- Guard-ul pentru navigation fallback ascunde încărcarea într-un singur iframe;
  nu este un al doilea webview complet. Este suficient pentru a nu expune
  documentul progresiv, dar un viitor double-buffer ar permite și tranziții
  între două generații deja pictate.
- Fonturile externe pot întârzia `styledReady` până la bugetul existent.
- O schimbare reală a CSS-ului sau a ordinii resurselor poate invalida legitim
  fonturile în WebKit. Calea este instrumentată și așteaptă din nou bariera de
  font; garanția zero-invalidare se aplică mutațiilor structurale cu resurse
  neschimbate.
- Hash-ul stabil rezolvă identitatea și cache-ul CSS. Costul randării canonice
  site-wide pentru modificări globale rămâne o limită separată și nu este pus
  pe calea critică a mutației live.
