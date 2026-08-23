use std::{error::Error, fmt::Write, path::Path};

use crate::{fisiere, profil::Specificatie};

const DISCIPLINE: &[&str] = &[
    "performance",
    "sunet",
    "instalatie",
    "imagine",
    "cercetare",
    "atelier",
];
const FORMATE: &[&str] = &["live", "expozitie", "discutie", "atelier", "interventie"];
const SUBIECTE: &[&str] = &[
    "corp",
    "memorie",
    "infrastructura",
    "ecologie",
    "algoritmi",
    "comunitate",
];
const PRENUME: &[&str] = &[
    "Ada", "Alma", "Amir", "Ana", "Anca", "Ari", "Cezar", "Daria", "Doru", "Elena", "Emil", "Eva",
    "Iasmin", "Ilinca", "Iris", "Lia", "Luca", "Mara", "Matei", "Mina", "Nadia", "Nora", "Oana",
    "Petru", "Radu", "Raisa", "Sara", "Sorin", "Teo", "Victor",
];
const NUME: &[&str] = &[
    "Aldea", "Arman", "Barbu", "Botez", "Cernat", "Dima", "Dragan", "Enache", "Faur", "Ganea",
    "Iliescu", "Iordan", "Lazar", "Manea", "Marin", "Mocanu", "Nistor", "Oprea", "Pavel", "Pop",
    "Rusu", "Sava", "Serban", "Stan", "Toma", "Varga", "Voicu", "Zamfir",
];
const ORASE: &[&str] = &[
    "Timișoara",
    "Cluj",
    "București",
    "Iași",
    "Brașov",
    "Belgrad",
    "Novi Sad",
    "Sofia",
    "Budapesta",
    "Viena",
    "Praga",
    "Berlin",
    "Varșovia",
    "Ljubljana",
    "Zagreb",
];
const TITLURI_A: &[&str] = &[
    "Arhiva",
    "Corpul",
    "Ecoul",
    "Frecvența",
    "Grila",
    "Interfața",
    "Linia",
    "Memoria",
    "Orașul",
    "Protocolul",
    "Rețeaua",
    "Semnalul",
    "Terenul",
    "Urma",
    "Volumul",
];
const TITLURI_B: &[&str] = &[
    "care respiră",
    "dintre ziduri",
    "fără centru",
    "în așteptare",
    "în buclă",
    "în mișcare",
    "invizibilă",
    "pentru două corpuri",
    "sub presiune",
    "după miezul nopții",
    "de rezervă",
];

pub fn genereaza(radacina: &Path, spec: Specificatie) -> Result<(), Box<dyn Error>> {
    genereaza_evenimente(radacina, spec)?;
    genereaza_artisti(radacina, spec)?;
    genereaza_locatii(radacina, spec)?;
    genereaza_jurnal(radacina, spec)?;
    genereaza_program(radacina, spec)?;
    genereaza_densitate(radacina, spec)?;
    genereaza_motion(radacina, spec)?;
    genereaza_media(radacina, spec)?;
    genereaza_css(radacina, spec)?;
    Ok(())
}

