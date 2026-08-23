use std::{
    error::Error,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

pub const PROPRIETAR: &str = "index-zero-generator-stres-v1";
pub const MARKER: &str = ".index-zero-generator.json";

const IESIRI_DIRECTOARE: &[&str] = &[
    "sursa/content/evenimente",
    "sursa/content/artisti",
    "sursa/content/locatii",
    "sursa/content/jurnal",
    "sursa/templates/generat",
    "sursa/static/imagini/stres-media",
    "materiale/margine-disk",
];

const IESIRI_FISIERE: &[&str] = &[
    "sursa/date/program-generat.toml",
    "sursa/sass/pagini/laborator-css-generat.scss",
    "manifest-stres.toml",
];

const DIRECTOARE_IGNORATE: &[&str] = &[
    ".git",
    ".panastudio",
    ".zola-cache",
    ".svelte-kit",
    "build",
    "export",
    "node_modules",
    "package",
    "public",
    "target",
];

#[derive(Clone, Debug)]
pub struct Inventar {
    pub fisiere: usize,
    pub directoare: usize,
    pub text_bytes: u64,
    pub max_text_bytes: u64,
    pub max_text_path: String,
}

pub fn radacina_proiect() -> Result<PathBuf, Box<dyn Error>> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let radacina = manifest
        .parent()
        .and_then(Path::parent)
        .ok_or("Nu am putut rezolva rădăcina proiectului din CARGO_MANIFEST_DIR.")?;
    Ok(radacina.canonicalize()?)
}

pub fn verifica_identitatea_proiectului(radacina: &Path) -> Result<(), Box<dyn Error>> {
    let agenti = fs::read_to_string(radacina.join("AGENTS.md"))?;
    if !agenti.contains("INDEX ZERO") || !agenti.contains("generator Rust determinist") {
        return Err("Generatorul a refuzat un director care nu este proiectul INDEX ZERO.".into());
    }
    Ok(())
}

pub fn pregateste_iesirile_detinute(radacina: &Path) -> Result<(), Box<dyn Error>> {
    let marker = radacina.join(MARKER);
    let exista_iesiri = IESIRI_DIRECTOARE
        .iter()
        .chain(IESIRI_FISIERE.iter())
        .any(|cale| radacina.join(cale).exists());

    if exista_iesiri {
        let continut = fs::read_to_string(&marker).map_err(|_| {
            "Există ieșiri generate fără markerul de proprietate; regenerarea a fost refuzată."
        })?;
        if !continut.contains(PROPRIETAR) {
            return Err("Markerul generatorului are un proprietar necunoscut.".into());
        }
    }

    for cale in IESIRI_DIRECTOARE {
        elimina_director_explicit(radacina, cale)?;
    }
    for cale in IESIRI_FISIERE {
        elimina_fisier_explicit(radacina, cale)?;
    }
    Ok(())
}

pub fn scrie(
    radacina: &Path,
    cale_relativa: impl AsRef<Path>,
    continut: impl AsRef<[u8]>,
) -> Result<(), Box<dyn Error>> {
    let cale_relativa = cale_relativa.as_ref();
    if cale_relativa.is_absolute()
        || cale_relativa
            .components()
            .any(|componenta| matches!(componenta, std::path::Component::ParentDir))
    {
        return Err(format!("Cale de scriere nesigură: {}", cale_relativa.display()).into());
    }
    let destinatie = radacina.join(cale_relativa);
    if let Some(parinte) = destinatie.parent() {
        fs::create_dir_all(parinte)?;
    }
    if destinatie
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(format!("Generatorul a refuzat symlink-ul {}.", destinatie.display()).into());
    }
    fs::write(destinatie, continut)?;
    Ok(())
}

pub fn inventar_proiect(radacina: &Path) -> Result<Inventar, Box<dyn Error>> {
    let mut inventar = Inventar {
        fisiere: 0,
        directoare: 0,
        text_bytes: 0,
        max_text_bytes: 0,
        max_text_path: String::new(),
    };
    inventariaza(radacina, radacina, &mut inventar)?;
    Ok(inventar)
}

pub fn fisiere_text(radacina: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut rezultate = Vec::new();
    colecteaza_fisiere_text(radacina, radacina, &mut rezultate)?;
    rezultate.sort();
    Ok(rezultate)
}

fn inventariaza(
    radacina: &Path,
    director: &Path,
    inventar: &mut Inventar,
) -> Result<(), Box<dyn Error>> {
    let mut intrari = fs::read_dir(director)?.collect::<Result<Vec<_>, _>>()?;
    intrari.sort_by_key(|intrare| intrare.file_name());
    for intrare in intrari {
        let tip = intrare.file_type()?;
        let cale = intrare.path();
        if tip.is_symlink() {
            continue;
        }
        if tip.is_dir() {
            if director_ignorat(&intrare.file_name()) {
                continue;
            }
            inventar.directoare += 1;
            inventariaza(radacina, &cale, inventar)?;
        } else if tip.is_file() {
            inventar.fisiere += 1;
            if este_text(&cale) {
                let bytes = intrare.metadata()?.len();
                inventar.text_bytes = inventar.text_bytes.saturating_add(bytes);
                if bytes > inventar.max_text_bytes {
                    inventar.max_text_bytes = bytes;
                    inventar.max_text_path = cale
                        .strip_prefix(radacina)?
                        .to_string_lossy()
                        .replace('\\', "/");
                }
            }
        }
    }
    Ok(())
}

fn colecteaza_fisiere_text(
    radacina: &Path,
    director: &Path,
    rezultate: &mut Vec<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    let mut intrari = fs::read_dir(director)?.collect::<Result<Vec<_>, _>>()?;
    intrari.sort_by_key(|intrare| intrare.file_name());
    for intrare in intrari {
        let tip = intrare.file_type()?;
        let cale = intrare.path();
        if tip.is_symlink() {
            continue;
        }
        if tip.is_dir() {
            if !director_ignorat(&intrare.file_name()) {
                colecteaza_fisiere_text(radacina, &cale, rezultate)?;
            }
        } else if tip.is_file() && este_text(&cale) {
            rezultate.push(cale);
        }
    }
    Ok(())
}

fn director_ignorat(nume: &OsStr) -> bool {
    nume.to_str()
        .is_some_and(|nume| DIRECTOARE_IGNORATE.contains(&nume))
}

fn este_text(cale: &Path) -> bool {
    cale.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extensie| {
            matches!(
                extensie,
                "css" | "html" | "js" | "json" | "md" | "rs" | "scss" | "toml" | "txt"
            )
        })
}

fn elimina_director_explicit(radacina: &Path, relativ: &str) -> Result<(), Box<dyn Error>> {
    let cale = radacina.join(relativ);
    if !cale.exists() {
        return Ok(());
    }
    if cale.symlink_metadata()?.file_type().is_symlink() {
        return Err(format!("Refuz ștergerea symlink-ului {}.", cale.display()).into());
    }
    fs::remove_dir_all(cale)?;
    Ok(())
}

fn elimina_fisier_explicit(radacina: &Path, relativ: &str) -> Result<(), Box<dyn Error>> {
    let cale = radacina.join(relativ);
    if !cale.exists() {
        return Ok(());
    }
    if cale.symlink_metadata()?.file_type().is_symlink() {
        return Err(format!("Refuz ștergerea symlink-ului {}.", cale.display()).into());
    }
    fs::remove_file(cale)?;
    Ok(())
}
