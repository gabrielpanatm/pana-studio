# Contribuții la Pană Studio

Mulțumim pentru interesul de a îmbunătăți Pană Studio.

## Înainte de o modificare

- caută mai întâi în [Issues](https://github.com/gabrielpanatm/pana-studio/issues)
  dacă există deja aceeași problemă;
- pentru schimbări arhitecturale sau funcționalități mari, deschide un issue și
  descrie scopul, comportamentul propus și riscurile;
- nu include chei API, credențiale, proiecte de client sau alte date private.

## Workflow local

```bash
npm ci
npm run tauri dev
```

Înainte de un pull request rulează:

```bash
npm run check
npm run test:kernel
npm run build
cargo test --locked --manifest-path src-tauri/Cargo.toml
npm run licenses:check
```

Adaugă teste pentru bug fix-uri și pentru comportamentele noi. Actualizează
documentația și `CHANGELOG.md` când modificarea este vizibilă utilizatorilor.

Nu adăuga în Git directoarele generate (`node_modules/`, `build/`,
`.svelte-kit/`, `src-tauri/target/`) sau artefacte AppImage.

## Pull request

Descrierea trebuie să explice:

- problema rezolvată;
- soluția aleasă;
- verificările efectuate;
- orice limitare sau migrare necesară.

Prin trimiterea unei contribuții confirmi că ai dreptul să o publici și că
aceasta poate fi distribuită sub licența `EUPL-1.2-or-later` a proiectului.
