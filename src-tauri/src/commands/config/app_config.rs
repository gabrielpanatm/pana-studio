use std::{fs, path::Path};

use tauri::{AppHandle, Manager};

use crate::{
    app_home::{app_config_path, project_app_config_path, projects_config_dir},
    commands::config::model::{
        ApplicationAccentPreference, ApplicationBootProjection, ApplicationLanguagePreference,
        ApplicationPreferenceResolutionSource, ApplicationPreferenceSelections,
        ApplicationSettingsPatchInput, ApplicationSettingsSnapshot, ApplicationTheme,
        ApplicationThemePreference, EffectiveApplicationPreferences, GlobalAppConfig,
        ProjectAppConfig, ProjectAppConfigInput, APPLICATION_BOOT_PROJECTION_SCHEMA_VERSION,
        APPLICATION_SETTINGS_SCHEMA_VERSION, DEFAULT_BLOCK_PROPERTIES_HEIGHT,
        MAX_BLOCK_PROPERTIES_HEIGHT, MIN_BLOCK_PROPERTIES_HEIGHT,
    },
    kernel::write_authority::{
        WriteAuthority, WriteCategory, WriteIntent, WriteOperationKind, WriteOwner, WritePolicy,
        WriteTarget,
    },
    localization::{format_message, LocalizedDiagnostic, BASE_LOCALE},
    system_preferences::{
        locale_candidate_matches_supported, negotiate_supported_locale,
        supported_application_locale, SystemColorScheme, SystemPreferenceSource,
        SystemPreferencesRuntime, DEFAULT_APPLICATION_ACCENT,
    },
};

pub(super) fn read_application_settings(
    app: &AppHandle,
) -> Result<ApplicationSettingsSnapshot, LocalizedDiagnostic> {
    let config = read_global_app_config(app)?;
    Ok(application_settings_snapshot(app, &config))
}

pub(super) fn write_application_settings(
    app: &AppHandle,
    input: ApplicationSettingsPatchInput,
) -> Result<ApplicationSettingsSnapshot, LocalizedDiagnostic> {
    let mut config = read_global_app_config(app)?;
    if input.expected_revision != config.revision {
        return Err(
            LocalizedDiagnostic::new("diagnostic-application-settings-stale")
                .with_argument("expected", input.expected_revision)
                .with_argument("actual", config.revision),
        );
    }
    let current_preferences = application_preference_selections(&config);
    let mut next_preferences = current_preferences.clone();
    if let Some(language) = input.patch.language {
        validate_language_preference(&language)?;
        next_preferences.language = language;
    }
    if let Some(theme) = input.patch.theme {
        next_preferences.theme = theme;
    }
    if let Some(accent) = input.patch.accent {
        next_preferences.accent = normalize_accent_preference(accent)?;
    }
    let current_height = config
        .block_properties_height
        .unwrap_or(DEFAULT_BLOCK_PROPERTIES_HEIGHT)
        .clamp(MIN_BLOCK_PROPERTIES_HEIGHT, MAX_BLOCK_PROPERTIES_HEIGHT);
    let next_height = input
        .patch
        .block_properties_height
        .map(|height| height.clamp(MIN_BLOCK_PROPERTIES_HEIGHT, MAX_BLOCK_PROPERTIES_HEIGHT))
        .unwrap_or(current_height);
    let current_collapsed = config.block_properties_collapsed.unwrap_or(false);
    let next_collapsed = input
        .patch
        .block_properties_collapsed
        .unwrap_or(current_collapsed);
    if current_preferences == next_preferences
        && current_height == next_height
        && current_collapsed == next_collapsed
    {
        return Ok(application_settings_snapshot(app, &config));
    }

    config.revision = config.revision.saturating_add(1);
    config.version = 3;
    config.legacy_theme = None;
    config.language_preference = Some(next_preferences.language);
    config.theme_preference = Some(next_preferences.theme);
    config.accent_preference = Some(next_preferences.accent);
    config.block_properties_height = Some(next_height);
    config.block_properties_collapsed = Some(next_collapsed);
    let body = serde_json::to_string_pretty(&config).map_err(|error| {
        settings_diagnostic("diagnostic-application-settings-save-failed", error)
    })?;
    let path = app_config_path(app).map_err(|error| {
        settings_diagnostic("diagnostic-application-settings-save-failed", error)
    })?;
    let boundary = path
        .parent()
        .ok_or_else(|| {
            settings_diagnostic(
                "diagnostic-application-settings-save-failed",
                "application config has no parent directory",
            )
        })?
        .to_path_buf();
    write_internal_config(
        app,
        path,
        boundary,
        "config/config.json",
        "Scriere setări globale Pană Studio",
        format!("{body}\n"),
    )
    .map_err(|error| settings_diagnostic("diagnostic-application-settings-save-failed", error))?;
    Ok(application_settings_snapshot(app, &config))
}

