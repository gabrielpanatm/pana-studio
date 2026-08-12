use std::collections::BTreeMap;

use serde::Serialize;

use crate::css::variables::{parse_variables_from_source, update_variable_in_source, ScssVariable};

use super::{normalize_font_family_name, FontDeliveryKind, FontFaceFamily};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FontRoleId {
    Text,
    Titles,
    Ui,
    Mono,
}

impl FontRoleId {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "text" => Ok(Self::Text),
            "titles" => Ok(Self::Titles),
            "ui" => Ok(Self::Ui),
            "mono" => Ok(Self::Mono),
            _ => Err(format!("Rol semantic de font necunoscut: {value}.")),
        }
    }

    pub fn variable_name(self) -> &'static str {
        match self {
            Self::Text => "font-primary",
            Self::Titles => "font-display",
            Self::Ui => "font-ui",
            Self::Mono => "font-mono",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Text => "Text",
            Self::Titles => "Titluri",
            Self::Ui => "Interfață",
            Self::Mono => "Monospace",
        }
    }

    fn fallback(self) -> &'static str {
        match self {
            Self::Mono => "'SF Mono', SFMono-Regular, Consolas, monospace",
            Self::Text | Self::Titles | Self::Ui => "system-ui, sans-serif",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontRoleAssignment {
    pub id: FontRoleId,
    pub label: String,
    pub variable_name: String,
    pub value: Option<String>,
    pub family: Option<String>,
    pub source_path: Option<String>,
    pub delivery: FontRoleDeliveryKind,
    pub installed: bool,
    pub assignable: bool,
    pub diagnostic: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FontRoleDeliveryKind {
    Local,
    External,
    System,
    Missing,
}

pub fn read_font_roles<'a>(
    sources: impl Iterator<Item = (&'a str, &'a str)>,
    families: &[FontFaceFamily],
) -> Vec<FontRoleAssignment> {
    let variables = collect_font_variables(sources);
    [FontRoleId::Text, FontRoleId::Titles, FontRoleId::Ui, FontRoleId::Mono]
        .into_iter()
        .map(|role| {
            let matches = variables
                .get(role.variable_name())
                .cloned()
                .unwrap_or_default();
            let variable = matches.first();
            let value = variable.map(|variable| variable.value.clone());
            let family = value.as_deref().and_then(primary_font_family);
            let graph_family = family.as_deref().and_then(|role_family| {
                families.iter().find(|candidate| {
                    normalize_font_family_name(&candidate.family)
                        == normalize_font_family_name(role_family)
                })
            });
            let delivery = match graph_family.map(|candidate| &candidate.delivery) {
                Some(FontDeliveryKind::Local) => FontRoleDeliveryKind::Local,
                Some(FontDeliveryKind::System) => FontRoleDeliveryKind::System,
                Some(FontDeliveryKind::External) => FontRoleDeliveryKind::External,
                Some(FontDeliveryKind::Missing) => FontRoleDeliveryKind::Missing,
                None if family.as_deref().is_some_and(is_known_system_family) => {
                    FontRoleDeliveryKind::System
                }
                None => FontRoleDeliveryKind::Missing,
            };
            let installed = !matches!(delivery, FontRoleDeliveryKind::Missing);
            let diagnostic = if matches.len() > 1 {
                Some(format!(
                    "${} este definit de {} ori; atribuirea este blocată până la eliminarea duplicatelor.",
                    role.variable_name(),
                    matches.len()
                ))
            } else if variable.is_none() {
                Some(format!(
                    "Tokenul autoritativ ${} lipsește din SCSS.",
                    role.variable_name()
                ))
            } else if family.is_some() && !installed {
                Some("Familia principală din token nu există în biblioteca de fonturi.".to_string())
            } else {
                None
            };
            FontRoleAssignment {
                id: role,
                label: role.label().to_string(),
                variable_name: role.variable_name().to_string(),
                value,
                family,
                source_path: variable.map(|variable| variable.file.clone()),
                delivery,
                installed,
                assignable: matches.len() == 1,
                diagnostic,
            }
        })
        .collect()
}

pub fn prepare_font_role_assignment<'a>(
    sources: impl Iterator<Item = (&'a str, &'a str)>,
    role: FontRoleId,
    family: &str,
) -> Result<(String, String, String), String> {
    let family = family.trim();
    if family.is_empty() {
        return Err("Familia atribuită rolului semantic este goală.".to_string());
    }
    let sources = sources.collect::<Vec<_>>();
    let variables = collect_font_variables(sources.iter().copied());
    let matches = variables
        .get(role.variable_name())
        .cloned()
        .unwrap_or_default();
    if matches.is_empty() {
        return Err(format!(
            "Rolul {} nu poate fi atribuit: tokenul ${} lipsește din SCSS.",
            role.label(),
            role.variable_name()
        ));
    }
    if matches.len() > 1 {
        return Err(format!(
            "Rolul {} nu poate fi atribuit: tokenul ${} este definit de {} ori.",
            role.label(),
            role.variable_name(),
            matches.len()
        ));
    }
    let variable = &matches[0];
    let stack = format!(
        "'{}', {}",
        family.replace('\\', "\\\\").replace('\'', "\\'"),
        role.fallback()
    );
    let source = sources
        .iter()
        .find_map(|(path, source)| (*path == variable.file).then_some(*source))
        .ok_or_else(|| {
            format!(
                "Font Manager nu mai găsește sursa SCSS {} pentru ${}.",
                variable.file,
                role.variable_name()
            )
        })?;
    let updated =
        update_variable_in_source(source, role.variable_name(), &stack).ok_or_else(|| {
            format!(
                "Tokenul ${} a dispărut din {} în timpul planificării.",
                role.variable_name(),
                variable.file
            )
        })?;
    Ok((variable.file.clone(), updated, stack))
}

