# Politica de securitate

Pană Studio este în public preview. Folosește proiecte versionate și păstrează
backup-uri independente pentru datele importante.

## Raportarea unei vulnerabilități

Nu publica vulnerabilități exploatabile, credențiale sau date private într-un
issue public.

Folosește [GitHub Security Advisories](https://github.com/gabrielpanatm/pana-studio/security/advisories/new)
pentru un raport privat. Dacă opțiunea nu este disponibilă, contactează
maintainer-ul prin datele publice de la <https://pana.tm.ro/> și menționează
doar că dorești un canal privat pentru raportare.

Include, pe cât posibil:

- versiunea Pană Studio și distribuția Linux;
- impactul observat;
- pașii minimi de reproducere;
- dacă problema implică traversare de directoare, symlink-uri, execuție de
  comenzi, WebView/preview, Git, MCP/Codex sau credențiale Bunny;
- o sugestie de remediere, dacă există.

Nu include tokenuri sau fișiere reale de client. Folosește date demonstrative.

## Artefacte de release

Verifică întotdeauna checksum-ul SHA-256 publicat împreună cu AppImage-ul.
Versiunile preview pot fi nesemnate; acest lucru trebuie precizat explicit în
notele release-ului.
