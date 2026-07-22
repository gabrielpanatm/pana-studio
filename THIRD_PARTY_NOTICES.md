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

Pană Studio distribuie bundle-ul UMD minificat Anime.js folosit pentru resursele
de animație generate în proiectele utilizatorului.

- proiect upstream: <https://github.com/juliangarnier/anime>;
- versiune: `4.4.1`;
- licență: MIT;
- textul licenței: `src-tauri/resources/licenses/ANIMEJS-MIT.txt`.

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
