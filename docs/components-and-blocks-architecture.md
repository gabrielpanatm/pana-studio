# Componente Zola/Tera și Blocuri vizuale

## Decizia de arhitectură

Pană Studio tratează **Componentele** și **Blocurile** ca două domenii diferite. Ele nu sunt variante ale aceluiași catalog și nu folosesc un model comun de definiție.

- O componentă este o entitate semantică reală a proiectului Zola/Tera: template, partial, macro, shortcode, block Tera sau structură repetitivă inline.
- Un bloc este un ansamblu vizual preasamblat: markup, stil gestionat și, opțional, comportament JavaScript cu lifecycle.
- Un template Tera poate conține blocuri, dar această relație de conținere nu transformă blocul într-o componentă Tera.
- `AGENTS`, skill-urile și directoarele personale nu sunt surse runtime ale aplicației.

Această limită elimină vechile concepte `Blueprint` și `RuntimeProvider` din `ComponentGraph`.

## Autorități Rust

### ComponentGraph

`ComponentGraph` este derivat numai din sursele proiectului și ale temei:

- definiții Tera/Zola;
- invocări și rezolvarea lor;
- parametri, argumente și legături de date;
- dependențe către template-uri, conținut, date, stiluri, scripturi, resurse și funcțiile runtime Zola;
- instanțe randate provenite din expansiunea semantică Tera;
- capabilități și diagnostice.

Originile admise sunt `project` și `theme`. Nu există provider extern de componente.

### NativeBlockRegistry

`NativeBlockRegistry` este registrul compilat în nucleul Rust. Versiunea inițială conține:

- `counter`;
- `accordion`;
- `tabs`;
- `dialog`;
- `offcanvas`;
- `nav-menu`.

Fiecare definiție are identitate și versiune stabile, familie, variantă, scară, capabilități, cerințe, sloturi și o schemă tipizată de opțiuni. Schema descrie controlul, default-ul, constrângerile și serializarea canonică în markup. UI-ul citește registrul; nu reconstruiește definițiile sau validarea în TypeScript.

### BlockGraph

`BlockGraph` este separat de `ComponentGraph` și descrie exclusiv adevărul sursă:

1. `BlockDefinition` — definiția stabilă din registru;
2. `BlockSourceInstance` — marcajul găsit într-un fișier sursă.

Instanțele randate aparțin exclusiv `CanvasGraph`. Comanda `read_ui_block_graph` construiește o proiecție efemeră `UiBlockGraph`: unește definițiile și valorile citite din sursa `ProjectWorkspace` cu instanțele Canvas ale exact aceleiași sesiuni și revizii. Relația se face prin `sourceInstanceId`; nu există un câmp `BlockGraph.renderedInstances` gol și nici două autorități concurente.

Un provider necunoscut nu este șters și nu este convertit în componentă. El rămâne instanță nerezolvată cu diagnostic explicit.

## Marcaje și compatibilitate

Markup-ul nou folosește:

```html
data-pana-block="accordion"
```

Proiectele existente care folosesc `data-pana-component` continuă să fie citite de scanner, reconciliere, duplicare și runtime. Marcajul legacy este un alias de intrare, nu o a doua schemă și nu este emis pentru blocuri noi. Instanțele legacy și providerii necunoscuți sunt read-only în panoul de proprietăți, cu diagnostic explicit. Deschiderea proiectului nu migrează și nu normalizează aceste marcaje.

Blocurile de stil noi folosesc:

```scss
/* pana:block accordion:start */
/* pana:block accordion:end */
```

Contractul poate localiza și curăța vechile delimitatoare `pana:component`. Configurația Page JS și metadata generată emit exclusiv formele canonice `blocks` și `@pana-block`; câmpul vechi `components` și comentariul `@pana-component` sunt acceptate numai la citirea proiectelor deja create.

## Mutația structurală atomică

Orice operație structurală asupra unui template — inserare, duplicare, ștergere, mutare, schimbare de tag, atribute sau marcaje — trece prin executorul Rust și prin aceeași reconciliere.

Pașii sunt calculați pe un candidat izolat:

1. se aplică schimbarea de markup;
2. se rescanează blocurile active;
3. se adaugă, actualizează sau elimină secțiunile SCSS gestionate;
4. se adaugă sau se elimină legătura stylesheet-ului paginii;
5. se reconciliază configurația Page JS;
6. toate resursele sunt staged printr-un singur `stage_composite_changes`;
7. se construiește `ProjectModel` din proiecția candidatului;
8. candidatul devine autoritativ numai dacă toate validările reușesc.

