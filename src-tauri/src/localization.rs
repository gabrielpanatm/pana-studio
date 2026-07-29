use std::collections::BTreeMap;

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use serde::{Deserialize, Serialize};
use unic_langid::LanguageIdentifier;

pub const BASE_LOCALE: &str = "en-US";

pub struct EmbeddedFluentResource {
    pub domain: &'static str,
    pub source: &'static str,
}

pub struct EmbeddedLocale {
    pub locale: &'static str,
    pub native_name: &'static str,
    pub direction: &'static str,
    pub contributors: &'static [&'static str],
    pub resources: &'static [EmbeddedFluentResource],
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizedDiagnostic {
    pub schema_version: u32,
    pub code: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub arguments: BTreeMap<String, serde_json::Value>,
}

impl LocalizedDiagnostic {
    pub fn new(code: impl Into<String>) -> Self {
        Self {
            schema_version: 1,
            code: code.into(),
            arguments: BTreeMap::new(),
        }
    }

    pub fn with_argument(
        mut self,
        name: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.arguments.insert(name.into(), value.into());
        self
    }
}

include!(concat!(env!("OUT_DIR"), "/pana-studio-locales.rs"));

pub fn available_locale_ids() -> Vec<&'static str> {
    EMBEDDED_LOCALES
        .iter()
        .map(|locale| locale.locale)
        .collect()
}

pub fn embedded_locale(locale: &str) -> Option<&'static EmbeddedLocale> {
    EMBEDDED_LOCALES
        .iter()
        .find(|candidate| candidate.locale.eq_ignore_ascii_case(locale))
}

pub fn validate_embedded_catalogs() -> Result<(), String> {
    if embedded_locale(BASE_LOCALE).is_none() {
        return Err(format!("Locale de bază {BASE_LOCALE} lipsește."));
    }
    for locale in EMBEDDED_LOCALES {
        if locale.native_name.trim().is_empty() {
            return Err(format!("Locale {} nu are nume nativ.", locale.locale));
        }
        if !matches!(locale.direction, "ltr" | "rtl") {
            return Err(format!(
                "Locale {} are direcția invalidă {}.",
                locale.locale, locale.direction
            ));
        }
        if locale.contributors.is_empty() {
            return Err(format!("Locale {} nu are contributori.", locale.locale));
        }
        format_message(locale.locale, "common-loading", None)?;
    }
    Ok(())
}

pub fn format_message(
    locale: &str,
    id: &str,
    arguments: Option<&FluentArgs<'_>>,
) -> Result<String, String> {
    let selected = embedded_locale(locale)
        .or_else(|| embedded_locale(BASE_LOCALE))
        .ok_or_else(|| "Catalogul Fluent de bază lipsește din aplicație.".to_string())?;
    match format_from_locale(selected, id, arguments) {
        Ok(message) => Ok(message),
        Err(primary_error) if selected.locale != BASE_LOCALE => {
            let fallback = embedded_locale(BASE_LOCALE)
                .ok_or_else(|| "Catalogul Fluent de bază lipsește din aplicație.".to_string())?;
            format_from_locale(fallback, id, arguments)
                .map_err(|fallback_error| format!("{primary_error}; fallback: {fallback_error}"))
        }
        Err(error) => Err(error),
    }
}

fn format_from_locale(
    locale: &EmbeddedLocale,
    id: &str,
    arguments: Option<&FluentArgs<'_>>,
) -> Result<String, String> {
    let language = locale
        .locale
        .parse::<LanguageIdentifier>()
        .map_err(|error| format!("Locale Fluent invalid {}: {error}", locale.locale))?;
    let mut bundle = FluentBundle::new(vec![language]);
    for resource in locale.resources {
        let fluent_resource = FluentResource::try_new(resource.source.to_string()).map_err(
            |(_resource, errors)| {
                format!(
                    "Resursa Fluent {}/{} este invalidă: {errors:?}",
                    locale.locale, resource.domain
                )
            },
        )?;
        bundle.add_resource(fluent_resource).map_err(|errors| {
            format!(
                "Resursa Fluent {}/{} intră în conflict: {errors:?}",
                locale.locale, resource.domain
            )
        })?;
    }
    let message = bundle
        .get_message(id)
        .ok_or_else(|| format!("Mesajul Fluent {id} lipsește din locale {}", locale.locale))?;
    let pattern = message.value().ok_or_else(|| {
        format!(
            "Mesajul Fluent {id} nu are valoare în locale {}",
            locale.locale
        )
    })?;
    let mut errors = Vec::new();
    let formatted = bundle
        .format_pattern(pattern, arguments, &mut errors)
        .into_owned();
    if errors.is_empty() {
        Ok(formatted)
    } else {
        Err(format!(
            "Mesajul Fluent {id} nu a putut fi formatat în {}: {errors:?}",
            locale.locale
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_catalog_discovers_locales_without_a_rust_registry() {
        assert!(embedded_locale("en-US").is_some());
        assert!(embedded_locale("ro").is_some());
        assert_eq!(embedded_locale("ro").unwrap().direction, "ltr");
        assert!(!embedded_locale("ro").unwrap().contributors.is_empty());
    }

    #[test]
    fn rust_formats_the_same_fluent_resources_as_the_frontend() {
        assert_eq!(
            format_message("ro", "settings-language-title", None).unwrap(),
            "Limba interfeței"
        );
        assert_eq!(
            format_message("en-US", "settings-language-title", None).unwrap(),
            "Interface language"
        );
    }

    #[test]
    fn unknown_locale_falls_back_without_exposing_a_raw_message_id() {
        assert_eq!(
            format_message("de-DE", "common-loading", None).unwrap(),
            "Loading…"
        );
    }
}