fn read_global_app_config(app: &AppHandle) -> Result<GlobalAppConfig, LocalizedDiagnostic> {
    let path = app_config_path(app).map_err(|error| {
        settings_diagnostic("diagnostic-application-settings-load-failed", error)
    })?;
    if !path.exists() {
        return Ok(GlobalAppConfig::default());
    }
    let source = fs::read_to_string(&path).map_err(|error| {
        settings_diagnostic("diagnostic-application-settings-load-failed", error)
    })?;
    serde_json::from_str(&source)
        .map_err(|error| settings_diagnostic("diagnostic-application-settings-load-failed", error))
}

fn settings_diagnostic(
    code: &'static str,
    technical_error: impl std::fmt::Display,
) -> LocalizedDiagnostic {
    eprintln!("[Pană Studio] {code}: {technical_error}");
    LocalizedDiagnostic::new(code)
}

fn application_settings_snapshot(
    app: &AppHandle,
    config: &GlobalAppConfig,
) -> ApplicationSettingsSnapshot {
    let preferences = application_preference_selections(config);
    let system = app.state::<SystemPreferencesRuntime>().snapshot();
    let effective = effective_application_preferences(&preferences, &system);
    let boot = application_boot_projection(config.revision, &effective, &system);
    ApplicationSettingsSnapshot {
        schema_version: APPLICATION_SETTINGS_SCHEMA_VERSION,
        revision: config.revision,
        brand_accent: DEFAULT_APPLICATION_ACCENT.to_string(),
        preferences,
        effective,
        system,
        boot,
        block_properties_height: config
            .block_properties_height
            .unwrap_or(DEFAULT_BLOCK_PROPERTIES_HEIGHT)
            .clamp(MIN_BLOCK_PROPERTIES_HEIGHT, MAX_BLOCK_PROPERTIES_HEIGHT),
        block_properties_collapsed: config.block_properties_collapsed.unwrap_or(false),
    }
}

fn application_boot_projection(
    settings_revision: u64,
    effective: &EffectiveApplicationPreferences,
    system: &crate::system_preferences::SystemPreferencesSnapshot,
) -> ApplicationBootProjection {
    ApplicationBootProjection {
        schema_version: APPLICATION_BOOT_PROJECTION_SCHEMA_VERSION,
        authority: "rust_application_settings",
        settings_schema_version: APPLICATION_SETTINGS_SCHEMA_VERSION,
        settings_revision,
        system_generation: system.generation,
        locale: effective.locale.clone(),
        direction: effective.direction.clone(),
        theme: effective.theme,
        accent: effective.accent.clone(),
        contrast: system.contrast,
        reduced_motion: system.reduced_motion,
        loading_label: localized_boot_message(&effective.locale, "application-loading-label"),
        loading_subtitle: localized_boot_message(&effective.locale, "application-loading-subtitle"),
    }
}

fn localized_boot_message(locale: &str, id: &str) -> String {
    format_message(locale, id, None)
        .or_else(|primary_error| {
            eprintln!("[Pană Studio] Boot locale {locale}/{id}: {primary_error}");
            format_message(BASE_LOCALE, id, None)
        })
        .unwrap_or_else(|fallback_error| {
            eprintln!("[Pană Studio] Boot fallback {BASE_LOCALE}/{id}: {fallback_error}");
            "Pană Studio".to_string()
        })
}

fn application_preference_selections(config: &GlobalAppConfig) -> ApplicationPreferenceSelections {
    ApplicationPreferenceSelections {
        language: config.language_preference.clone().unwrap_or_default(),
        theme: config.theme_preference.clone().unwrap_or_else(|| {
            config
                .legacy_theme
                .map(|value| ApplicationThemePreference::Fixed { value })
                .unwrap_or_default()
        }),
        accent: config.accent_preference.clone().unwrap_or_default(),
    }
}

