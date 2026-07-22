# Versionare Git pentru site-ul Zola

## Scop

Pană Studio versionează numai site-ul efectiv. Repository-ul acceptat are
rădăcina canonică exactă în `ProjectSession.zola_root`, adică în
`proiect/`. Un repository dintr-un director părinte nu acordă autoritate
Git aplicației.

Versionarea Git este distinctă de celelalte două istorice ale aplicației:

- `ProjectWorkspace` deține drafturile și Undo/Redo din sesiunea curentă;
- Save persistă atomic revizia curentă pe disk;
- Git păstrează versiunile manuale între sesiuni.

## Domeniul implementat

Subsistemul include:

- detectarea și inițializarea repository-ului local;
- status staged/unstaged/untracked/conflict;
- stage și unstage pe fișier sau pe întreg repository-ul;
- diff textual limitat și clasificare explicită pentru fișiere binare;
- configurarea identității Git locale repository-ului;
- commit manual cu mesaj obligatoriu;
- istoric paginat pentru branch-ul curent;
- materializarea și previzualizarea izolată a unui commit;
- restaurarea întregului arbore al unui commit ca o versiune nouă;
- recovery durabil pentru o restaurare întreruptă;
- configurarea și eliminarea remote-urilor HTTPS/SSH/git fără credențiale în
  URL;
- fetch cu prune, progres, anulare și timeout;
- push explicit al branch-ului curent, fără force;
- branch-uri locale, upstream și comparație ahead/behind/diverged;
- preview de commit-uri și patch înaintea integrării;
- fast-forward sau merge explicit, fără `git pull` implicit;
- schimbarea branch-ului prin aceeași frontieră ProjectWorkspace;
- conflicte de merge cu Continue/Abort și recovery durabil.

Nu include rebase, force-push, tag-uri, submodule, worktree Git sau sincronizare
automată în fundal. Aplicația nu execută niciodată `git pull`: Pull este modelat
controlat ca Fetch urmat de analiză și o alegere explicită între fast-forward și
merge.

## Invariante de autoritate

1. Calea repository-ului nu este furnizată liber de frontend. Backendul o
   derivă din ProjectSession și cere `zola_root == project_root`.
2. `.git` trebuie să fie un director real aflat direct în rădăcina Zola. Un fișier
   `.git`, un symlink sau un gitdir extern este clasificat ca nesuportat.
3. Root-ul și directorul metadata raportate de Git trebuie să fie identice cu
   `zola_root` și `zola_root/.git`. `commondir`, object alternates, symlink-uri
   în metadata critică și include-uri de configurație externă sunt refuzate.
4. Fiecare comandă mutabilă este legată de `projectRoot`, runtime session ID,
   HEAD și status token observate de apelant.
5. Stage, unstage și commit cer un ProjectWorkspace curat și un AcceptedDisk
   complet, identic cu discul live.
6. Procesele Git sunt pornite fără shell, cu argumente fixe și cu un director
   capturat din authority-ul activ. Prompturile interactive și pagerele sunt
   dezactivate.
7. Operațiile Git locale nu rulează hook-uri din proiect.
8. Symlink-urile, gitlink-urile/submodulele și path-urile non-UTF-8 blochează
   restaurarea înainte de primul efect.
9. Configurațiile Git system/global și fișierul global de atribute sunt
   neutralizate. Pentru rețea sunt reimportate numai credential helper-ele din
   scope-ul global. Helper-ele locale, comenzile SSH/proxy, rewrite-urile URL și
   include-urile locale blochează remote-ul.
10. Atributele urmărite sau locale `filter`/`merge` fac repository-ul
    nesuportat, pentru a nu executa drivere clean/smudge/merge externe.
11. Sunt acceptate numai protocoalele `https`, `ssh` și `git`; `file`, `ext`,
    căile locale, URL-urile cu userinfo secret, query sau fragment sunt refuzate.
12. Partial clone/promisor este nesuportat, deoarece preview-ul, restaurarea și
    recovery-ul cer toate obiectele local și verificabile.

## Tokenul de stare

Snapshotul Git publică un `statusToken` SHA-256 calculat din:

- HEAD sau starea unborn;
- branch-ul simbolic ori detached HEAD;
- rezultatul brut `git status --porcelain -z`;
- identitatea repository-ului;
- formatul obiectelor și identitatea locală de commit;
- toate ref-urile locale, remote-tracking și interne Pană Studio;
- întreaga configurație Git locală, inclusiv configurația remote/upstream și
  cheile care pot modifica transportul.

Comenzile mutabile recitesc snapshotul sub mutex și refuză un token stale.
Commit-ul actualizează referința HEAD cu compare-and-swap față de OID-ul
observat.

## Commit manual

Frontendul finalizează editorii și execută Save înainte să permită stage sau
commit. Backendul verifică independent aceeași condiție.

Commit-ul este construit din index fără `git commit` și fără hook-uri:

