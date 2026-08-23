mod continut;
mod fisiere;
mod profil;
mod raport;

use std::{env, error::Error};

use profil::Profil;

fn main() -> Result<(), Box<dyn Error>> {
    let mut argumente = env::args().skip(1);
    let actiune = argumente.next().unwrap_or_else(|| "genereaza".to_string());
    let profil = Profil::parse(argumente.next().as_deref().unwrap_or("mare"))?;
    if let Some(argument) = argumente.next() {
        return Err(format!("Argument necunoscut: {argument}").into());
    }

    let radacina = fisiere::radacina_proiect()?;
    fisiere::verifica_identitatea_proiectului(&radacina)?;

    match actiune.as_str() {
        "genereaza" => {
            let specificatie = profil.specificatie();
            fisiere::pregateste_iesirile_detinute(&radacina)?;
            continut::genereaza(&radacina, specificatie)?;
            raport::scrie_marker_provizoriu(&radacina, specificatie)?;
            let inventar_initial = fisiere::inventar_proiect(&radacina)?;
            raport::scrie_raport(&radacina, specificatie, &inventar_initial)?;
            raport::completeaza_marginea_disk(&radacina, specificatie)?;
            let inventar = fisiere::inventar_proiect(&radacina)?;
            raport::scrie_marker(&radacina, specificatie, &inventar)?;
            let inventar_final = fisiere::inventar_proiect(&radacina)?;
            raport::scrie_raport(&radacina, specificatie, &inventar_final)?;
            let inventar_validat = fisiere::inventar_proiect(&radacina)?;
            raport::verifica(&radacina, specificatie)?;
            println!(
                "[index-zero] profil={} fisiere={} directoare={} text_bytes={} max_text_bytes={}",
                specificatie.profil.label(),
                inventar_validat.fisiere,
                inventar_validat.directoare,
                inventar_validat.text_bytes,
                inventar_validat.max_text_bytes,
            );
        }
        "verifica" => raport::verifica(&radacina, profil.specificatie())?,
        _ => {
            return Err("Acțiunea trebuie să fie `genereaza` sau `verifica`."
                .to_string()
                .into())
        }
    }

    Ok(())
}