fn effective_application_preferences(
    preferences: &ApplicationPreferenceSelections,
    system: &crate::system_preferences::SystemPreferencesSnapshot,
) -> EffectiveApplicationPreferences {
    let fixed_locale = match &preferences.language {
        ApplicationLanguagePreference::System => None,
        ApplicationLanguagePreference::Fixed { value } => Some(value.as_str()),
    };
    let locale = negotiate_supported_locale(fixed_locale, &system.locale_candidates);
    let language_source = if fixed_locale.is_some() {
        ApplicationPreferenceResolutionSource::Fixed
    } else if system
        .locale_candidates
        .iter()
        .any(|candidate| locale_candidate_matches_supported(candidate, &locale))
    {
        ApplicationPreferenceResolutionSource::PosixLocale
    } else {
        ApplicationPreferenceResolutionSource::Fallback
    };
    let (theme, theme_source) = match preferences.theme {
        ApplicationThemePreference::Fixed { value } => {
            (value, ApplicationPreferenceResolutionSource::Fixed)
        }
        ApplicationThemePreference::System => match system.color_scheme {
            Some(SystemColorScheme::Light) => (
                ApplicationTheme::Light,
                resolution_source(system.color_scheme_source),
            ),
            Some(SystemColorScheme::Dark) => (
                ApplicationTheme::Dark,
                resolution_source(system.color_scheme_source),
            ),
            None => (
                ApplicationTheme::Dark,
                ApplicationPreferenceResolutionSource::Fallback,
            ),
        },
    };
    let (accent, accent_source) = match &preferences.accent {
        ApplicationAccentPreference::Fixed { value } => {
            (value.clone(), ApplicationPreferenceResolutionSource::Fixed)
        }
        ApplicationAccentPreference::Brand => (
            DEFAULT_APPLICATION_ACCENT.to_string(),
            ApplicationPreferenceResolutionSource::Fixed,
        ),
        ApplicationAccentPreference::System => system.accent.map_or_else(
            || {
                (
                    DEFAULT_APPLICATION_ACCENT.to_string(),
                    ApplicationPreferenceResolutionSource::Fallback,
                )
            },
            |accent| (accent.css_hex(), resolution_source(system.accent_source)),
        ),
    };
    EffectiveApplicationPreferences {
        direction: locale_direction(&locale).to_string(),
        locale,
        theme,
        accent,
        language_source,
        theme_source,
        accent_source,
    }
}

fn resolution_source(source: SystemPreferenceSource) -> ApplicationPreferenceResolutionSource {
    match source {
        SystemPreferenceSource::XdgPortal => ApplicationPreferenceResolutionSource::XdgPortal,
        SystemPreferenceSource::TauriWindow => ApplicationPreferenceResolutionSource::TauriWindow,
        SystemPreferenceSource::PosixLocale => ApplicationPreferenceResolutionSource::PosixLocale,
        SystemPreferenceSource::Fallback | SystemPreferenceSource::Unavailable => {
            ApplicationPreferenceResolutionSource::Fallback
        }
    }
}

fn locale_direction(locale: &str) -> &'static str {
    crate::localization::embedded_locale(locale)
        .map(|catalog| catalog.direction)
        .unwrap_or("ltr")
}

fn validate_language_preference(
    preference: &ApplicationLanguagePreference,
) -> Result<(), LocalizedDiagnostic> {
    if let ApplicationLanguagePreference::Fixed { value } = preference {
        if !supported_application_locale(value) {
            return Err(LocalizedDiagnostic::new(
                "diagnostic-application-settings-invalid-language",
            )
            .with_argument("locale", value.clone()));
        }
    }
    Ok(())
}

fn normalize_accent_preference(
    preference: ApplicationAccentPreference,
) -> Result<ApplicationAccentPreference, LocalizedDiagnostic> {
    let ApplicationAccentPreference::Fixed { value } = preference else {
        return Ok(preference);
    };
    let value = value.trim().to_ascii_lowercase();
    let valid = value.len() == 7
        && value.starts_with('#')
        && value[1..].bytes().all(|byte| byte.is_ascii_hexdigit());
    if !valid {
        return Err(LocalizedDiagnostic::new(
            "diagnostic-application-settings-invalid-accent",
        ));
    }
    Ok(ApplicationAccentPreference::Fixed { value })
}

