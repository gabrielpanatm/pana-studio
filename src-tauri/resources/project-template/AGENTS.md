# [Nume Proiect] — Context Claude

> Actualizează acest fișier pe parcursul proiectului. Este sursa unică de adevăr pentru orice sesiune nouă.

---

## Proiect

- **Client:** [Nume / Firmă]
- **URL final:** [https://...]
- **Tip:** Zola
- **Temă:** [ ] Light  |  [ ] Dark
- **Deadline:** [Data]
- **Dosar proiect:** `[dosarul curent de lucru sau path-ul confirmat de Gabriel]`

## Obiectiv principal

[Ce trebuie să facă site-ul — 1-2 fraze. Ex: "Să genereze lead-uri pentru serviciile de contabilitate ale clientului."]

## Audiență țintă

[Cine sunt utilizatorii. Ex: "Antreprenori IMM din București, 30-50 ani, caută un contabil."]

---

## Stack ales

- **Framework:** Zola
- **Fonturi:** [Font heading] + [Font body]
- **Deploy:** [Bunny CDN / alt]
- **Analytics:** [Umami / alt / fără]

---

## Pană Studio AI context

Fișierele locale Pană Studio sunt numai descriptori de diagnostic și lifecycle;
nu sunt sursa autoritară a contextului UI. Pe Linux le găsești implicit în:

`${XDG_CONFIG_HOME:-~/.config}/com.gabriel.panastudio/mcp/current-context.json`

Discovery/config local:

`${XDG_CONFIG_HOME:-~/.config}/com.gabriel.panastudio/mcp/mcp.json`

Contextul canonic există în RAM și se citește prin serverul MCP local
autentificat, disponibil cât timp Pană Studio este deschis:

`http://127.0.0.1:48731/mcp`

Configurarea din Pană Studio instalează endpointul și tokenul în configul activ
Codex (`$CODEX_HOME/config.toml`, implicit `~/.codex/config.toml`). Datele și
fișierele expuse sunt read-only. Numai operațiile de coordonare a edit lease-ului
modifică stare volatilă în RAM.

Înainte de orice editare prin filesystem:

1. Citește `get_current_context` și folosește exact `project.sessionId` și
   `project.projectRevision` observate.
2. Dacă `dirtyState.dirty` este `true`, cere utilizatorului să salveze sau să
   arunce modificările. Nu edita încă discul.
3. Solicită `request_edit_lease` și așteaptă statusul `granted`; starea
   `pending_ui_quiescence` nu acordă drept de editare.
4. Reînnoiește lease-ul înainte de TTL-ul de 120 de secunde cât timp editezi
   direct fișierele sursă din `sursa/`.
5. La final apelează `release_edit_lease`, declară fișierele așteptate și
   așteaptă reconcilierea Pană Studio înainte să consideri transferul încheiat.

Nu controla UI-ul Pană Studio ca mecanism de editare a surselor și nu modifica
fișierele fără un lease activ.

---

## Decizii luate

| Data | Decizie | Motivul |
|------|---------|---------|
| [dd.mm.yyyy] | [Decizie] | [Motiv] |

---

## Status proiect

- [ ] Brief aprobat
- [ ] Structură aprobată (`structura.md`)
- [ ] Direcție vizuală aprobată (`inspiratie/`)
- [ ] Materiale client primite (`materiale/`)
- [ ] Conținut text finalizat (`resurse/text/`)
- [ ] Setup proiect Zola (`sursa/` — `config.toml`, `content/`, `templates/`, `sass/`)
- [ ] Dezvoltare în curs
- [ ] Review final
- [ ] Deploy

---

## Next steps

1. [Primul pas concret]
2. [Al doilea pas]