1. `git write-tree` produce arborele exact staged;
2. `git commit-tree` creează obiectul commit cu părintele HEAD curent;
3. referința simbolică HEAD este actualizată prin `git update-ref` cu OID vechi
   așteptat;
4. snapshotul Git rezultat este recitit și returnat ca receipt.

Un eșec înainte de `update-ref` nu publică versiunea. Un eșec după publicarea
referinței este raportat ca efect comis, nu ca operație sigură pentru retry.

## Remote și autentificare

Remote-ul este inventariat înaintea fiecărei operații. El este utilizabil numai
dacă are exact un URL de fetch, exact un URL efectiv de push și refspec-ul fix
`+refs/heads/*:refs/remotes/<remote>/*`. Mirror, refspec-uri care scriu în
branch-uri locale, upload/receive-pack configurabil și transporturile externe
sunt blocate. URL-ul este validat și transmis explicit procesului Git; comanda
nu se bazează pe rezolvarea implicită a numelui remote.

Pană Studio nu cere și nu persistă token-uri, parole sau chei. HTTPS folosește
numai credential helper-ele definite în configurația globală a utilizatorului,
copiate ca valori de configurare în procesul izolat. SSH folosește executabilul
`ssh` controlat și agentul/configurația de sistem; prompturile Git/askpass sunt
dezactivate. Mediul procesului este curățat de variabilele care ar putea
redirecționa repository-ul, indexul, executabilul SSH, proxy-ul sau configurația
Git.

Fetch rulează atomic, cu refspec explicit, `--no-tags`, fără submodule și
opțional `--prune`. Push publică numai
`refs/heads/<branch-local>:refs/heads/<branch-remote>`, fără force și fără
wildcard. După un push confirmat, ref-ul remote-tracking este actualizat local
prin compare-and-swap; astfel primul push poate seta upstream fără un fetch
intermediar. Un refuz non-fast-forward cere Fetch și integrare explicită.

Operațiile de rețea sunt mutual exclusive, au limită de cinci minute, output
limitat, progres sanitizat și un token de anulare. Procesul Git și copiii săi
de transport rulează într-un grup separat, iar anularea oprește întregul grup și
este diferențiată de eroare. Anularea sau timeout-ul unui Push sunt tratate ca
rezultat remote necunoscut, deoarece serverul poate să fi primit commit-ul
înaintea opririi locale; mesajul interzice retry-ul automat și cere
Fetch/verificare.

## Sincronizare, branch-uri și integrare

Snapshotul conține branch-urile locale și remote-tracking, upstream-ul activ,
OID-urile și starea `up_to_date`, `ahead`, `behind`, `diverged`,
`upstream_missing` sau `no_upstream`. Configurarea upstream-ului cere ca ref-ul
remote-tracking să existe. Un branch local poate fi șters numai dacă nu este
activ și este deja strămoș al HEAD; interfața cere confirmarea numelui.

După Fetch, utilizatorul selectează un ref inventariat împreună cu OID-ul
observat. Backendul recitește exact ref-ul/OID-ul și calculează:

- relația dintre HEAD și țintă;
- numărul și lista limitată de commit-uri numai locale și numai în țintă;
- patch-ul incoming față de baza comună, fără textconv sau diff extern;
- dacă fast-forward și/sau merge sunt încă permise.

Fast-forward publică ținta prin CAS. Merge-ul folosește `git merge-tree` numai
pentru calcul, apoi `commit-tree` pentru un commit cu exact doi părinți: HEAD-ul
inițial și OID-ul țintă. Nu sunt executate hook-uri sau drivere merge externe.
Schimbarea branch-ului este tot o tranzacție: arborele este publicat mai întâi
prin ProjectWorkspace, iar HEAD devine simbolic către noul branch numai după
verificarea byte-cu-byte a surselor.

Înainte de prima modificare a surselor, integrarea este ancorată în
`refs/pana-studio/integrations/<transaction-id>`. Marker-ul păstrează branch-ul
inițial, HEAD, ținta, tree-ul rezultat, mesajul și lista conflictelor. Un merge
cu conflicte materializează numai tree-ul calculat și păstrează HEAD neschimbat.
Continue verifică fișierele permise și lipsa markerelor standard, creează un
commit rezolvat cu doi părinți, actualizează durabil marker-ul și abia apoi
publică HEAD. Abort restaurează arborele anterior prin Save-ul atomic existent.

La repornire, marker-ele sunt clasificate în `ready_to_finalize`,
`ready_to_rollback`, `conflict_resolution`, `cleanup_required` sau
`manual_review`. Sunt oferite numai acțiunile demonstrate sigure de HEAD,
branch, status și comparația byte-cu-byte; o divergență neașteptată păstrează
marker-ul pentru intervenție manuală.

## Restaurarea unei versiuni

Restaurarea nu folosește `checkout`, `switch` sau `reset --hard` și nu mută
HEAD înapoi. Arborele commit-ului ales este restaurat în surse, apoi este creat
un commit nou al cărui părinte este HEAD-ul de la începutul operației.