pub(crate) fn read_project_app_config_for_root(
    app: &AppHandle,
    root: &Path,
) -> Result<ProjectAppConfig, String> {
    let project_path = canonical_project_path(root);
    let path = project_app_config_path(app, &project_path)?;
    if !path.exists() {
        return Ok(default_project_app_config(project_path));
    }

    let source = fs::read_to_string(&path)
        .map_err(|e| format!("Nu am putut citi configurația locală Pană Studio: {}", e))?;
    let mut config: ProjectAppConfig = serde_json::from_str(&source)
        .map_err(|e| format!("Configurația locală Pană Studio este invalidă: {}", e))?;
    config.project_path = project_path;
    Ok(config)
}

pub(super) fn project_app_config_from_input(
    root: &Path,
    config: ProjectAppConfigInput,
) -> ProjectAppConfig {
    ProjectAppConfig {
        project_path: canonical_project_path(root),
        cachebust_assets: config.cachebust_assets,
    }
}

pub(super) fn write_project_app_config_for_root(
    app: &AppHandle,
    _root: &Path,
    stored: ProjectAppConfig,
) -> Result<ProjectAppConfig, String> {
    let project_path = stored.project_path.clone();
    let global_path = app_config_path(app)?;
    if !global_path.exists() {
        let global = serde_json::to_string_pretty(&GlobalAppConfig::default())
            .map_err(|e| format!("Nu am putut serializa config-ul Pană Studio: {}", e))?;
        let boundary = global_path
            .parent()
            .ok_or_else(|| "Config-ul Pană Studio nu are folder părinte.".to_string())?
            .to_path_buf();
        write_internal_config(
            app,
            global_path,
            boundary,
            "config/config.json",
            "Scriere config global Pană Studio",
            format!("{}\n", global),
        )?;
    }

    let body = serde_json::to_string_pretty(&stored)
        .map_err(|e| format!("Nu am putut serializa config-ul proiectului: {}", e))?;
    let projects_root = projects_config_dir(app)?;
    let project_config_path = project_app_config_path(app, &project_path)?;
    let project_config_label = format!(
        "config/projects/{}",
        project_config_path
            .file_name()
            .and_then(|file_name| file_name.to_str())
            .unwrap_or("project.json")
    );
    write_internal_config(
        app,
        project_config_path,
        projects_root,
        project_config_label,
        "Scriere config local proiect Pană Studio",
        format!("{}\n", body),
    )?;
    Ok(stored)
}

fn default_project_app_config(project_path: String) -> ProjectAppConfig {
    ProjectAppConfig {
        project_path,
        cachebust_assets: false,
    }
}

pub(super) fn write_internal_config(
    app: &AppHandle,
    path: impl Into<std::path::PathBuf>,
    boundary: impl Into<std::path::PathBuf>,
    public_label: impl Into<String>,
    description: impl Into<String>,
    contents: String,
) -> Result<(), String> {
    let target = WriteTarget::new(path, boundary, public_label);
    let intent = WriteIntent::new(
        WriteCategory::InternalAppWrite,
        WriteOwner::AppConfig,
        WriteOperationKind::WriteText,
        target,
        WritePolicy::internal_atomic(),
        description,
    );
    WriteAuthority::new(app)
        .write_text(intent, &contents)
        .map_err(|error| error.into_terminal_diagnostic())
        .map(|_| ())
}