fn genereaza_evenimente(radacina: &Path, spec: Specificatie) -> Result<(), Box<dyn Error>> {
    fisiere::scrie(
        radacina,
        "sursa/content/evenimente/_index.md",
        concat!(
            "+++\n",
            "title = \"Evenimente\"\n",
            "description = \"Arhiva completă a evenimentelor INDEX ZERO.\"\n",
            "template = \"evenimente.html\"\n",
            "page_template = \"eveniment.html\"\n",
            "sort_by = \"date\"\n",
            "+++\n",
        ),
    )?;

    for index in 0..spec.evenimente {
        let id = index + 1;
        let zi = index % 10 + 1;
        let data = 6 + zi;
        let ora = 11 + index % 12;
        let disciplina = DISCIPLINE[index % DISCIPLINE.len()];
        let format = FORMATE[(index * 3 + 1) % FORMATE.len()];
        let titlu = format!(
            "{} {}",
            TITLURI_A[index % TITLURI_A.len()],
            TITLURI_B[(index * 7 + 2) % TITLURI_B.len()]
        );
        let descriere = format!(
            "Un {} despre {}, corp și infrastructură, construit pentru ziua {} a programului INDEX ZERO.",
            format, disciplina, zi
        );
        let continut = format!(
            "+++\n\
title = \"{titlu}\"\n\
description = \"{descriere}\"\n\
date = 2027-10-{data:02}T{ora:02}:00:00+03:00\n\
weight = {id}\n\
\n\
[taxonomies]\n\
discipline = [\"{disciplina}\"]\n\
formate = [\"{format}\"]\n\
zile = [\"ziua-{zi:02}\"]\n\
\n\
[extra]\n\
id = \"eveniment-{id:03}\"\n\
ora = \"{ora:02}:00\"\n\
durata = \"{} minute\"\n\
locatie_id = \"locatie-{:02}\"\n\
artist_id = \"artist-{:03}\"\n\
imagine = \"imagini/vizual-{:02}.webp\"\n\
recomandat = {}\n\
+++\n\n\
## O situație construită în timp real\n\n\
{descriere} Publicul intră într-un circuit de gesturi, sunete și măsurători care se modifică odată cu prezența fiecărei persoane.\n\n\
## Ce vei întâlni\n\n\
Spațiul este organizat ca un instrument deschis. Nu există un singur punct corect de observație, iar durata poate fi parcursă integral sau fragmentar.\n\n\
> Fiecare corp schimbă datele pe care încearcă să le observe.\n",
            45 + (index % 5) * 15,
            index % spec.locatii + 1,
            index % spec.artisti + 1,
            index % 8 + 1,
            index < 12,
        );
        fisiere::scrie(
            radacina,
            format!("sursa/content/evenimente/eveniment-{id:03}.md"),
            continut,
        )?;
    }
    Ok(())
}

fn genereaza_artisti(radacina: &Path, spec: Specificatie) -> Result<(), Box<dyn Error>> {
    fisiere::scrie(
        radacina,
        "sursa/content/artisti/_index.md",
        concat!(
            "+++\n",
            "title = \"Artiști\"\n",
            "description = \"O sută de practici artistice fictive reunite în INDEX ZERO.\"\n",
            "template = \"artisti.html\"\n",
            "page_template = \"artist.html\"\n",
            "sort_by = \"weight\"\n",
            "+++\n",
        ),
    )?;

    for index in 0..spec.artisti {
        let id = index + 1;
        let nume = format!(
            "{} {}",
            PRENUME[index % PRENUME.len()],
            NUME[(index * 11 + index / PRENUME.len()) % NUME.len()]
        );
        let disciplina = DISCIPLINE[(index * 5 + 1) % DISCIPLINE.len()];
        let oras = ORASE[(index * 7 + 3) % ORASE.len()];
        let continut = format!(
            "+++\n\
title = \"{nume}\"\n\
description = \"Artist fictiv din {oras}, cu o practică la intersecția dintre {disciplina}, spațiu și procese colective.\"\n\
weight = {id}\n\
\n\
[extra]\n\
id = \"artist-{id:03}\"\n\
disciplina = \"{disciplina}\"\n\
oras = \"{oras}\"\n\
imagine = \"imagini/vizual-{:02}.webp\"\n\
+++\n\n\
{nume} lucrează cu materiale instabile: înregistrări de teren, obiecte recuperate și protocoale scrise împreună cu publicul. Practica urmărește felul în care o regulă aparent neutră schimbă comportamentul unui grup.\n\n\
În proiectele recente, instalația nu este tratată ca obiect finit, ci ca o situație care acumulează urme. La INDEX ZERO, această cercetare devine un sistem deschis pentru un spațiu fictiv din Timișoara.\n",
            index % 8 + 1,
        );
        fisiere::scrie(
            radacina,
            format!("sursa/content/artisti/artist-{id:03}.md"),
            continut,
        )?;
    }
    Ok(())
}

