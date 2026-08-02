# Contractul inspectorului Fundal

## Autoritate și proiecție

Modelul canonic este `CssBackground` din `src-tauri/src/css/background.rs`. Rust îl construiește din declarațiile regulii active și îl include în `CssRuleContext.background`. Schema contextului CSS este versiunea 2, iar schema modelului de fundal este versiunea 1.

Frontend-ul folosește `background-model.ts` numai ca proiecție compatibilă pentru editarea optimistă dintre două confirmări. Scrierea continuă să treacă prin comenzile CSS legate de `ProjectWorkspace`; validarea, selecția țintei, revizia, istoricul și persistența rămân în Rust.

La breakpoint-uri, proiecția canonică îmbină declarațiile de bază cu suprascrierile viewport-ului în ordinea cascadei. Lista `viewportRules` rămâne separată drept țintă de scriere; astfel, o suprascriere izolată precum `background-size` nu pierde straturile definite pe Desktop.

## Model

Un fundal conține:

- o singură culoare de bază, din `background-color`;
- zero sau mai multe straturi, în ordinea CSS: primul strat este deasupra;
- sursa fiecărui strat (`url`, funcție imagine, gradient sau expresie opacă);
- valorile aliniate pentru position, size, repeat, attachment, origin, clip și blend mode;
- declarația `background` compactă, dacă există;
- longhand-uri opace care pot produce liste dinamic prin CSS/SCSS.

Listele mai scurte urmează repetarea definită de CSS. Separatorul recunoaște paranteze, paranteze drepte, șiruri cu escape, comentarii și interpolări SCSS, astfel încât virgulele din funcții sau URL-uri nu creează straturi false.

Declarația compactă este păstrată în modul brut. O valoare validă, dar nereprezentabilă complet, nu este înlocuită cu un fallback. O expresie care poate genera o listă la compilare rămâne în `opaqueProperties`; reordonarea structurală este oprită până când lista devine explicită.

## Gradient

`CssGradient` păstrează tipul linear/radial/conic, caracterul repeating, preambulul specific tipului, stopurile, stopurile cu două poziții, reperele de tranziție și elementele opace. Pozițiile își păstrează unitatea originală. Expresiile dinamice ambigue rămân opace în loc să fie interpretate distructiv.

`GradientEditor.svelte` oferă proiecția locală, iar `ColorInput` rămâne unica piesă pentru culoarea și transparența stopului. Drag-ul rampei emite numai drafturi live; pointer-up produce commitul.

Stopurile nu sunt reordonate implicit când pozițiile lor se intersectează: ordinea scrisă în CSS rămâne stabilă, inclusiv pentru stopuri suprapuse sau poziții descrescătoare. Toate controalele rămân accesibile prin tastatură, iar stopul activ este ridicat numai vizual deasupra celorlalte, fără să modifice ordinea serializată.

## Atomicitatea mutațiilor

`CssPropertyEditController` expune operații pentru un set de proprietăți. O adăugare, ștergere, duplicare sau reordonare serializează longhand-urile într-un singur set și este introdusă o singură dată în coada mutației CSS. Astfel, Undo/Redo restaurează configurația completă într-o singură etapă. Editarea unui singur atribut al unui strat scrie doar longhand-ul afectat, pentru a nu normaliza proprietăți independente.

## Compatibilitate

Editorul vechi cu un singur `color | image | gradient` nu mai face parte din suprafața de producție. Culoarea textului aparține secțiunii Tipografie. Valorile existente sunt citite direct din longhand-urile standard și nu necesită migrare pe disc.