fn canonical_project_path(root: &Path) -> String {
    fs::canonicalize(root)
        .unwrap_or_else(|_| root.to_path_buf())
        .to_string_lossy()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system_preferences::{
        SystemAccentColor, SystemContrast, SystemPreferencesSnapshot,
        SYSTEM_PREFERENCES_SCHEMA_VERSION,
    };

    fn system_snapshot() -> SystemPreferencesSnapshot {
        SystemPreferencesSnapshot {
            schema_version: SYSTEM_PREFERENCES_SCHEMA_VERSION,
            generation: 7,
            locale_candidates: vec!["ro-RO".to_string()],
            locale_source: SystemPreferenceSource::PosixLocale,
            color_scheme: Some(SystemColorScheme::Light),
            color_scheme_source: SystemPreferenceSource::XdgPortal,
            accent: Some(SystemAccentColor {
                red: 12,
                green: 34,
                blue: 56,
            }),
            accent_source: SystemPreferenceSource::XdgPortal,
            contrast: Some(SystemContrast::High),
            contrast_source: SystemPreferenceSource::XdgPortal,
            reduced_motion: Some(true),
            reduced_motion_source: SystemPreferenceSource::XdgPortal,
            portal_available: true,
        }
    }

    #[test]
    fn legacy_explicit_theme_migrates_as_fixed_but_missing_preferences_use_system() {
        let legacy = GlobalAppConfig {
            legacy_theme: Some(ApplicationTheme::Light),
            ..GlobalAppConfig::default()
        };
        let migrated = application_preference_selections(&legacy);
        assert_eq!(
            migrated.theme,
            ApplicationThemePreference::Fixed {
                value: ApplicationTheme::Light
            }
        );
        assert_eq!(migrated.language, ApplicationLanguagePreference::System);
        assert_eq!(migrated.accent, ApplicationAccentPreference::System);
    }

    #[test]
    fn system_preferences_resolve_locale_theme_and_accent_with_sources() {
        let effective = effective_application_preferences(
            &ApplicationPreferenceSelections {
                language: ApplicationLanguagePreference::System,
                theme: ApplicationThemePreference::System,
                accent: ApplicationAccentPreference::System,
            },
            &system_snapshot(),
        );
        assert_eq!(effective.locale, "ro");
        assert_eq!(effective.direction, "ltr");
        assert_eq!(effective.theme, ApplicationTheme::Light);
        assert_eq!(effective.accent, "#0c2238");
        assert_eq!(
            effective.language_source,
            ApplicationPreferenceResolutionSource::PosixLocale
        );
        assert_eq!(
            effective.theme_source,
            ApplicationPreferenceResolutionSource::XdgPortal
        );
        assert_eq!(
            effective.accent_source,
            ApplicationPreferenceResolutionSource::XdgPortal
        );
    }

    #[test]
    fn fixed_preferences_override_system_and_invalid_accent_is_structured() {
        let effective = effective_application_preferences(
            &ApplicationPreferenceSelections {
                language: ApplicationLanguagePreference::Fixed {
                    value: "en-US".to_string(),
                },
                theme: ApplicationThemePreference::Fixed {
                    value: ApplicationTheme::Dark,
                },
                accent: ApplicationAccentPreference::Brand,
            },
            &system_snapshot(),
        );
        assert_eq!(effective.locale, "en-US");
        assert_eq!(effective.theme, ApplicationTheme::Dark);
        assert_eq!(effective.accent, DEFAULT_APPLICATION_ACCENT);
        assert_eq!(
            effective.language_source,
            ApplicationPreferenceResolutionSource::Fixed
        );
        let error = normalize_accent_preference(ApplicationAccentPreference::Fixed {
            value: "blue".to_string(),
        })
        .expect_err("invalid accent");
        assert_eq!(error.code, "diagnostic-application-settings-invalid-accent");
    }

    #[test]
    fn boot_projection_uses_effective_rust_preferences_and_fluent_copy() {
        let system = system_snapshot();
        let effective = effective_application_preferences(
            &ApplicationPreferenceSelections {
                language: ApplicationLanguagePreference::Fixed {
                    value: "ro".to_string(),
                },
                theme: ApplicationThemePreference::Fixed {
                    value: ApplicationTheme::Dark,
                },
                accent: ApplicationAccentPreference::Fixed {
                    value: "#c2410c".to_string(),
                },
            },
            &system,
        );
        let boot = application_boot_projection(7, &effective, &system);

        assert_eq!(boot.schema_version, 1);
        assert_eq!(boot.authority, "rust_application_settings");
        assert_eq!(boot.settings_revision, 7);
        assert_eq!(boot.system_generation, system.generation);
        assert_eq!(boot.locale, "ro");
        assert_eq!(boot.theme, ApplicationTheme::Dark);
        assert_eq!(boot.accent, "#c2410c");
        assert_eq!(boot.loading_label, "Pană Studio se încarcă");
        assert_eq!(boot.loading_subtitle, "Se pregătește editorul vizual");
    }

    #[test]
    fn direction_is_resolved_from_the_embedded_locale_manifest() {
        for locale in crate::localization::available_locale_ids() {
            let manifest_direction = crate::localization::embedded_locale(locale)
                .expect("embedded locale")
                .direction;
            assert_eq!(locale_direction(locale), manifest_direction);
        }
        assert_eq!(locale_direction("unsupported"), "ltr");
    }
}