fn genereaza_locatii(radacina: &Path, spec: Specificatie) -> Result<(), Box<dyn Error>> {
    fisiere::scrie(
        radacina,
        "sursa/content/locatii/_index.md",
        concat!(
            "+++\n",
            "title = \"Spații\"\n",
            "description = \"Douăzeci de spații fictive pentru programul INDEX ZERO.\"\n",
            "template = \"locatii.html\"\n",
            "page_template = \"locatie.html\"\n",
            "sort_by = \"weight\"\n",
            "+++\n",
        ),
    )?;
    let tipuri = [
        "Hala",
        "Atelierul",
        "Depoul",
        "Cinema",
        "Laboratorul",
        "Curtea",
    ];
    let repere = [
        "Laminor", "Nord", "Bega", "Semnal", "Fabric", "Arhiva", "Zero",
    ];
    for index in 0..spec.locatii {
        let id = index + 1;
        let titlu = format!(
            "{} {}",
            tipuri[index % tipuri.len()],
            repere[(index * 3 + 1) % repere.len()]
        );
        let continut = format!(
            "+++\n\
title = \"{titlu}\"\n\
description = \"Spațiu fictiv INDEX ZERO, configurat pentru instalații, performance și întâlniri.\"\n\
weight = {id}\n\
\n\
[extra]\n\
id = \"locatie-{id:02}\"\n\
adresa = \"Strada Semnalului {id}, Timișoara\"\n\
coordonata_x = {}\n\
coordonata_y = {}\n\
accesibil = {}\n\
imagine = \"imagini/vizual-{:02}.webp\"\n\
+++\n\n\
{titlu} este o adresă imaginară construită pentru acest proiect de test. Traseul interior alternează zone de liniște, suprafețe reflectante și infrastructură tehnică expusă.\n\n\
Accesul publicului este descris clar, iar programul asociat folosește aceeași identificare stabilă în toate colecțiile generate.\n",
            12 + (index * 17) % 76,
            10 + (index * 23) % 78,
            index % 4 != 0,
            index % 8 + 1,
        );
        fisiere::scrie(
            radacina,
            format!("sursa/content/locatii/locatie-{id:02}.md"),
            continut,
        )?;
    }
    Ok(())
}

fn genereaza_jurnal(radacina: &Path, spec: Specificatie) -> Result<(), Box<dyn Error>> {
    fisiere::scrie(
        radacina,
        "sursa/content/jurnal/_index.md",
        concat!(
            "+++\n",
            "title = \"Jurnal\"\n",
            "description = \"Eseuri și conversații fictive din procesul INDEX ZERO.\"\n",
            "template = \"jurnal.html\"\n",
            "page_template = \"articol.html\"\n",
            "sort_by = \"date\"\n",
            "+++\n",
        ),
    )?;

    for index in 0..spec.articole {
        let id = index + 1;
        let subiect = SUBIECTE[(index * 5 + 2) % SUBIECTE.len()];
        let titlu = format!(
            "{} {}",
            TITLURI_A[(index * 3 + 4) % TITLURI_A.len()],
            TITLURI_B[(index * 9 + 1) % TITLURI_B.len()]
        );
        let continut = format!(
            "+++\n\
title = \"{titlu}\"\n\
description = \"Un eseu fictiv despre {subiect}, tehnologie și spațiul comun.\"\n\
date = 2027-{:02}-{:02}\n\
weight = {id}\n\
\n\
[taxonomies]\n\
subiecte = [\"{subiect}\"]\n\
\n\
[extra]\n\
autor = \"Redacția INDEX ZERO\"\n\
durata_lectura = \"{} minute\"\n\
imagine = \"imagini/vizual-{:02}.webp\"\n\
+++\n\n\
## Un sistem nu începe cu interfața\n\n\
Înainte ca o informație să devină vizibilă, ea trece prin spații, reguli și alegeri. În cazul orașului, aceste alegeri capătă forma unei străzi, a unei frecvențe sau a unui timp de așteptare.\n\n\
## Corpul ca instrument de măsură\n\n\
Datele nu înlocuiesc experiența directă. Ele pot însă descrie variații pe care memoria le comprimă. Practica artistică pune cele două forme de cunoaștere una lângă alta, fără să le oblige să coincidă.\n\n\
> O hartă devine interesantă exact acolo unde nu mai poate explica traseul.\n\n\
## Ce rămâne deschis\n\n\
Pentru INDEX ZERO, {subiect} nu este o temă decorativă, ci o metodă de lucru. Fiecare proiect păstrează o zonă în care publicul poate schimba rezultatul și poate observa costul schimbării.\n\n\
Concluzia nu închide procesul. Ea notează starea lui temporară și lasă loc următoarei intervenții.\n",
            7 + index % 5,
            1 + index % 8,
            6 + index % 7,
            1 + index % 8,
        );
        fisiere::scrie(
            radacina,
            format!("sursa/content/jurnal/articol-{id:03}.md"),
            continut,
        )?;
    }
    Ok(())
}

