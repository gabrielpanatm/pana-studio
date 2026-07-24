# Pană Studio

Pană Studio este un editor vizual desktop open-source pentru site-uri construite
cu [Zola](https://www.getzola.org/). Aplicația combină un preview controlat cu
editarea surselor HTML/Tera, SCSS, Markdown și JavaScript într-un workspace
local. Este construită cu Tauri 2, Rust, SvelteKit și TypeScript.

> **Stadiu:** versiune publică de test `0.1.1`. Interfața este în limba română,
> iar distribuția pregătită în prezent este pentru Linux x86-64. Fă backup sau
> inițializează versionarea Git înainte de a lucra pe un proiect important.

## Funcționalități

- creare și deschidere de proiecte Zola;
- preview vizual izolat și editare sincronizată cu sursele proiectului;
- navigare prin fișiere, straturi și structura site-ului;
- catalog și administrare Rust-first pentru teme și șabloane Zola;
- blocuri native configurabile, componente Tera și editare vizuală a datelor;
- sistem de design cu tokeni, clase și stilurile tematice ale elementelor;
- editare HTML/Tera, SCSS, Markdown și comportamente JavaScript;
- timeline pentru animații și generare locală de resurse Anime.js;
- gestionare imagini, fonturi și resurse;
- build Zola și deploy opțional către Bunny Storage/CDN;
- versionare Git locală cu status, diff, istoric, branch-uri, fetch, push și
  integrare explicită;
- integrare opțională AI prin server MCP local pentru Codex;
- recovery tranzacțional și protecții pentru conflictele dintre editor, disk și
  operațiile externe.

Interfața este organizată în activități dedicate — Editor, Șabloane,
Componente, Blocuri, Sistem de design, Resurse, Conținut, Date, Teme,
Control versiuni, Probleme și audit și Publicare. Setările aplicației sunt
separate de proiectul deschis. Navigarea restaurabilă, documentele,
split-urile, viewport-ul și panoul inferior sunt proiectate dintr-o stare
canonică administrată de nucleul Rust; frontendul Svelte nu păstrează un al
doilea model al proiectului.

Pană Studio integrează direct motorul Rust Zola `0.22.1`, fixat la revizia
upstream `29540e9897dbe8aca388b13f7bdf615985f6ca2c`. Preview-ul, validarea și
buildul folosesc crate-urile oficiale ale acestei revizii; aplicația nu livrează,
nu caută și nu pornește un executabil Zola separat. Nu este necesară instalarea
Zola pentru funcțiile oferite de aplicație. Proveniența motorului este
documentată în [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).

## Instalare din AppImage

După publicarea unei versiuni, descarcă fișierul `.AppImage` și fișierul său
`.sha256` din [pagina Releases](https://github.com/gabrielpanatm/pana-studio/releases).
Verifică și pornește aplicația astfel:

```bash
sha256sum --check "Pana.Studio_0.1.1_amd64.AppImage.sha256"
chmod +x "Pana.Studio_0.1.1_amd64.AppImage"
./"Pana.Studio_0.1.1_amd64.AppImage"
```

AppImage-ul nu necesită instalare. Compatibilitatea depinde de distribuția și
versiunea Linux folosite pentru build; fiecare release trebuie să precizeze
platforma pe care a fost construit și testat.

## Dezvoltare locală

### Cerințe

- Linux x86-64;
- Node.js `20.19` sau mai nou; Node.js 24 LTS este recomandat;
- Rust stable și Cargo;
- Git;
- [dependențele de sistem Tauri 2 pentru Linux](https://v2.tauri.app/start/prerequisites/).

Pe Debian/Ubuntu, dependențele Tauri recomandate sunt:

```bash
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev \
  build-essential curl wget file libxdo-dev libssl-dev \
  libayatana-appindicator3-dev librsvg2-dev
```

Instalează dependențele proiectului și pornește aplicația:

```bash
npm ci
npm run tauri dev
```

## Verificări

```bash
npm run check
npm run test:kernel
npm run build
cargo test --locked --manifest-path src-tauri/Cargo.toml
npm run licenses:check
```

Testul browser real este separat și presupune un preview local pregătit:

```bash
npm run test:preview-browser
```

## Build AppImage

Actualizează mai întâi inventarul licențelor, apoi construiește pachetul:

```bash
npm run licenses:generate
npm run tauri build
```

Artefactul rezultat se găsește în:

```text
src-tauri/target/release/bundle/appimage/
```

`src-tauri/target/`, `node_modules/`, `build/` și artefactele AppImage sunt
generate local și nu trebuie adăugate în Git.

Un tag Git cu forma `v0.1.1` pornește workflow-ul de release pe Ubuntu 22.04.
Acesta repetă verificările, construiește AppImage-ul, generează checksum-ul și
publică release-ul GitHub ca prerelease după finalizarea cu succes.

## Structura repository-ului

- `src/` — interfața SvelteKit și logica frontend;
- `src-tauri/src/` — nucleul Rust, comenzile Tauri și serviciile locale;
- `src-tauri/resources/` — Zola, startere, template-uri și licențe distribuite;
- `tests/` — teste de regresie și contracte frontend;
- `docs/` — documentație și audituri de arhitectură;
- `scripts/` — utilitare pentru dezvoltare, build și conformitate.

Arhitectura versionării Git este descrisă în
[`docs/git-versioning-architecture.md`](docs/git-versioning-architecture.md).
Reconstrucția Workbench-ului, principiile Rust-first și verificările de livrare
sunt documentate în
[`docs/ux-reconstruction.md`](docs/ux-reconstruction.md).

## Siguranța datelor

Pană Studio poate modifica fișierele proiectului deschis, poate rula build-uri
Zola și poate executa operații Git sau deploy cerute explicit. Folosește-l pe
proiecte versionate și nu comite fișiere `.env`, chei API sau alte credențiale.

Aplicația nu stochează credențialele Git. Remote-urile HTTPS folosesc credential
helper-ul Git, iar remote-urile SSH folosesc cheia și agentul sistemului.

Pentru vulnerabilități, consultă [`SECURITY.md`](SECURITY.md).

## Contribuții

Issues și pull request-uri sunt binevenite. Pentru modificări importante,
deschide mai întâi un [issue](https://github.com/gabrielpanatm/pana-studio/issues)
care descrie problema și soluția propusă. Vezi
[`CONTRIBUTING.md`](CONTRIBUTING.md).

## Licență

Copyright © 2026 Gabriel Pană.

Pană Studio este licențiat sub
[European Union Public Licence 1.2 sau o versiune ulterioară](LICENSE)
(`EUPL-1.2-or-later`). Componentele terțe rămân sub licențele autorilor lor;
detaliile sunt în [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md) și în
`src-tauri/resources/licenses/`.