Fluxul autoritativ este:

1. preflight pe ProjectSession, ProjectWorkspace, AcceptedDisk, recovery și
   starea Git curată;
2. citirea și validarea completă a arborelui țintă;
3. calcularea setului exact de create/update/delete față de HEAD;
4. crearea anticipată a commit-ului de restaurare cu `commit-tree`; acesta are
   ca tree versiunea țintă și ca părinte HEAD-ul curent;
5. ancorarea commit-ului într-un ref durabil intern
   `refs/pana-studio/restores/<transaction-id>` înainte de prima scriere în
   surse;
6. aplicarea planului create/update/delete prin Save-ul atomic
   ProjectWorkspace și jurnalul său hot existent;
7. publicarea candidatului ProjectWorkspace și a snapshotului său de recovery,
   urmată de verificarea tuturor fișierelor relevante byte-cu-byte și fără
   urmărire de symlink-uri;
8. alinierea indexului la tree-ul țintă și actualizarea branch-ului prin
   `update-ref` compare-and-swap;
9. ștergerea ref-ului intern numai după ce HEAD publică commit-ul anticipat;
10. rescanarea surselor și revenirea Preview-ului la versiunea live.

Dacă aplicația este întreruptă, panoul clasifică marker-ul prin HEAD, statusul
Git și comparația byte-cu-byte a ambelor tree-uri. Sunt expuse numai acțiunile
demonstrate ca sigure: finalizarea commit-ului, rollback prin același Save
atomic sau curățarea marker-ului când commit-ul este deja publicat. Orice stare
divergentă intră în `manual_review`, fără acțiune automată. Un jurnal Save hot
trebuie rezolvat mai întâi de Recovery Coordinator.

## Interfață

Panoul slide „Versiuni” se deschide din topbar și este reciproc exclusiv cu
History și Settings. El conține starea repository-ului, schimbările staged și
unstaged, formularul de commit, istoricul paginat, diff-ul selectat și acțiunile
de previzualizare/restaurare. Secțiunile remote includ configurarea URL-urilor,
Fetch/Push/anulare, upstream, branch-uri și analiza integrării. Aplicația arată
explicit că nu rulează `git pull`, afișează istoricul din ambele laturi și
patch-ul țintei înainte să activeze Fast-forward sau Merge.

Panoul afișează separat `ProjectWorkspace dirty` și `Git working tree dirty`,
pentru ca un repository curat să nu ascundă drafturi care există numai în RAM.
Restaurarea cere confirmarea explicită a OID-ului scurt și explică faptul că
istoricul este păstrat: rezultatul este un commit nou, nu mutarea HEAD înapoi.
Eliminarea unui remote sau branch cere confirmarea exactă a numelui. Conflictele
și tranzacțiile întrerupte apar prioritar în Recovery, iar celelalte mutații Git
sunt blocate până la Continue, Finalize, Rollback/Abort sau Cleanup.

## Observabilitate

Kernel log diferențiază mutațiile Git publicate/blocate, pornirea și oprirea
Preview-ului istoric, restaurarea publicată, intrarea în recovery și rezoluția
explicită a recovery-ului. Operațiile remote finalizate, eșuate și anulate au
evenimente distincte; la fel integrarea publicată, conflictul și recovery-ul de
integrare. Mesajele de rețea sunt redactate înainte de UI și log. Un eșec al
sink-ului de observabilitate este raportat în stderr, dar nu transformă un efect
Git deja publicat într-o operație aparent necomisă și nu autorizează retry
automat.

## Verificare

Implementarea este completă numai după teste pentru:

- repository absent, invalid, părinte, worktree și root exact;
- repository unborn și repository cu HEAD;
- toate stările porcelain, rename și conflict;
- CAS stale pentru status, index, sesiune și HEAD;
- identitate Git lipsă sau invalidă;
- diff mare/binar și paginare istoric;
- restaurare cu add/update/delete și commit nou fără pierderea descendenților;
- coliziuni untracked, symlink, gitlink și eșecuri parțiale;
- recovery după fiecare limită de efect;
- validarea URL/refspec și blocarea helper-elor/configurațiilor de transport;
- primul push cu tracking, refuz non-fast-forward și Fetch fără tag-uri;
- timeout, anulare și sanitizarea erorilor de autentificare/rețea;
- relații same/fast-forward/local-ahead/diverged și preview-ul țintei;
- merge curat/conflict/rezolvat cu exact doi părinți;
- marker de integrare recitit după întrerupere, HEAD divergent și rollback;
- schimbarea branch-ului, inclusiv branch-uri diferite la același OID;
- registrul comenzilor Tauri, permisiunile generate și scope-ul webview-ului;
- mutual exclusion pentru panouri și blocarea operațiilor în stări dirty;
- typecheck, build Rust/TypeScript și suitele de regresie existente.