fn genereaza_program(radacina: &Path, spec: Specificatie) -> Result<(), Box<dyn Error>> {
    let mut sursa = String::from("# Fișier generat determinist. Nu edita manual.\n\n");
    for index in 0..spec.evenimente {
        let id = index + 1;
        let zi = index % 10 + 1;
        let ora = 11 + index % 12;
        writeln!(
            sursa,
            "[[evenimente]]\nid = \"eveniment-{id:03}\"\nzi = {zi}\nora = \"{ora:02}:00\"\ndisciplina = \"{}\"\nformat = \"{}\"\nlocatie = \"locatie-{:02}\"\nartist = \"artist-{:03}\"\n",
            DISCIPLINE[index % DISCIPLINE.len()],
            FORMATE[(index * 3 + 1) % FORMATE.len()],
            index % spec.locatii + 1,
            index % spec.artisti + 1,
        )?;
    }
    fisiere::scrie(radacina, "sursa/date/program-generat.toml", sursa)
}

fn genereaza_densitate(radacina: &Path, spec: Specificatie) -> Result<(), Box<dyn Error>> {
    let mut sursa = format!(
        "{{% extends \"base.html\" %}}\n\
{{% block title %}}Laborator densitate DOM | INDEX ZERO{{% endblock title %}}\n\
{{% block description %}}Laborator cu aproximativ {} de noduri DOM pentru testarea editorului.{{% endblock description %}}\n\
{{% block css_pagina %}}<link rel=\"stylesheet\" href=\"{{{{ get_url(path='pagini/laborator.css', cachebust=true) }}}}\"><link rel=\"stylesheet\" href=\"{{{{ get_url(path='pagini/laborator-css-generat.css', cachebust=true) }}}}\">{{% endblock css_pagina %}}\n\
{{% block content %}}\n\
<section class=\"laborator laborator-densitate\" data-profil=\"{}\">\n\
  <header class=\"laborator-antet container-larg\"><p class=\"eticheta-tehnica\">LAB / DOM</p><h1>Densitate controlată</h1><p>Acest profil publică {} celule și aproximativ {} de noduri element.</p></header>\n\
  <div class=\"matrice-stres\">\n",
        spec.celule_dom * 4,
        spec.profil.label(),
        spec.celule_dom,
        spec.celule_dom * 4,
    );
    for index in 0..spec.celule_dom {
        writeln!(
            sursa,
            "    <article class=\"celula-stres matrice-stil-{:04}\" data-index=\"{index}\"><span>{:04}</span><h2>Semnal {}</h2><p>Celulă deterministă pentru măsurarea selecției, proiecției și stilurilor.</p></article>",
            index % spec.reguli_css,
            index + 1,
            index % 97,
        )?;
    }
    sursa.push_str("  </div>\n</section>\n{% endblock content %}\n");
    fisiere::scrie(
        radacina,
        "sursa/templates/generat/laborator-densitate.html",
        sursa,
    )
}

fn genereaza_motion(radacina: &Path, spec: Specificatie) -> Result<(), Box<dyn Error>> {
    let mut sursa = format!(
        "{{% extends \"base.html\" %}}\n\
{{% block title %}}Laborator motion | INDEX ZERO{{% endblock title %}}\n\
{{% block description %}}Laborator cu {} de elemente animate, controlabil și compatibil cu prefers-reduced-motion.{{% endblock description %}}\n\
{{% block content %}}\n\
<section class=\"laborator laborator-motion\" data-profil=\"{}\">\n\
  <header class=\"laborator-antet container-larg\"><p class=\"eticheta-tehnica\">LAB / MOTION</p><h1>Mișcare sub sarcină</h1><p>{} de elemente animate exclusiv prin transform și opacity.</p><button class=\"buton buton-secundar\" type=\"button\" data-motion-toggle>Oprește animațiile</button></header>\n\
  <div class=\"camp-motion\">\n",
        spec.elemente_motion,
        spec.profil.label(),
        spec.elemente_motion,
    );
    for index in 0..spec.elemente_motion {
        writeln!(
            sursa,
            "    <span class=\"particula-motion particula-tip-{}\" data-motion-item data-index=\"{index}\">{:03}</span>",
            index % 6,
            index + 1,
        )?;
    }
    sursa.push_str("  </div>\n</section>\n{% endblock content %}\n");
    fisiere::scrie(
        radacina,
        "sursa/templates/generat/laborator-motion.html",
        sursa,
    )
}

