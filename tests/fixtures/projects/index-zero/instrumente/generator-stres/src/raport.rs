use std::{error::Error, fs, path::Path};

use crate::{
    fisiere::{self, Inventar, MARKER, PROPRIETAR},
    profil::Specificatie,
};

const MAX_FISIER_TEXT: u64 = 2 * 1024 * 1024;
const MAX_TEXT_TOTAL: u64 = 24 * 1024 * 1024;

pub fn scrie_marker_provizoriu(radacina: &Path, spec: Specificatie) -> Result<(), Box<dyn Error>> {
    fisiere::scrie(
        radacina,
        MARKER,
        format!(
            "{{\n  \"schemaVersion\": 1,\n  \"owner\": \"{PROPRIETAR}\",\n  \"profile\": \"{}\",\n  \"state\": \"generating\"\n}}\n",
            spec.profil.label(),
        ),
    )
}

pub fn completeaza_marginea_disk(
    radacina: &Path,
    spec: Specificatie,
) -> Result<(), Box<dyn Error>> {
    let Some(tinta) = spec.tinta_fisiere else {
        return Ok(());
    };
    let inventar = fisiere::inventar_proiect(radacina)?;
    if inventar.fisiere > tinta {
        return Err(format!(
            "Proiectul de bază are {} fișiere și depășește ținta {tinta}.",
            inventar.fisiere
        )
        .into());
    }
    for index in 0..(tinta - inventar.fisiere) {
        fisiere::scrie(
            radacina,
            format!("materiale/margine-disk/intrare-{index:04}.txt"),
            format!(
                "INDEX ZERO / {} / intrare deterministă {index:04}\n",
                spec.profil.label()
            ),
        )?;
    }
    Ok(())
}

pub fn scrie_raport(
    radacina: &Path,
    spec: Specificatie,
    inventar: &Inventar,
) -> Result<(), Box<dyn Error>> {
    let continut = format!(
        "schema_version = 1\n\
owner = \"{PROPRIETAR}\"\n\
profil = \"{}\"\n\
seed = 20270821\n\
\n\
[continut]\n\
evenimente = {}\n\
artisti = {}\n\
locatii = {}\n\
articole = {}\n\
\n\
[stres]\n\
celule_dom = {}\n\
noduri_dom_aproximate = {}\n\
reguli_css = {}\n\
elemente_motion = {}\n\
\n\
[inventar]\n\
fisiere = {}\n\
directoare = {}\n\
intrari = {}\n\
text_bytes = {}\n\
max_text_bytes = {}\n\
max_text_path = \"{}\"\n",
        spec.profil.label(),
        spec.evenimente,
        spec.artisti,
        spec.locatii,
        spec.articole,
        spec.celule_dom,
        spec.celule_dom * 4,
        spec.reguli_css,
        spec.elemente_motion,
        inventar.fisiere,
        inventar.directoare,
        inventar.fisiere + inventar.directoare,
        inventar.text_bytes,
        inventar.max_text_bytes,
        inventar.max_text_path,
    );
    fisiere::scrie(radacina, "manifest-stres.toml", continut)
}

pub fn scrie_marker(
    radacina: &Path,
    spec: Specificatie,
    inventar: &Inventar,
) -> Result<(), Box<dyn Error>> {
    fisiere::scrie(
        radacina,
        MARKER,
        format!(
            "{{\n  \"schemaVersion\": 1,\n  \"owner\": \"{PROPRIETAR}\",\n  \"profile\": \"{}\",\n  \"state\": \"complete\",\n  \"expectedFiles\": {},\n  \"expectedDirectories\": {}\n}}\n",
            spec.profil.label(),
            inventar.fisiere,
            inventar.directoare,
        ),
    )
}

pub fn verifica(radacina: &Path, spec: Specificatie) -> Result<(), Box<dyn Error>> {
    let marker = fs::read_to_string(radacina.join(MARKER))?;
    if !marker.contains(PROPRIETAR)
        || !marker.contains(spec.profil.label())
        || !marker.contains("\"state\": \"complete\"")
    {
        return Err("Markerul nu corespunde profilului cerut sau este incomplet.".into());
    }

    let inventar = fisiere::inventar_proiect(radacina)?;
    if let Some(tinta) = spec.tinta_fisiere {
        if inventar.fisiere != tinta {
            return Err(format!(
                "Profilul {} cere {tinta} fișiere, dar inventarul are {}.",
                spec.profil.label(),
                inventar.fisiere
            )
            .into());
        }
    }
    if inventar.max_text_bytes > MAX_FISIER_TEXT {
        return Err(format!(
            "Fișierul {} are {} bytes și depășește 2 MiB.",
            inventar.max_text_path, inventar.max_text_bytes
        )
        .into());
    }
    if inventar.text_bytes > MAX_TEXT_TOTAL {
        return Err(format!(
            "Sursele text au {} bytes și depășesc 24 MiB.",
            inventar.text_bytes
        )
        .into());
    }
    for cale in fisiere::fisiere_text(&radacina.join("sursa"))? {
        let continut = fs::read_to_string(&cale)?;
        if continut.to_ascii_lowercase().contains("lorem ipsum") {
            return Err(format!("Conținut placeholder detectat în {}.", cale.display()).into());
        }
    }
    if spec.profil.label() == "mare" && inventar.fisiere + inventar.directoare >= 500 {
        return Err(
            "Profilul mare trebuie să rămână sub 500 de intrări urmărite (fișiere + directoare)."
                .into(),
        );
    }
    println!(
        "[index-zero] verificare profil={} status=ok fisiere={} text_bytes={}",
        spec.profil.label(),
        inventar.fisiere,
        inventar.text_bytes,
    );
    Ok(())
}
