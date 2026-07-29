use std::{
    collections::BTreeSet,
    fs,
    path::Path,
    sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use fluent_langneg::LanguageIdentifier;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Theme};

pub const SYSTEM_PREFERENCES_SCHEMA_VERSION: u32 = 1;
pub const SYSTEM_PREFERENCES_CHANGED_EVENT: &str = "system-preferences://changed";
pub const DEFAULT_APPLICATION_LOCALE: &str = crate::localization::BASE_LOCALE;
pub const DEFAULT_APPLICATION_ACCENT: &str = "#1d7f6a";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemPreferenceSource {
    XdgPortal,
    TauriWindow,
    PosixLocale,
    Fallback,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemColorScheme {
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemContrast {
    Normal,
    High,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemAccentColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl SystemAccentColor {
    pub fn css_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.red, self.green, self.blue)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemPreferencesSnapshot {
    pub schema_version: u32,
    pub generation: u64,
    pub locale_candidates: Vec<String>,
    pub locale_source: SystemPreferenceSource,
    pub color_scheme: Option<SystemColorScheme>,
    pub color_scheme_source: SystemPreferenceSource,
    pub accent: Option<SystemAccentColor>,
    pub accent_source: SystemPreferenceSource,
    pub contrast: Option<SystemContrast>,
    pub contrast_source: SystemPreferenceSource,
    pub reduced_motion: Option<bool>,
    pub reduced_motion_source: SystemPreferenceSource,
    pub portal_available: bool,
}

#[derive(Clone, Debug)]
struct SystemPreferencesState {
    snapshot: SystemPreferencesSnapshot,
    portal_color_scheme: Option<SystemColorScheme>,
    tauri_color_scheme: Option<SystemColorScheme>,
}

pub struct SystemPreferencesRuntime {
    state: RwLock<SystemPreferencesState>,
}

impl Default for SystemPreferencesRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemPreferencesRuntime {
    pub fn new() -> Self {
        let locale_candidates = detect_locale_candidates();
        let locale_source = if locale_candidates.is_empty() {
            SystemPreferenceSource::Fallback
        } else {
            SystemPreferenceSource::PosixLocale
        };
        Self {
            state: RwLock::new(SystemPreferencesState {
                snapshot: SystemPreferencesSnapshot {
                    schema_version: SYSTEM_PREFERENCES_SCHEMA_VERSION,
                    generation: 1,
                    locale_candidates,
                    locale_source,
                    color_scheme: None,
                    color_scheme_source: SystemPreferenceSource::Unavailable,
                    accent: None,
                    accent_source: SystemPreferenceSource::Fallback,
                    contrast: None,
                    contrast_source: SystemPreferenceSource::Unavailable,
                    reduced_motion: None,
                    reduced_motion_source: SystemPreferenceSource::Unavailable,
                    portal_available: false,
                },
                portal_color_scheme: None,
                tauri_color_scheme: None,
            }),
        }
    }

    pub fn snapshot(&self) -> SystemPreferencesSnapshot {
        self.read().snapshot.clone()
    }

    pub fn refresh_locale_candidates(&self) -> Option<SystemPreferencesSnapshot> {
        let locale_candidates = detect_locale_candidates();
        let locale_source = if locale_candidates.is_empty() {
            SystemPreferenceSource::Fallback
        } else {
            SystemPreferenceSource::PosixLocale
        };
        let mut state = self.write();
        if state.snapshot.locale_candidates == locale_candidates
            && state.snapshot.locale_source == locale_source
        {
            return None;
        }
        state.snapshot.locale_candidates = locale_candidates;
        state.snapshot.locale_source = locale_source;
        Some(increment_generation(&mut state))
    }

    pub fn set_tauri_theme(&self, theme: Theme) -> Option<SystemPreferencesSnapshot> {
        let theme = match theme {
            Theme::Light => SystemColorScheme::Light,
            Theme::Dark => SystemColorScheme::Dark,
            _ => return None,
        };
        let mut state = self.write();
        if state.tauri_color_scheme == Some(theme) {
            return None;
        }
        state.tauri_color_scheme = Some(theme);
        if state.portal_color_scheme.is_none() {
            state.snapshot.color_scheme = Some(theme);
            state.snapshot.color_scheme_source = SystemPreferenceSource::TauriWindow;
        }
        Some(increment_generation(&mut state))
    }

    fn set_portal_state(
        &self,
        available: bool,
        color_scheme: Option<SystemColorScheme>,
        accent: Option<SystemAccentColor>,
        contrast: Option<SystemContrast>,
        reduced_motion: Option<bool>,
    ) -> Option<SystemPreferencesSnapshot> {
        let mut state = self.write();
        let resolved_color_scheme = color_scheme.or(state.tauri_color_scheme);
        let resolved_color_source = if color_scheme.is_some() {
            SystemPreferenceSource::XdgPortal
        } else if state.tauri_color_scheme.is_some() {
            SystemPreferenceSource::TauriWindow
        } else {
            SystemPreferenceSource::Unavailable
        };
        let accent_source = if accent.is_some() {
            SystemPreferenceSource::XdgPortal
        } else {
            SystemPreferenceSource::Fallback
        };
        let contrast_source = if contrast.is_some() {
            SystemPreferenceSource::XdgPortal
        } else {
            SystemPreferenceSource::Unavailable
        };
        let reduced_motion_source = if reduced_motion.is_some() {
            SystemPreferenceSource::XdgPortal
        } else {
            SystemPreferenceSource::Unavailable
        };
        let unchanged = state.snapshot.portal_available == available
            && state.portal_color_scheme == color_scheme
            && state.snapshot.color_scheme == resolved_color_scheme
            && state.snapshot.color_scheme_source == resolved_color_source
            && state.snapshot.accent == accent
            && state.snapshot.accent_source == accent_source
            && state.snapshot.contrast == contrast
            && state.snapshot.contrast_source == contrast_source
            && state.snapshot.reduced_motion == reduced_motion
            && state.snapshot.reduced_motion_source == reduced_motion_source;
        if unchanged {
            return None;
        }
        state.snapshot.portal_available = available;
        state.portal_color_scheme = color_scheme;
        state.snapshot.color_scheme = resolved_color_scheme;
        state.snapshot.color_scheme_source = resolved_color_source;
        state.snapshot.accent = accent;
        state.snapshot.accent_source = accent_source;
        state.snapshot.contrast = contrast;
        state.snapshot.contrast_source = contrast_source;
        state.snapshot.reduced_motion = reduced_motion;
        state.snapshot.reduced_motion_source = reduced_motion_source;
        Some(increment_generation(&mut state))
    }

    fn read(&self) -> RwLockReadGuard<'_, SystemPreferencesState> {
        self.state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, SystemPreferencesState> {
        self.state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn increment_generation(state: &mut SystemPreferencesState) -> SystemPreferencesSnapshot {
    state.snapshot.generation = state.snapshot.generation.saturating_add(1);
    state.snapshot.clone()
}

pub fn publish_system_preferences(app: &AppHandle, snapshot: SystemPreferencesSnapshot) {
    if let Err(error) = app.emit(SYSTEM_PREFERENCES_CHANGED_EVENT, snapshot) {
        eprintln!("[Pană Studio] SystemPreferences event failed: {error}");
    }
}

pub fn refresh_system_locale(app: &AppHandle) {
    if let Some(snapshot) = app
        .state::<SystemPreferencesRuntime>()
        .refresh_locale_candidates()
    {
        publish_system_preferences(app, snapshot);
    }
}

pub fn update_tauri_window_theme(app: &AppHandle, theme: Theme) {
    if let Some(snapshot) = app
        .state::<SystemPreferencesRuntime>()
        .set_tauri_theme(theme)
    {
        publish_system_preferences(app, snapshot);
    }
}

pub fn negotiate_supported_locale(
    fixed_locale: Option<&str>,
    system_candidates: &[String],
) -> String {
    use fluent_langneg::{negotiate_languages, NegotiationStrategy};

    let available = crate::localization::available_locale_ids()
        .into_iter()
        .filter_map(|locale| locale.parse::<LanguageIdentifier>().ok())
        .collect::<Vec<_>>();
    let default = DEFAULT_APPLICATION_LOCALE
        .parse::<LanguageIdentifier>()
        .expect("DEFAULT_APPLICATION_LOCALE must be a valid language identifier");
    let requested = fixed_locale
        .into_iter()
        .chain(system_candidates.iter().map(String::as_str))
        .filter_map(|locale| normalize_locale_candidate(locale))
        .collect::<Vec<_>>();
    negotiate_languages(
        &requested,
        &available,
        Some(&default),
        NegotiationStrategy::Filtering,
    )
    .first()
    .map(|locale| locale.to_string())
    .unwrap_or_else(|| DEFAULT_APPLICATION_LOCALE.to_string())
}

pub fn supported_application_locale(locale: &str) -> bool {
    normalize_locale_candidate(locale).is_some_and(|candidate| {
        crate::localization::available_locale_ids()
            .into_iter()
            .any(|available| candidate.to_string() == available)
    })
}

pub fn locale_candidate_matches_supported(candidate: &str, effective: &str) -> bool {
    let Some(candidate) = normalize_locale_candidate(candidate) else {
        return false;
    };
    let Some(effective) = normalize_locale_candidate(effective) else {
        return false;
    };
    supported_application_locale(&effective.to_string()) && candidate.language == effective.language
}

fn detect_locale_candidates() -> Vec<String> {
    let mut candidates = Vec::new();
    let mut seen = BTreeSet::new();
    for locale in sys_locale::get_locales() {
        push_locale_candidate(&mut candidates, &mut seen, &locale);
    }
    if candidates.is_empty() {
        #[cfg(target_os = "linux")]
        for path in ["/etc/locale.conf", "/etc/default/locale"] {
            for locale in read_locale_file(Path::new(path)) {
                push_locale_candidate(&mut candidates, &mut seen, &locale);
            }
        }
    }
    candidates
}

fn push_locale_candidate(candidates: &mut Vec<String>, seen: &mut BTreeSet<String>, locale: &str) {
    let Some(locale) = normalize_locale_candidate(locale) else {
        return;
    };
    let canonical = locale.to_string();
    let uniqueness_key = canonical.to_ascii_lowercase();
    if seen.insert(uniqueness_key) {
        candidates.push(canonical);
    }
}

fn normalize_locale_candidate(locale: &str) -> Option<LanguageIdentifier> {
    let locale = locale.trim().trim_matches('"').trim_matches('\'');
    let locale = locale
        .split_once('.')
        .map(|(head, _)| head)
        .unwrap_or(locale);
    let locale = locale
        .split_once('@')
        .map(|(head, _)| head)
        .unwrap_or(locale)
        .replace('_', "-");
    if locale.is_empty() || locale.eq_ignore_ascii_case("c") || locale.eq_ignore_ascii_case("posix")
    {
        return None;
    }
    locale.parse::<LanguageIdentifier>().ok()
}

#[cfg(target_os = "linux")]
fn read_locale_file(path: &Path) -> Vec<String> {
    let Ok(source) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut values = Vec::new();
    for key in ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
        for line in source.lines() {
            let Some((candidate_key, value)) = line.split_once('=') else {
                continue;
            };
            if candidate_key.trim() != key {
                continue;
            }
            values.extend(
                value
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .split(':')
                    .map(str::to_string),
            );
        }
    }
    values
}

#[cfg(not(target_os = "linux"))]
fn read_locale_file(_path: &Path) -> Vec<String> {
    Vec::new()
}

#[cfg(target_os = "linux")]
pub fn start_linux_system_preferences_monitor(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        use std::time::Duration;

        loop {
            let settings = match ashpd::desktop::settings::Settings::new().await {
                Ok(settings) => settings,
                Err(error) => {
                    mark_portal_unavailable(&app);
                    eprintln!("[Pană Studio] XDG Settings Portal unavailable: {error}");
                    tokio::time::sleep(Duration::from_secs(15)).await;
                    continue;
                }
            };
            refresh_portal_snapshot(&app, &settings).await;
            let mut changes = match settings.receive_setting_changed().await {
                Ok(changes) => changes,
                Err(error) => {
                    mark_portal_unavailable(&app);
                    eprintln!("[Pană Studio] XDG Settings Portal listener failed: {error}");
                    tokio::time::sleep(Duration::from_secs(15)).await;
                    continue;
                }
            };
            use futures_util::StreamExt;
            while changes.next().await.is_some() {
                refresh_portal_snapshot(&app, &settings).await;
            }
            mark_portal_unavailable(&app);
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
}

#[cfg(not(target_os = "linux"))]
pub fn start_linux_system_preferences_monitor(_app: AppHandle) {}

#[cfg(target_os = "linux")]
async fn refresh_portal_snapshot(app: &AppHandle, settings: &ashpd::desktop::settings::Settings) {
    use ashpd::desktop::settings::{ColorScheme, Contrast, ReducedMotion};

    let (color_scheme, accent, contrast, reduced_motion) = tokio::join!(
        settings.color_scheme(),
        settings.accent_color(),
        settings.contrast(),
        settings.reduced_motion(),
    );
    let color_scheme = color_scheme.ok().and_then(|value| match value {
        ColorScheme::PreferDark => Some(SystemColorScheme::Dark),
        ColorScheme::PreferLight => Some(SystemColorScheme::Light),
        ColorScheme::NoPreference => None,
    });
    let accent = accent.ok().and_then(|value| {
        Some(SystemAccentColor {
            red: normalized_channel(value.red())?,
            green: normalized_channel(value.green())?,
            blue: normalized_channel(value.blue())?,
        })
    });
    let contrast = contrast.ok().map(|value| match value {
        Contrast::High => SystemContrast::High,
        Contrast::NoPreference => SystemContrast::Normal,
    });
    let reduced_motion = reduced_motion
        .ok()
        .map(|value| matches!(value, ReducedMotion::ReducedMotion));
    if let Some(snapshot) = app.state::<SystemPreferencesRuntime>().set_portal_state(
        true,
        color_scheme,
        accent,
        contrast,
        reduced_motion,
    ) {
        publish_system_preferences(app, snapshot);
    }
}

#[cfg(target_os = "linux")]
fn mark_portal_unavailable(app: &AppHandle) {
    if let Some(snapshot) = app
        .state::<SystemPreferencesRuntime>()
        .set_portal_state(false, None, None, None, None)
    {
        publish_system_preferences(app, snapshot);
    }
}

#[cfg(target_os = "linux")]
fn normalized_channel(value: f64) -> Option<u8> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return None;
    }
    Some((value * 255.0).round() as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_normalization_rejects_posix_sentinels_and_canonicalizes_tags() {
        assert_eq!(
            normalize_locale_candidate("ro_RO.UTF-8").map(|locale| locale.to_string()),
            Some("ro-RO".to_string())
        );
        assert_eq!(normalize_locale_candidate("C.UTF-8"), None);
        assert_eq!(normalize_locale_candidate("POSIX"), None);
        assert_eq!(normalize_locale_candidate("../ro"), None);
    }

    #[test]
    fn locale_negotiation_uses_region_fallback_and_default_locale() {
        assert_eq!(
            negotiate_supported_locale(None, &["ro-RO".to_string()]),
            "ro"
        );
        assert_eq!(
            negotiate_supported_locale(None, &["de-DE".to_string()]),
            "en-US"
        );
        assert_eq!(
            negotiate_supported_locale(Some("en-US"), &["ro-RO".to_string()]),
            "en-US"
        );
    }

    #[test]
    fn tauri_theme_is_fallback_but_portal_has_priority() {
        let runtime = SystemPreferencesRuntime::new();
        runtime.set_tauri_theme(Theme::Dark);
        assert_eq!(
            runtime.snapshot().color_scheme_source,
            SystemPreferenceSource::TauriWindow
        );
        runtime.set_portal_state(
            true,
            Some(SystemColorScheme::Light),
            None,
            Some(SystemContrast::Normal),
            Some(false),
        );
        assert_eq!(
            runtime.snapshot().color_scheme,
            Some(SystemColorScheme::Light)
        );
        assert_eq!(
            runtime.snapshot().color_scheme_source,
            SystemPreferenceSource::XdgPortal
        );
        runtime.set_portal_state(false, None, None, None, None);
        assert_eq!(
            runtime.snapshot().color_scheme,
            Some(SystemColorScheme::Dark)
        );
    }

    #[test]
    fn generation_changes_only_when_observable_preference_state_changes() {
        let runtime = SystemPreferencesRuntime::new();
        let initial = runtime.snapshot().generation;
        assert!(runtime.set_tauri_theme(Theme::Dark).is_some());
        let after_dark = runtime.snapshot().generation;
        assert!(after_dark > initial);
        assert!(runtime.set_tauri_theme(Theme::Dark).is_none());
        assert_eq!(runtime.snapshot().generation, after_dark);
        runtime.set_portal_state(true, None, None, None, None);
        assert_eq!(
            runtime.snapshot().accent_source,
            SystemPreferenceSource::Fallback
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn portal_channels_reject_invalid_values() {
        assert_eq!(normalized_channel(-0.1), None);
        assert_eq!(normalized_channel(1.1), None);
        assert_eq!(normalized_channel(f64::NAN), None);
        assert_eq!(normalized_channel(0.5), Some(128));
    }
}