Rezultatul este o singură revizie `ProjectWorkspace` și o singură intrare Undo. Un eșec nu publică o stare parțială. O operație identică este `Noop`, fără revizie și fără istoric nou. Ștergerea ultimei instanțe elimină stilul, configurația Page JS și fișierele gestionate rămase goale.

Identitățile `class`, `data-anim` și `data-pana-instance` sunt generate în Rust. Duplicarea recalculează identitățile și evită coliziunile.

## Runtime unic

`blocks/runtime.js` este sursa canonică pentru comportamentul celor șase provideri. Același text este:

- inclus în Page JS generat pentru site;
- injectat în preview-ul interactiv.

`interactive_runtime.js` nu implementează provideri; el raportează starea preview-ului și inspecția DOM către aplicație.

Contractul lifecycle este `register`, `installPageConfig`, `reconcile`, `dispose`, `start` și `shutdown`. Fiecare provider eliberează resursele pe care le deține:

- listeners;
- `IntersectionObserver`;
- cadre `requestAnimationFrame`;
- timere;
- listeners `matchMedia`;
- focusul anterior și blocarea scrollului pentru overlay-uri.

Providerii normalizează rolurile și relațiile ARIA necesare. Evenimentele canonice sunt `pana:blocks:init` și `pana:blocks:dispose`; aliasurile `pana:components:*` rămân numai pentru proiectele vechi.

Schimbarea unei opțiuni produce un `CanvasPatch.setBlockOption` emis de Rust. Patch-ul poate actualiza inclusiv un atribut intern autorizat de registry, dar nu poate modifica identitatea providerului sau a instanței. Runtime-ul detectează schimbarea contractului, execută `dispose` și remontează instanța fără listeners, observere sau timere duplicate.

## Stiluri și design tokens

Stilurile blocurilor nu conțin culori fixe. Ele consumă contractul CSS custom properties `--pana-block-*`, cu fallback-uri bazate pe culorile de sistem. Framework-ul livrat de aplicație mapează aceste proprietăți la tokenii SCSS existenți, astfel încât schimbarea temei sau a tokenilor actualizează și blocurile.

## UI

Activitatea **Componente** afișează exclusiv catalogul semantic Zola/Tera.

Activitatea **Blocuri** afișează:

- definițiile din registrul Rust;
- filtrele de scară `element`, `section`, `composition`;
- instanțele din sursă și Canvas;
- diagnosticele;
- pregătirea inserării în elementul selectat.

În Editor, panoul drept are trei zone:

1. antetul mic al selecției, cu tag, clase și contextul celui mai apropiat bloc;
2. zona dominantă cu taburile tehnice HTML, CSS și JS, păstrate pure;
3. panoul contextual **Proprietăți bloc**, redimensionabil și pliabil.

Panoul inferior apare numai pentru rădăcina sau descendentul unui bloc. DOM-ul localizează rădăcina, dar valorile și editabilitatea sunt citite din `UiBlockGraph`. Textul și numerele se confirmă la blur sau Enter, controalele discrete la schimbare, iar un gest produce cel mult o revizie și un Undo. `Save` rămâne singura limită de scriere pe disc.

Înălțimea și starea pliată sunt setări globale ale aplicației, nu date ale proiectului.

Importul/exportul de blocuri, catalogul extern, lockfile-ul, motorul de migrare și migrarea forțată sau automată a markup-ului legacy sunt în afara acestei etape.

## Invariante verificate

- exact șase provideri nativi inițiali;
- niciun `Blueprint` sau `RuntimeProvider`;
- blocurile nu apar ca invocări în `ComponentGraph`;
- aceeași sursă runtime pentru preview și site;
- registry-ul Rust este unica schemă și validare a proprietăților;
- `BlockGraph` nu deține instanțe Canvas; `UiBlockGraph` le proiectează explicit;
- o singură tranzacție pentru markup, SCSS și Page JS;
- un singur gest de proprietate produce cel mult o revizie și un Undo;
- Undo/Redo restaurează toate resursele împreună;
- ultima ștergere curăță resursele gestionate;
- compatibilitatea legacy este numai la citire;
- frontend-ul nu execută o a doua reconciliere după mutația structurală.
