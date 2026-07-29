# Traducerea Pană Studio

Pană Studio folosește [Project Fluent](https://projectfluent.org/) și păstrează
sursele canonice în acest director. Aceleași fișiere sunt încorporate de
nucleul Rust și generate tipizat pentru frontend; nu există un registru manual
de limbi.

## Adăugarea unei limbi

1. Copiază directorul `en-US/` sub un tag BCP-47 canonic, de exemplu `fr` sau
   `pt-BR`.
2. Completează `manifest.json`:
   - `locale` trebuie să fie identic cu numele directorului;
   - `nativeName` este numele limbii scris în limba respectivă;
   - `direction` este `ltr` sau `rtl`;
   - `contributors` conține cel puțin un nume sau identificator public.
3. Tradu toate domeniile `.ftl`. `en-US` este sursa și fallbackul stabil.
4. Rulează `npm run i18n:generate`, `npm run i18n:check` și `npm run check`.

Generatorul descoperă automat directorul, actualizează `MessageId` și opțiunile
de limbă și validează:

- sintaxa Fluent, ID-urile duplicate și domeniile lipsă;
- paritatea mesajelor, variabilelor și referințelor față de `en-US`;
- formatarea nevidă a fiecărui mesaj în fiecare locale;
- mesajele nefolosite în Rust/Svelte.

## Reguli pentru mesaje

- Folosește ID-uri semantice (`data-file-created`), nu propoziția engleză ca ID.
- Păstrează numele variabilelor exact ca în `en-US`.
- Folosește selectoarele Fluent pentru plural, nu concatenări în Svelte.
- Nu introduce HTML în traduceri. Componentele păstrează structura și accesibilitatea.
- Nu traduce identificatori tehnici precum `TOML`, `Rust`, `Tauri`, `Zola`,
  `load_data`, căi sau scurtături, decât dacă contextul o cere.
- Etichetele vizibile, `title`, `aria-label`, placeholder-ele și mesajele de
  stare fac parte din aceeași traducere.

Fișierele `.ftl` pot fi administrate ca resurse separate într-o platformă
precum Weblate. `manifest.json` rămâne revizuit în Git, deoarece conține
metadate de runtime, nu text obișnuit al interfeței.