fn collect_font_variables<'a>(
    sources: impl Iterator<Item = (&'a str, &'a str)>,
) -> BTreeMap<String, Vec<ScssVariable>> {
    let mut by_name = BTreeMap::<String, Vec<ScssVariable>>::new();
    for (path, source) in sources {
        if !path.to_ascii_lowercase().ends_with(".scss") {
            continue;
        }
        let mut variables = Vec::new();
        parse_variables_from_source(source, path, &mut variables);
        for variable in variables {
            if matches!(
                variable.name.as_str(),
                "font-primary" | "font-display" | "font-ui" | "font-mono"
            ) {
                by_name
                    .entry(variable.name.clone())
                    .or_default()
                    .push(variable);
            }
        }
    }
    by_name
}

fn primary_font_family(value: &str) -> Option<String> {
    let mut quote = None;
    let mut escaped = false;
    let mut family = String::new();
    for character in value.trim().chars() {
        if escaped {
            family.push(character);
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if matches!(character, '\'' | '"') {
            if quote == Some(character) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(character);
            } else {
                family.push(character);
            }
            continue;
        }
        if character == ',' && quote.is_none() {
            break;
        }
        family.push(character);
    }
    let family = family.trim().to_string();
    (!family.is_empty()).then_some(family)
}

fn is_known_system_family(value: &str) -> bool {
    let key = normalize_font_family_name(value)
        .chars()
        .filter(|character| character.is_alphanumeric())
        .collect::<String>();
    matches!(
        key.as_str(),
        "systemui"
            | "uisansserif"
            | "uiserif"
            | "uimonospace"
            | "sansserif"
            | "serif"
            | "monospace"
            | "cursive"
            | "fantasy"
            | "arial"
            | "helvetica"
            | "verdana"
            | "tahoma"
            | "trebuchetms"
            | "georgia"
            | "timesnewroman"
            | "couriernew"
            | "sfmono"
            | "sfmonoregular"
            | "menlo"
            | "monaco"
            | "consolas"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts::{FontCssRegistration, FontLicenseMetadata, FontOrigin};

    fn family(name: &str) -> FontFaceFamily {
        FontFaceFamily {
            id: format!("css:{}", normalize_font_family_name(name)),
            family: name.to_string(),
            directories: vec![format!(
                "static/fonturi/{}",
                normalize_font_family_name(name)
            )],
            origin: FontOrigin::Local,
            theme_name: None,
            delivery: FontDeliveryKind::Local,
            ownership: crate::fonts::FontOwnership::Managed,
            romanian_supported: Some(true),
            files: Vec::new(),
            faces: Vec::new(),
            issues: Vec::new(),
            license: FontLicenseMetadata::default(),
            registration: FontCssRegistration::default(),
        }
    }

    #[test]
    fn reads_exact_semantic_roles_and_matches_installed_family() {
        let source = "$font-primary: 'Geist', system-ui, sans-serif;\n$font-display: 'Space Grotesk', system-ui, sans-serif;\n";
        let families = vec![family("Geist"), family("Space Grotesk")];
        let roles = read_font_roles(
            [("sass/css-framework/_variabile.scss", source)].into_iter(),
            &families,
        );

        assert_eq!(roles[0].family.as_deref(), Some("Geist"));
        assert!(roles[0].installed);
        assert_eq!(roles[1].family.as_deref(), Some("Space Grotesk"));
        assert!(roles[1].installed);
        assert!(roles[2].diagnostic.as_deref().unwrap().contains("$font-ui"));
    }

    #[test]
    fn parses_quoted_family_with_spaces() {
        assert_eq!(
            primary_font_family("'Bricolage Grotesque', system-ui, sans-serif").as_deref(),
            Some("Bricolage Grotesque")
        );
    }

    #[test]
    fn system_stack_is_installed_without_requiring_a_local_asset() {
        let roles = read_font_roles(
            [(
                "sass/_variabile.scss",
                "$font-mono: 'SF Mono', Consolas, monospace;",
            )]
            .into_iter(),
            &[],
        );
        let mono = roles
            .iter()
            .find(|role| role.id == FontRoleId::Mono)
            .expect("mono role");
        assert_eq!(mono.delivery, FontRoleDeliveryKind::System);
        assert!(mono.installed);
    }
}
