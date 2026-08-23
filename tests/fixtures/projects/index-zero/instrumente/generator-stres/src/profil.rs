use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Profil {
    Control,
    Mare,
    Densitate,
    MargineDisk,
    PesteLimita,
}

#[derive(Clone, Copy, Debug)]
pub struct Specificatie {
    pub profil: Profil,
    pub evenimente: usize,
    pub artisti: usize,
    pub locatii: usize,
    pub articole: usize,
    pub celule_dom: usize,
    pub reguli_css: usize,
    pub elemente_motion: usize,
    pub tinta_fisiere: Option<usize>,
}

#[derive(Debug)]
struct EroareProfil(String);

impl fmt::Display for EroareProfil {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for EroareProfil {}

impl Profil {
    pub fn parse(valoare: &str) -> Result<Self, Box<dyn Error>> {
        match valoare {
            "control" => Ok(Self::Control),
            "mare" => Ok(Self::Mare),
            "densitate" => Ok(Self::Densitate),
            "margine-disk" => Ok(Self::MargineDisk),
            "peste-limita" => Ok(Self::PesteLimita),
            _ => Err(Box::new(EroareProfil(format!(
                "Profil necunoscut `{valoare}`. Folosește control, mare, densitate, margine-disk sau peste-limita."
            )))),
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Control => "control",
            Self::Mare => "mare",
            Self::Densitate => "densitate",
            Self::MargineDisk => "margine-disk",
            Self::PesteLimita => "peste-limita",
        }
    }

    pub const fn specificatie(self) -> Specificatie {
        match self {
            Self::Control => Specificatie {
                profil: self,
                evenimente: 12,
                artisti: 8,
                locatii: 4,
                articole: 6,
                celule_dom: 250,
                reguli_css: 100,
                elemente_motion: 40,
                tinta_fisiere: None,
            },
            Self::Mare => Specificatie {
                profil: self,
                evenimente: 180,
                artisti: 100,
                locatii: 20,
                articole: 50,
                celule_dom: 1_250,
                reguli_css: 1_200,
                elemente_motion: 240,
                tinta_fisiere: None,
            },
            Self::Densitate => Specificatie {
                profil: self,
                evenimente: 60,
                artisti: 40,
                locatii: 12,
                articole: 20,
                celule_dom: 2_500,
                reguli_css: 2_000,
                elemente_motion: 480,
                tinta_fisiere: None,
            },
            Self::MargineDisk => Specificatie {
                profil: self,
                evenimente: 12,
                artisti: 8,
                locatii: 4,
                articole: 6,
                celule_dom: 250,
                reguli_css: 100,
                elemente_motion: 40,
                tinta_fisiere: Some(991),
            },
            Self::PesteLimita => Specificatie {
                profil: self,
                evenimente: 12,
                artisti: 8,
                locatii: 4,
                articole: 6,
                celule_dom: 250,
                reguli_css: 100,
                elemente_motion: 40,
                tinta_fisiere: Some(1_001),
            },
        }
    }
}
