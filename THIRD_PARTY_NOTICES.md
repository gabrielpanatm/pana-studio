# Third-party notices

Pană Studio include și distribuie componente open-source ale altor autori.
Licența Pană Studio nu înlocuiește și nu restrânge licențele acestor componente.

## Zola 0.22.1

Pană Studio integrează motorul Rust oficial Zola pentru preview-ul
tranzacțional, Source Browser, validare și build. Nu distribuie și nu pornește
un executabil Zola separat.

- proiect upstream: <https://github.com/getzola/zola>;
- versiune: `0.22.1`;
- revizie sursă: `29540e9897dbe8aca388b13f7bdf615985f6ca2c`;
- pachete Cargo integrate: `site` și `utils`, redenumite local `zola-site` și
  `zola-utils`;
- modificări aduse sursei upstream: niciuna;
- licențe upstream: EUPL-1.2 pentru codul nou și MIT pentru codul care precedă
  schimbarea licenței indicată de proiectul Zola.

Textele relevante sunt distribuite în:

- `src-tauri/resources/licenses/ZOLA-EUPL-1.2.txt`;
- `src-tauri/resources/licenses/ZOLA-MIT.txt`.

Sursa exactă corespunzătoare motorului inclus rămâne disponibilă în
repository-ul upstream la revizia menționată. `Cargo.toml` fixează această
revizie, astfel încât motorul nu poate fi actualizat implicit.

## Anime.js 4.4.1

Pană Studio integrează bundle-ul UMD minificat Anime.js exclusiv în Preview-ul
Motion intern. În proiectele utilizatorului materializează numai închiderea de
dependențe a modulelor ESM oficiale necesare animațiilor configurate, fără npm,
bundler sau CDN.

- proiect upstream: <https://github.com/juliangarnier/anime>;
- versiune: `4.4.1`;
- licență: MIT;
- textul licenței: `src-tauri/resources/licenses/ANIMEJS-MIT.txt`;
- sursele ESM publicate includ și copia `LICENSE.md` din distribuția upstream.

## Biblioteca offline de fonturi

Pană Studio include 36 de familii din Google Fonts ca fișiere WOFF2 variabile
pentru subseturile Latin și Latin Extended. Catalogul, URL-urile exacte ale
surselor, amprentele SHA-256 și data inventarului (`2026-08-14`) sunt păstrate
în `src-tauri/resources/font-library/catalog.json`.

- 35 de familii sunt distribuite sub SIL Open Font License 1.1;
- Roboto Slab este distribuit sub Apache License 2.0;
- textul exact al licenței fiecărei familii se află lângă fișierele sale în
  `src-tauri/resources/font-library/<familie>/LICENSE.txt`;
- instalarea unei familii copiază în proiect numai fișierele selectate și
  licența corespunzătoare; utilizarea nu depinde de Google Fonts sau de CDN.

Google Fonts și numele familiilor aparțin titularilor lor. Includerea lor nu
implică afilierea sau aprobarea Pană Studio de către Google.

## Fonturile interfeței

Inter, Urbanist și JetBrains Mono sunt incluse ca fișiere WOFF2 variabile în
bundle-ul aplicației, prin pachetele Fontsource corespunzătoare. Pană Studio
folosește fișierele nemodificate și nu depinde de fonturi instalate local sau de
un serviciu web extern.

- Inter — interfața generală;
- Urbanist — titlurile interfeței;
- JetBrains Mono — editorul de cod și valorile tehnice;
- licența tuturor celor trei familii: SIL Open Font License 1.1.

Textele complete ale licențelor sunt preluate din pachetele distribuite și
incluse de generator în `src-tauri/resources/licenses/THIRD_PARTY_LICENSES.txt`.

## Dependențe JavaScript și Rust

Inventarul pachetelor rezolvate din `package-lock.json` și `Cargo.lock`, împreună
cu textele de licență disponibile în distribuțiile lor, este generat în:

`src-tauri/resources/licenses/THIRD_PARTY_LICENSES.txt`

Inventarul poate fi regenerat și verificat astfel:

```bash
npm run licenses:generate
npm run licenses:check
```

În cazul unei diferențe între acest rezumat și textul unei licențe terțe,
textul licenței terțe prevalează.
