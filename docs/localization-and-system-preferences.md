# Localizare și preferințe automate de sistem

## Autoritate

Setările globale ale aplicației sunt deținute de Rust. Frontendul primește un
`ApplicationSettingsSnapshot` versionat și proiectează valorile efective; nu
decide singur limba, tema sau accentul. Fiecare selecție salvată este separată
de rezultat:

- `system` urmează sistemul de operare;
- `fixed { value }` păstrează explicit alegerea utilizatorului;
- accentul are suplimentar `brand`, pentru identitatea Pană Studio.

Scrierea folosește patch-uri și o revizie optimistă. O actualizare de layout nu
poate suprascrie accidental o schimbare recentă de limbă sau temă. Configurile
vechi cu `theme: light|dark` sunt migrate ca alegeri explicite; absența unei
alegeri devine `system`.

`SystemPreferencesRuntime` este un serviciu Rust reutilizabil. Snapshotul lui
conține o generație monotonă, candidatele de locale, schema de culoare,
accentul, contrastul, mișcarea redusă, sursa fiecărei valori și disponibilitatea
portalului. Evenimentul `system-preferences://changed` transportă generația;
frontendul recitește snapshotul autoritativ în loc să aplice direct payloadul.

Accentul efectiv este proiectat prin aceleași variabile de aplicație în chrome,
CodeMirror și paletele vizuale, este convertit într-o temă xterm și este trimis
prin protocolul controlat către iframe-ul Preview. Preview-ul validează
culoarea înainte să actualizeze variabilele sale interne; nu modifică stilurile
proiectului inspectat. Textul afișat peste accent folosește o culoare de contrast
derivată din aceeași valoare efectivă. Snapshotul expune separat și
`brandAccent`, astfel încât inclusiv mostra presetului Pană Studio vine din
contractul Rust, fără o copie de culoare întreținută în frontend.

## Linux

Pe Linux, prioritatea este:

1. XDG Desktop Portal Settings prin `ashpd`;
2. tema ferestrei raportată de Tauri, numai pentru light/dark;
3. fallbackul documentat al aplicației.

Runtimeul urmărește `SettingChanged` și se reconectează după pierderea
portalului. Nu citește nume de teme GTK sau chei private GNOME/KDE. Unele
desktopuri nu publică încă `accent-color`; în acest caz accentul Pană Studio
este folosit cu sursa `fallback`. Contrastul și `reduced-motion` sunt deja în
snapshot pentru consumatorii UI viitori.

Locale-urile POSIX sunt normalizate la BCP-47. `C`, `POSIX`, encodingul și
modificatorii sunt eliminați, iar lista este negociată prin Fluent cu limbile
descoperite din `locales/`. Deoarece Linux nu oferă un eveniment desktop
portabil pentru schimbarea locale-ului, acesta este recitit la pornire și la
reluarea aplicației.

## Boot

Fereastra nativă pornește ascunsă. Frontendul citește setările Rust, aplică
`lang`, `dir`, tema, accentul și catalogul Fluent, apoi afișează fereastra.
Rust are un fail-safe de șase secunde ca o eroare frontend să nu lase aplicația
invizibilă. Snapshotul Rust include o proiecție de boot versionată, cu textele
Fluent deja formatate. Frontendul o păstrează într-un cache local folosit numai
pentru primul paint al pornirii următoare; aplicația nu citește preferințe din
acest cache și îl suprascrie după fiecare snapshot Rust. Fail-safe-ul aplică
direct proiecția Rust curentă înainte să arate forțat o fereastră pornită lent.

La prima rulare, înainte să existe cache-ul, scriptul HTML folosește
`prefers-color-scheme` doar ca fallback de paint în fereastra ascunsă și nu
afișează text englezesc presupus. localStorage nu este autoritate pentru temă,
limbă sau accent.

## Cataloage

`locales/en-US` este sursa și fallbackul, iar `locales/ro` este traducerea
română completă. Domeniile separă responsabilitățile (`core`, `settings`,
`workbench`, `data`, `diagnostics`). `build.rs` încorporează aceleași fișiere
în binarul Rust, iar `scripts/generate-i18n-catalog.mjs` generează catalogul și
tipul `MessageId` pentru Svelte.

Erorile noi expuse de Rust trebuie să folosească `LocalizedDiagnostic` cu
`code` și `arguments`. Textul tehnic se scrie în jurnal; UI traduce codul prin
Fluent și nu afișează ID-ul brut.
