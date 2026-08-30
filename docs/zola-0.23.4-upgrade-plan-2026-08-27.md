# Plan de actualizare a motorului embedded la Zola 0.23.4

Data cercetării: 27 august 2026
Versiunea curentă în Pana: Zola 0.22.1 (`29540e9897dbe8aca388b13f7bdf615985f6ca2c`)
Versiunea țintă: Zola 0.23.4 (`28daab8d47cacb1e6c863b97739148f424433f5b`)
Stare: plan propus; implementarea nu a început.

## 1. Concluzie executivă

Zola 0.23.4 este versiunea corectă de adoptat. Nu este recomandată oprirea la 0.23.0: versiunile 0.23.1–0.23.4 repară probleme de Windows, multilingvism, URL-uri, sitemap, feed-uri, cache busting, taxonomii și randarea conținutului. Aceste corecții ating direct cazurile de utilizare ale Pana.

Upgrade-ul nu trebuie tratat ca o simplă schimbare a SHA-ului din `Cargo.toml`. Zola 0.23 este descris chiar de proiectul upstream drept probabil cea mai incompatibilă versiune Zola de până acum. Cauzele principale sunt:

- migrarea completă de la Tera 1 la Tera 2;
- eliminarea macro-urilor Tera și înlocuirea lor cu componente;
- eliminarea completă a shortcode-urilor Zola;
- executarea implicită a sintaxei Tera din conținutul Markdown;
- schimbarea API-ului public `Site`, a bibliotecii de conținut și a cache-ului de randare;
- mutarea fazelor interne de randare într-o coadă privată, ceea ce invalidează orchestrarea manuală a build-ului din Pana;
- ascunderea AST-ului și parserului intern Tera 2, pe care Source Graph le accesează astăzi direct.

Recomandarea este un upgrade într-o singură direcție, fără motor dublu, fallback la 0.22.1 sau strat de compatibilitate pentru macro-uri/shortcode-uri. Aplicația este pre-alpha, deci implementarea trebuie să elimine căile legacy și să facă din componentele Tera 2 singurul model de componentă templating.

## 2. Surse oficiale și schimbări upstream

Sursele de adevăr folosite pentru analiză:

- [Zola 0.23.4 release](https://github.com/getzola/zola/releases/tag/v0.23.4)
- [Zola 0.23.0 release](https://github.com/getzola/zola/releases/tag/v0.23.0)
- [CHANGELOG Zola](https://github.com/getzola/zola/blob/master/CHANGELOG.md)
- [Ghidul oficial de migrare Tera 2](https://github.com/Keats/tera2/blob/master/MIGRATION.md)
- [Documentația Zola pentru conținut](https://www.getzola.org/documentation/content/overview/)

### 2.1 Schimbări incompatibile din Zola 0.23.0

1. **Tera 2 înlocuiește Tera 1.** Macro-urile dispar și sunt înlocuite de componente. Unele filtre și teste sunt redenumite, eliminate sau au comportament schimbat. Validarea funcțiilor, filtrelor, testelor și componentelor devine mai strictă și are loc la compilarea template-urilor.
2. **Shortcode-urile Zola sunt eliminate complet.** Componentele Tera 2 pot fi apelate atât în template-uri, cât și în conținut.
3. **Conținutul Markdown este templatat implicit.** Orice `{{ ... }}` sau `{% ... %}` literal poate deveni expresie Tera; conținutul literal trebuie încadrat în `raw` ori exclus prin `skip_content_templating`.
4. **Căile multilingve din `get_page`/`get_section` nu mai acceptă sufixul limbii.** Trebuie folosită calea canonică și argumentul `lang`.
5. **Feature-ul Zola `native-tls` este eliminat.** Aceasta nu obligă la eliminarea TLS nativ folosit separat de alte subsisteme Pana.

### 2.2 Capabilități noi relevante

- componente Tera 2 cu argumente tipizate, valori implicite, rest arguments și body;
- `skip_content_templating` în configurație;
- `hidden` pentru pagini și secțiuni;
- `include_in_feeds` pentru pagini;
- `allow_missing` pentru `get_page` și `get_section`;
- `text_direction` și restaurarea `get_env` în 0.23.2;
- `filter` opțional pentru `resize_image`;
- `data_attr_position` pentru highlighting;
- asset-uri colocate adresabile prin linkuri interne și `get_url`;
- alias-uri accesibile în template-uri;
- timp de citire dependent de limbă;
- metadate `description` și `created` pentru imagini;
- CSS-ul temelor de syntax highlighting este generat în directorul de output, nu în `static`;
- argumentul `name` al `get_taxonomy_url` este depreciat în favoarea lui `term`.

### 2.3 De ce ținta este exact 0.23.4

Trebuie preluate cumulativ corecțiile 0.23.1–0.23.4, în special:

- compatibilitate și randare corectă pe Windows;
- panică reparată la templatarea secțiunilor;
- feed-uri și config multilingve corectate;
- asset-uri colocate ale secțiunilor;
- URL-uri pentru limba non-default, `render = false` și trailing slash;
- coliziunea indexului limbii implicite;
- date `lastmod` în sitemap;
- cache busting corect în `get_url`;
- erori Tera din headings și îmbinarea termenilor taxonomici;
- sortare stabilă după dată.

## 3. Inventarul integrării curente și impactul concret

### 3.1 Dependențe și identitatea runtime-ului

Fișiere principale:

- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock`
- `src-tauri/src/zola_engine.rs`
- `README.md`
- `THIRD_PARTY_NOTICES.md`
- `scripts/generate-third-party-notices.mjs`

Pana fixează acum `zola-site`, `zola-config` și `zola-utils` la commit-ul Zola 0.22.1 și depinde direct de Tera 1. Zola 0.23.4 rezolvă Tera 2.2.0 și introduce/actualizează dependențe precum `render`, `tera-contrib`, `giallo`, `reqwest` 0.13 și `sha2` 0.11.

Impact:

- cele trei crate-uri Zola trebuie fixate la același commit 0.23.4;
- dependența directă Tera trebuie mutată la 2.2.0, cu features compatibile cu upstream;
- lockfile-ul trebuie regenerat controlat;
- duplicatele `reqwest`/`sha2` trebuie auditate cu `cargo tree -d`; se aliniază numai dependențele directe pentru care schimbarea este sigură;
- manifestul licențelor trebuie să enumere corect toate dependențele Zola directe și tranzitive;
- versiunea și SHA-ul nu mai trebuie copiate în mai multe texte UI. Backend-ul rămâne sursa de adevăr, iar frontend-ul citește `embeddedZolaVersion` din contractul lifecycle.

### 3.2 Build-ul embedded de producție

Fișier principal: `src-tauri/src/deploy/zola.rs`.

Pana a reimplementat secvența internă de build Zola pentru a introduce checkpoint-uri de anulare între faze. În 0.23, multe dintre acele metode publice dispar, iar randarea este centralizată în coada internă privată a Zola.

Decizie arhitecturală:

- se elimină orchestrarea manuală a fazelor Zola;
- build-ul canonic apelează `Site::build()`;
- se păstrează staging-ul atomic, verificările înainte și după build și publicarea atomică a output-ului;
- anularea rămâne imediată înainte de intrarea în motor și înainte de publicare, dar în timpul apelului upstream devine „în curs de anulare” până când motorul revine;
- nu se copiază și nu se forchează implementarea cozii private Zola.

Această alegere păstrează paritatea cu upstream pentru Sass, search index multilingv, CSS de highlighting, imagini și static assets și evită o a doua implementare fragilă a motorului.

### 3.3 Motorul de preview

Fișier principal: `src-tauri/src/preview/engine.rs`.

Schimbări necesare:

- `Site::library` nu mai este un `RwLock`; se adaptează accesul la noul `Arc<Library>`;
- se folosește noul `RenderCache` public al site-ului pentru valorile canonice Tera ale paginilor, secțiunilor și configurațiilor;
- serializarea manuală duplicată a contextelor se elimină acolo unde cache-ul Zola oferă deja valoarea autoritativă;
- helper-ul eliminat `zola_utils::templates::render_template` se înlocuiește cu API-ul public Tera/Site, fără dependență pe un helper intern `pub(crate)`;
- `MacroScenario` și introspecția `template.macros` se elimină complet;
- se introduce `ComponentScenario`, bazat pe `get_component_definition` și `render_component` din Tera 2;
- argumentele preview-ului se generează exclusiv din definiția publică a componentei: tip, required/default, rest și body;
- refresh-ul incremental reconstruiește cache-ul când se schimbă biblioteca ori template-urile;
- se păstrează mutex-ul global în jurul Zola atât timp cât upstream păstrează `SITE_CONTENT` global.

Preview-ul trebuie să randeze aceeași ieșire ca build-ul de producție pentru aceeași rută și stare. Nu trebuie introdus un mini-renderer paralel pentru componente.

### 3.4 Parserul Tera și Source Graph

Fișiere principale:

- `src-tauri/src/source_graph/tera_cst.rs`
- `src-tauri/src/source_graph/tera_semantics.rs`
- `src-tauri/src/source_graph/tera.rs`
- `src-tauri/src/source_graph/component_graph.rs`
- `src-tauri/src/source_graph/scan/page.rs`
- `src-tauri/src/source_graph/zola_shortcode.rs`
- `src-tauri/src/source_graph/zola_shortcode.pest`

Tera 1 expune `Template` și AST-ul, iar Pana le folosește direct pentru semantică. Tera 2 ascunde parserul și AST-ul; actualul cod nu poate fi doar ajustat prin redenumiri.

Decizie arhitecturală:

1. Scannerul lossless deținut de Pana rămâne responsabil pentru range-uri, trivia, comentarii și editări locale.
2. Modelul semantic neutru al Pana nu mai stochează `tera::Template` și nu mai citește `tera::ast`.
3. Validarea autoritativă a template-urilor se face project-wide prin Tera configurat de Zola, deoarece Tera 2 validează la compile time și componentele sunt globale.
4. Parserul structural Pana recunoaște noile construcții relevante pentru editor, fără să pretindă că reproduce integral semantica Tera 2.

Modelul neutru trebuie să acopere cel puțin:

- definiții și apeluri de componente, inclusiv namespace;
- argumente tipizate, default, rest și body;
- `extends`, `include`, block, `for`, `if` și expresii;
- maps, spread, slices, optional chaining și ternary;
- list comprehensions și set blocks.

Se elimină:

- `MacroDefinition`, `MacroCall` și importurile de macro-uri;
- `Shortcode` ca primitivă Source Graph;
- parserul copiat din vechea gramatică Zola și fișierele `zola_shortcode.*`;
- dependențele `pest`/`pest_derive` dacă nu sunt folosite de noul parser propriu-zis.

Sintaxa veche de macro/shortcode primește un diagnostic clar de incompatibilitate, nu un runtime de compatibilitate și nu un convertor permanent.

### 3.5 Graful de componente și contractele frontend

Fișiere afectate includ `component_graph.rs`, contractele TypeScript, capabilitățile, mutațiile, navigarea/proveniența, adnotarea preview-ului și testele.

Noul model unic:

- `TeraComponentDefinition` globală/namespaced;
- `TeraComponentCall` din template ori Markdown;
- argumente declarate și argumente furnizate;
- range pentru apel, body și fiecare argument;
- relații consumer → componentă;
- diagnostic pentru componentă lipsă, argument lipsă, argument necunoscut sau tip incompatibil.

Nu trebuie păstrate în paralel variantele Macro, Shortcode și Component. Fingerprint-urile incrementale și indexurile se regenerează pentru noul model.

### 3.6 Catalogul funcțiilor Zola

Fișier principal: `src-tauri/src/source_graph/zola/runtime.rs`.

Catalogul fixat la vechiul SHA trebuie reaudiat după registrele reale din Zola 0.23.4. Sunt necesare cel puțin:

- adăugarea `text_direction` și `get_env`;
- actualizarea semnăturii `get_taxonomy_url` pentru `term` și diagnostic de deprecire pentru `name`;
- suport pentru `allow_missing` la `get_page`/`get_section`;
- validarea căilor canonice plus `lang`;
- actualizarea filtrelor/testelor redenumite sau eliminate în Tera 2;
- separarea corectă a funcțiilor înregistrate devreme de cele care depind de librăria site-ului.

### 3.7 UI pentru componente

Fișiere principale:

- `src/lib/components/creation/ComponentsWorkspace.svelte`
- `src/lib/components/creation/TemplatesWorkspace.svelte`
- modelele și store-urile aferente;
- cheile de localizare și fișierele generate.

Schimbări:

- tab-urile Macro și Shortcode dispar;
- se introduce un singur tab Components, cu filtrare după namespace și origine;
- wizard-ul creează componente Tera 2 cu nume, namespace, argumente tipizate, default/rest/body și exemplu de apel;
- acțiunile de navigare, rename, delete și „find usages” operează pe modelul unic;
- textele despre shortcode-uri și macro-uri se elimină din toate localele;
- artefactele i18n se regenerează din sursele locale, nu se corectează doar fișierele generate.

### 3.8 Settings, front matter și inspectorul de imagini

Fișiere principale:

- `src-tauri/src/commands/config/zola_settings.rs`
- contractul TypeScript `ZolaProjectSettings`
- `ProjectSettingsWorkspace.svelte`
- `src/lib/markdown/frontmatter.ts`
- `ProjectPageSettingsTab.svelte`
- controalele care emit `resize_image`.

Capabilități de expus:

- `skip_content_templating: Vec<String>` în settings, cu editare lossless TOML;
- `markdown.highlighting.data_attr_position` cu valori validate;
- `hidden` pentru pagină și secțiune; pentru secțiuni UI-ul trebuie să păstreze starea moștenită, deci nu este suficient un checkbox boolean;
- `include_in_feeds` pentru pagini, cu default upstream `true` și fără scriere inutilă a cheii;
- `filter` opțional în `resize_image`, păstrând absența ca default upstream.

Funcționalitățile nu trebuie doar citite. Pentru fiecare câmp sunt necesare round-trip TOML/front matter, UI, contract Rust–TypeScript și test de build Zola.

### 3.9 Startere, fixture-uri și documentație

Cele cinci startere bundled trebuie validate și apoi marcate `tested = "0.23.4"`. În fixture-ul `index-zero` există deja utilizări ale filtrului Tera 1 `slice`, eliminat în Tera 2; acestea trebuie rescrise în sintaxa nativă de slicing Tera 2.

Trebuie auditate toate template-urile și fișierele Markdown bundled pentru:

- macro-uri și shortcode-uri vechi;
- filtre/teste Tera 1 eliminate sau redenumite;
- fragmente literale `{{ ... }}` / `{% ... %}` care vor fi executate;
- căi multilingve cu sufix de limbă;
- `get_taxonomy_url(name=...)`.

Se actualizează versiunea/SHA-ul în README, About, notices, teste și metadata starterelor. Referințele istorice din changelog și baseline-urile istorice de performanță nu se rescriu.

## 4. Plan de implementare pe etape

### Etapa 0 — Baseline reproductibil înainte de upgrade

Obiectiv: capturarea comportamentului util actual, astfel încât schimbările intenționate să fie separate de regresii.

Lucrări:

1. Rulare și arhivare rezultate pentru build frontend, teste kernel, `cargo test --locked`, testul runtime-ului embedded și toate starterele.
2. Fixture-uri reprezentative pentru preview/build: pagină, secțiune, taxonomie, paginare, Sass, image processing, search, feed, i18n și asset colocat.
3. Captură de timp, memorie și dimensiune binar pentru scenariile standard; se creează un baseline nou, fără suprascrierea documentelor istorice.
4. Test explicit pentru publicarea atomică și anularea înainte/după build.

Criteriu de acceptare: baseline-ul este verde ori fiecare problemă preexistentă este documentată înainte de schimbarea dependențelor.

### Etapa 1 — Pin unic la Zola 0.23.4 și compilare minimă

Obiectiv: un singur runtime Zola 0.23.4 în dependency graph.

Lucrări:

1. Fixarea `zola-site`, `zola-config`, `zola-utils` la commit-ul complet al tagului 0.23.4.
2. Actualizarea Tera direct la 2.2.0 și eliminarea features Tera 1 inexistente.
3. Regenerarea lockfile-ului.
4. Actualizarea constantelor backend pentru versiune/SHA; UI va consuma contractul backend.
5. Audit `cargo tree -d`, licențe și surse git.
6. Rezolvarea exclusivă a erorilor de API necesare pentru compilare; niciun shim Tera 1.

Criteriu de acceptare: `cargo check --locked` trece, toate crate-urile Zola provin din același commit și nu există Tera 1 în graph.

### Etapa 2 — Înlocuirea semanticii Tera 1 și eliminarea legacy

Obiectiv: Source Graph și component graph modelează Tera 2 fără AST intern upstream.

Lucrări:

1. Separarea CST lossless de validarea autoritativă.
2. Implementarea IR-ului neutru Tera 2 și a scanării structurale necesare editorului.
3. Model unic definition/call pentru componente.
4. Ștergerea parserului și gramaticii shortcode, a modelelor macro/shortcode și a căilor de reconciliere aferente.
5. Diagnostic explicit pentru sintaxă legacy.
6. Actualizarea catalogului runtime Zola/Tera.
7. Actualizarea fingerprint-urilor și a rebuild-ului incremental.

Criteriu de acceptare: nu mai există referințe de producție la AST-ul Tera 1, Macro ori Shortcode; definițiile/apelurile de componente sunt indexate cu range-uri corecte în template și Markdown.

### Etapa 3 — Motor embedded, producție și preview

Obiectiv: build-ul și preview-ul folosesc API-ul canonic Zola 0.23.4.

Lucrări:

1. Înlocuirea fazelor manuale cu `Site::build()`.
2. Păstrarea staging-ului/publicării atomice și adaptarea stării de anulare.
3. Adaptarea `Library`/`RenderCache` și eliminarea serializării manuale duplicate.
4. Înlocuirea helperelor Zola eliminate cu API-uri publice stabile.
5. Implementarea preview-ului de componente prin metadatele publice Tera 2.
6. Reconstrucția corectă a cache-ului la refresh incremental.
7. Validarea noii locații a CSS-ului de highlighting și a copierii asset-urilor.

Criteriu de acceptare: pentru matricea de fixture-uri, preview-ul și build-ul de producție produc rezultate echivalente; un build anulat nu publică output parțial.

### Etapa 4 — Contracte și experiența Components

Obiectiv: frontend-ul expune un singur concept coerent de componentă Tera 2.

Lucrări:

1. Actualizarea contractelor Rust–TypeScript și a serializării.
2. Refactorizarea Components Workspace și a wizard-ului.
3. Find usages, navigation, rename, delete, provenance și adnotări preview.
4. Eliminarea completă a UI-ului și localizărilor macro/shortcode.
5. Regenerarea fișierelor i18n și actualizarea testelor UI/model.

Criteriu de acceptare: utilizatorul poate crea, apela, previzualiza, găsi și modifica o componentă Tera 2 fără căi legacy vizibile sau cod mort.

### Etapa 5 — Funcționalități Zola 0.23 în editor

Obiectiv: noile opțiuni care afectează fidelitatea proiectului sunt editabile, nu doar tolerate.

Lucrări:

1. `skip_content_templating` și `data_attr_position` în settings.
2. `hidden` și `include_in_feeds` în front matter.
3. filtrul de sampling pentru `resize_image`.
4. semnături runtime pentru `allow_missing`, `text_direction`, `get_env`, `term` și `lang`.
5. teste lossless/read–modify–write și build pentru fiecare opțiune.

Criteriu de acceptare: deschiderea și salvarea unui proiect 0.23.4 nu pierde aceste câmpuri, iar valorile introduse în UI produc un build valid.

### Etapa 6 — Conversia conținutului bundled și metadata

Obiectiv: toate resursele livrate de Pana sunt native 0.23.4.

Lucrări:

1. Conversia fixture-urilor Tera 1 și eliminarea oricărui shortcode/macro bundled.
2. Auditul Markdown templating și folosirea `raw`/`skip_content_templating` numai unde conținutul este intenționat literal.
3. Build embedded pentru toate cele cinci startere.
4. Actualizarea `tested`, README, About, notices, teste și comentarii active.
5. Regenerarea `THIRD_PARTY_NOTICES.md` și verificarea licențelor.

Criteriu de acceptare: fiecare starter se creează, se deschide, se previzualizează și se construiește cu motorul embedded 0.23.4; căutarea după versiunea/SHA-ul vechi găsește numai istoric explicit.

### Etapa 7 — Validare finală și pregătirea release-ului

Obiectiv: confirmarea funcțională, arhitecturală și de performanță.

Gate-uri obligatorii:

1. `npm run check`
2. testele frontend/kernel și build-ul frontend
3. `cargo test --locked`
4. testul runtime embedded, care confirmă și absența unui sidecar `zola`
5. verificarea licențelor
6. toate starterele și matricea multilingvă
7. smoke test în aplicația reală pentru preview, editare și deploy local
8. build AppImage și smoke test pe artefact
9. comparație de performanță, memorie și dimensiune cu baseline-ul etapei 0
10. audit `rg` pentru legacy, `cargo tree -d` și cod mort

Criteriu de acceptare: toate gate-urile sunt verzi, nu există fallback la 0.22.1, macro/shortcode runtime sau faze Zola duplicate, iar regresiile de performanță semnificative sunt explicate și rezolvate.

## 5. Matrice minimă de teste noi

### Tera 2 și componente

- componentă simplă și namespaced;
- argument required, typed, default, rest și body;
- apel din template și apel din Markdown;
- componentă nested și eroare de componentă/argument lipsă;
- maps, spread, slicing, optional chaining, ternary și comprehension;
- diagnostic pentru macro și shortcode vechi;
- filtrele/testele redenumite și eroare pentru cele eliminate.

### Templatarea conținutului

- expresie Tera validă în Markdown;
- text literal protejat prin `raw`;
- fișier exclus prin `skip_content_templating`;
- heading cu eroare Tera și diagnostic/range util.

### Multilingvism și URL-uri

- `get_page`/`get_section` cu path canonic plus `lang`;
- rută non-default cu trailing slash;
- homepage pentru limba default fără coliziune;
- feed și search index în mai multe limbi;
- `text_direction`;
- asset colocat de pagină și secțiune;
- `get_url` cu `render = false` și cache busting.

### Front matter și output

- `hidden` explicit și moștenit;
- excludere prin `include_in_feeds = false`;
- `allow_missing` true/false;
- `get_taxonomy_url(term=...)` și diagnostic pentru `name`;
- sitemap `lastmod`;
- taxonomii cu termeni combinați;
- CSS de highlighting în output și `data_attr_position`;
- `resize_image(filter=...)` și noile image metadata.

### Runtime Pana

- paritate preview/build pentru toate scenariile;
- refresh incremental al template-ului, componentei și conținutului;
- Source Graph range/provenance și find usages;
- anulare înainte, în timpul și după apelul Zola;
- output atomic la succes și la eroare;
- lipsa sidecar-ului și raportarea corectă a versiunii embedded.

## 6. Riscuri și măsuri de control

| Risc | Măsură |
| --- | --- |
| Reimplementarea incompletă a parserului Tera 2 | Parser structural limitat la nevoile editorului; Tera/Zola rămâne validatorul autoritativ project-wide. |
| Divergență între preview și build | Context din `RenderCache`, funcții Zola înregistrate de `Site` și teste golden pe aceleași fixture-uri. |
| Pierderea anulării fine-grained | Apel canonic `Site::build`, status „cancelling”, staging atomic; fără fork intern upstream. |
| Conținut Markdown interpretat accidental ca Tera | Audit automat, diagnostic clar, `raw` sau `skip_content_templating` explicit. |
| Teme/proiecte Tera 1 incompatibile | Diagnostic de versiune; fără motor dual sau compatibilitate permanentă. |
| Dublarea dependențelor și creșterea binarului | `cargo tree -d`, baseline de dimensiune și aliniere selectivă a dependențelor directe. |
| Câmpuri 0.23 pierdute la salvare | Contracte tipizate și teste lossless TOML/front matter. |
| Cod legacy rămas în UI/model | Audit final `rg`, ștergerea variantelor și testelor vechi, nu doar ascunderea lor. |

## 7. Decizii care nu trebuie redeschise în timpul implementării

- Versiunea țintă este 0.23.4, nu 0.23.0–0.23.3.
- Upgrade-ul este embedded; nu se introduce binar extern Zola.
- Nu se păstrează suport runtime pentru Tera 1, macro-uri sau shortcode-uri.
- Nu se creează un motor dublu 0.22/0.23 și nici migrare permanentă.
- Build-ul de producție apelează API-ul canonic `Site::build()`.
- Pana deține CST-ul/range-urile editorului; Zola/Tera dețin validarea semantică autoritativă.
- Componentele Tera 2 devin singura primitivă de reutilizare în template și Markdown.
- Baseline-urile și changelog-ul istoric rămân neschimbate; se adaugă artefacte noi.

## 8. Definition of Done

Upgrade-ul este complet numai când:

- aplicația compilează și rulează exclusiv cu Zola 0.23.4/Tera 2;
- runtime-ul raportează versiunea și SHA-ul corecte dintr-o singură sursă backend;
- toate starterele și fixture-urile bundled sunt native 0.23.4;
- preview-ul, Source Graph, componentele, settings și front matter cunosc noile contracte;
- nu există parser shortcode copiat, modele macro/shortcode, fallback 0.22.1 sau orchestrare paralelă a build-ului;
- matricea de teste și toate gate-urile release sunt verzi;
- documentația, licențele și metadata sunt actualizate;
- performanța și dimensiunea binarului sunt comparate cu baseline-ul și orice regresie relevantă este rezolvată.