fn genereaza_media(radacina: &Path, spec: Specificatie) -> Result<(), Box<dyn Error>> {
    const RESURSE_MEDIA: usize = 24;
    for index in 0..RESURSE_MEDIA {
        let nuanta = (index * 31 + 18) % 360;
        let accent = (nuanta + 74) % 360;
        let svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1600\" height=\"1000\" viewBox=\"0 0 1600 1000\"><defs><filter id=\"n\"><feTurbulence baseFrequency=\"0.017\" numOctaves=\"3\" seed=\"{}\"/><feColorMatrix values=\"1 0 0 0 0 0 1 0 0 0 0 0 1 0 0 0 0 0 .22 0\"/></filter><linearGradient id=\"g\" x2=\"1\" y2=\"1\"><stop stop-color=\"hsl({nuanta} 70% 20%)\"/><stop offset=\"1\" stop-color=\"hsl({accent} 88% 54%)\"/></linearGradient></defs><rect width=\"1600\" height=\"1000\" fill=\"#11100f\"/><circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"url(#g)\"/><path d=\"M0 {} L1600 {} L1600 1000 L0 1000Z\" fill=\"hsl({accent} 90% 58% / .42)\"/><rect width=\"1600\" height=\"1000\" filter=\"url(#n)\" opacity=\".7\"/><text x=\"72\" y=\"900\" fill=\"#f1eadb\" font-family=\"monospace\" font-size=\"42\">INDEX ZERO / MEDIA {:02}</text></svg>",
            index + 1,
            220 + (index * 83) % 1160,
            160 + (index * 59) % 640,
            170 + (index * 23) % 320,
            260 + (index * 41) % 430,
            540 + (index * 17) % 330,
            index + 1,
        );
        fisiere::scrie(
            radacina,
            format!(
                "sursa/static/imagini/stres-media/cadru-{:02}.svg",
                index + 1
            ),
            svg,
        )?;
    }

    let repetari = (spec.evenimente / 2).clamp(24, 120);
    let mut sursa = format!(
        "{{% extends \"base.html\" %}}\n\
{{% block title %}}Laborator media | INDEX ZERO{{% endblock title %}}\n\
{{% block description %}}Laborator cu {} de suprafețe media eager pentru presiune pe decodare și memorie.{{% endblock description %}}\n\
{{% block content %}}\n\
<section class=\"laborator laborator-media\" data-profil=\"{}\">\n\
  <header class=\"laborator-antet container-larg\"><p class=\"eticheta-tehnica\">LAB / MEDIA</p><h1>Decodare în rafală</h1><p>{} imagini SVG de 1600 × 1000, încărcate eager din 24 de resurse distincte.</p></header>\n\
  <div class=\"grila-media-stres\">\n",
        repetari,
        spec.profil.label(),
        repetari,
    );
    for index in 0..repetari {
        writeln!(
            sursa,
            "    <figure><img src=\"{{{{ get_url(path='imagini/stres-media/cadru-{:02}.svg') }}}}\" alt=\"Cadru procedural de stres {:03}\" width=\"1600\" height=\"1000\" loading=\"eager\" decoding=\"async\"><figcaption>Cadru {:03} / resursa {:02}</figcaption></figure>",
            index % RESURSE_MEDIA + 1,
            index + 1,
            index + 1,
            index % RESURSE_MEDIA + 1,
        )?;
    }
    sursa.push_str("  </div>\n</section>\n{% endblock content %}\n");
    fisiere::scrie(
        radacina,
        "sursa/templates/generat/laborator-media.html",
        sursa,
    )
}

fn genereaza_css(radacina: &Path, spec: Specificatie) -> Result<(), Box<dyn Error>> {
    let mut sursa = String::from(
        "// Fișier generat determinist. Valorile numerice sunt indecși mecanici de stres.\n\
@import '../css-framework/variabile';\n\n",
    );
    for index in 0..spec.reguli_css {
        writeln!(
            sursa,
            ".matrice-stil-{index:04} {{ --indice-stres: {index}; --coloana-stres: {}; --rand-stres: {}; }}",
            index % 12 + 1,
            index % 17 + 1,
        )?;
    }
    fisiere::scrie(
        radacina,
        "sursa/sass/pagini/laborator-css-generat.scss",
        sursa,
    )
}
