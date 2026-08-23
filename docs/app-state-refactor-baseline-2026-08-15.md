# Baseline refactorizare AppState — 2026-08-15

Baseline-ul fix este în `app-state-refactor-baseline-2026-08-15.json`. Măsurarea
reproductibilă se rulează după un build de producție:

```bash
npm run build
npm run audit:app-state
```

Scriptul parsează clasa cu TypeScript, inventariază consumatorii Svelte și citește
graful static din manifestul Vite. El raportează separat metricile curente; fișierul
JSON rămâne referința nemodificabilă pentru comparația finală.

## Referință

- `app.svelte.ts`: 4.579 linii, 102 importuri, 133 `$state`, 39 `$derived`,
  aproximativ 272 metode și 30 Host-uri care returnează `this`;
- 27 componente consumă aproximativ 242 membri și efectuează 25 de scrieri
  directe în 16 câmpuri;
- `pana-state`: 419.681 B raw / 101.731 B gzip și este parte din bootstrap;
- cel mai mare graf inițial: 1.735.161 B raw / 470.390 B gzip.

Ținta minimă de acceptare pentru chunk este 314.760 B raw / 76.298 B gzip,
iar chunk-urile domeniilor lazy nu trebuie să facă parte din graful inițial.

## Runtime

Worktree-ul nu avea un harness care să instanțieze comportamental `AppState` sau
să delimiteze importul, construcția și stabilizarea celor 20 de efecte, deci nu
există o măsurare runtime retroactivă validă pentru snapshotul structural de mai
sus. A fost adăugată o probă exclusiv `DEV`, citită direct prin inspectorul
WebKitGTK al ferestrei Tauri. Primul milestone instrumentat, încă având toate cele
20 de efecte în `AppEffects`, este păstrat în
`app-state-runtime-baseline-2026-08-15.json`.

Pe 5 reload-uri calde cu `ignoreCache`, construcția are 19 ms p50 / 25 ms p95,
iar al doilea `requestAnimationFrame` după construcție este atins la 108 ms p50 /
125 ms p95. Încărcarea rece a modulului `app.svelte.ts` a transferat 483.735 B și
a decodat 483.435 B. Cinci loturi calde de câte 100 de comutări reactive ale
layoutului au avut p95 sub rezoluția de 1 ms a ceasului WebKit și maxim 1 ms.

Acesta este primul punct comparabil pentru măsurătorile finale, nu o rescriere a
baseline-ului structural original. Proba trebuie eliminată odată cu `AppState` și
nu trebuie să apară în bundle-ul de producție.
