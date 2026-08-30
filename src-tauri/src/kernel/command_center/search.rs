use std::collections::HashSet;

use crate::{
    kernel::{
        command_center::model::{
            CommandCenterAction, CommandCenterAppCommand, CommandCenterItem, CommandCenterItemKind,
            CommandCenterScope, CommandCenterSearchRequest, CommandCenterSearchResponse,
            COMMAND_CENTER_DEFAULT_LIMIT, COMMAND_CENTER_MAX_LIMIT, COMMAND_CENTER_MAX_QUERY_BYTES,
            COMMAND_CENTER_SCHEMA_VERSION,
        },
        workbench::{WorkbenchActivity, WorkbenchSurface},
    },
    localization::LocalizedDiagnostic,
    project_model::model::{ProjectModel, ProjectModelFileKind},
};

#[derive(Clone, Debug)]
struct Candidate {
    id: String,
    kind: CommandCenterItemKind,
    title: String,
    title_diagnostic: Option<LocalizedDiagnostic>,
    subtitle: String,
    subtitle_diagnostic: Option<LocalizedDiagnostic>,
    keywords: String,
    shortcut: Option<String>,
    enabled: bool,
    disabled_diagnostic: Option<LocalizedDiagnostic>,
    priority: i32,
    action: CommandCenterAction,
}

impl Candidate {
    fn item(self, score: i32) -> CommandCenterItem {
        let search_title = self.title;
        CommandCenterItem {
            id: self.id,
            kind: self.kind,
            title: self
                .title_diagnostic
                .is_none()
                .then(|| search_title.clone()),
            title_diagnostic: self.title_diagnostic,
            subtitle: self.subtitle_diagnostic.is_none().then_some(self.subtitle),
            subtitle_diagnostic: self.subtitle_diagnostic,
            shortcut: self.shortcut,
            enabled: self.enabled,
            disabled_diagnostic: self.disabled_diagnostic,
            score,
            action: self.action,
            search_title,
        }
    }
}

pub fn search_command_center_index(
    request: CommandCenterSearchRequest,
    project_root: Option<&str>,
    runtime_session_id: Option<&str>,
    model: Option<&ProjectModel>,
) -> Result<CommandCenterSearchResponse, LocalizedDiagnostic> {
    if request.query.len() > COMMAND_CENTER_MAX_QUERY_BYTES {
        return Err(LocalizedDiagnostic::new("command-center-query-too-long")
            .with_argument("limit", COMMAND_CENTER_MAX_QUERY_BYTES as u64));
    }
    let has_project = project_root.is_some() && runtime_session_id.is_some();
    let mut candidates = static_candidates(has_project);
    if let Some(model) = model {
        append_project_candidates(&mut candidates, model);
    }

    let normalized_query = normalize(&request.query);
    let tokens = normalized_query
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let scope = request.scope;
    let mut matched = candidates
        .into_iter()
        .filter(|candidate| scope_accepts(scope, candidate.kind))
        .filter_map(|candidate| {
            if tokens.is_empty()
                && !matches!(
                    candidate.kind,
                    CommandCenterItemKind::Command | CommandCenterItemKind::Activity
                )
            {
                return None;
            }
            let score = candidate_score(&candidate, &tokens)?;
            Some(candidate.item(score))
        })
        .collect::<Vec<_>>();
    matched.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| right.enabled.cmp(&left.enabled))
            .then_with(|| {
                left.search_title
                    .to_lowercase()
                    .cmp(&right.search_title.to_lowercase())
            })
            .then_with(|| left.id.cmp(&right.id))
    });

    let total_matches = matched.len();
    let limit = request
        .limit
        .unwrap_or(COMMAND_CENTER_DEFAULT_LIMIT)
        .clamp(1, COMMAND_CENTER_MAX_LIMIT);
    matched.truncate(limit);
    Ok(CommandCenterSearchResponse {
        schema_version: COMMAND_CENTER_SCHEMA_VERSION,
        project_root: project_root.map(str::to_string),
        runtime_session_id: runtime_session_id.map(str::to_string),
        query: request.query,
        scope,
        total_matches,
        truncated: total_matches > matched.len(),
        results: matched,
    })
}

fn static_candidates(has_project: bool) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    push_app_command(
        &mut candidates,
        "command.open_project",
        "Deschide proiect",
        "Alege root-ul unui proiect Pană Studio",
        "folder open project proiect dosar",
        Some("Ctrl+O"),
        false,
        if has_project { 610 } else { 980 },
        CommandCenterAppCommand::OpenProject,
        has_project,
    );
    push_app_command(
        &mut candidates,
        "command.close_application",
        "Închide Pană Studio",
        "Închide fereastra aplicației după verificarea sesiunii active",
        "close quit exit inchide aplicatie fereastra",
        Some("Ctrl+Q"),
        false,
        590,
        CommandCenterAppCommand::CloseApplication,
        has_project,
    );
    push_app_command(
        &mut candidates,
        "command.save",
        "Salvează proiectul",
        "Persistă atomic modificările sesiunii proiectului",
        "save write disk salveaza proiect",
        Some("Ctrl+S"),
        true,
        970,
        CommandCenterAppCommand::Save,
        has_project,
    );
    push_app_command(
        &mut candidates,
        "command.open_about",
        "Despre Pană Studio",
        "Deschide informațiile despre aplicație",
        "about despre versiune aplicatie",
        None,
        false,
        580,
        CommandCenterAppCommand::OpenAbout,
        has_project,
    );
    push_app_command(
        &mut candidates,
        "command.undo",
        "Anulează",
        "Revine la operația anterioară din sesiunea proiectului",
        "undo inapoi istoric",
        Some("Ctrl+Z"),
        true,
        940,
        CommandCenterAppCommand::Undo,
        has_project,
    );
    push_app_command(
        &mut candidates,
        "command.redo",
        "Refă",
        "Reaplică operația următoare din sesiunea proiectului",
        "redo inainte istoric",
        Some("Ctrl+Shift+Z"),
        true,
        930,
        CommandCenterAppCommand::Redo,
        has_project,
    );
    push_app_command(
        &mut candidates,
        "command.validate",
        "Validează proiectul Zola",
        "Rulează verificarea structurală Zola",
        "check validate zola audit verifica validare",
        None,
        true,
        900,
        CommandCenterAppCommand::Validate,
        has_project,
    );
    push_app_command(
        &mut candidates,
        "command.run_external",
        "Deschide site-ul în browser",
        "Rulează site-ul complet în browserul extern",
        "run preview browser extern site",
        None,
        true,
        890,
        CommandCenterAppCommand::RunExternal,
        has_project,
    );
    push_app_command(
        &mut candidates,
        "command.refresh_session",
        "Reîncarcă sesiunea",
        "Reproiectează starea sesiunii Rust în interfață",
        "refresh reload session sesiune reincarca",
        None,
        true,
        760,
        CommandCenterAppCommand::RefreshSession,
        has_project,
    );
    push_app_command(
        &mut candidates,
        "command.rescan_project",
        "Rescanează proiectul",
        "Actualizează structura proiectului acceptat",
        "rescan scan fisiere proiect actualizeaza",
        None,
        true,
        750,
        CommandCenterAppCommand::RescanProject,
        has_project,
    );
    push_app_command(
        &mut candidates,
        "command.close_project",
        "Închide proiectul",
        "Închide ProjectSession după verificarea modificărilor",
        "close inchide proiect sesiune",
        None,
        true,
        620,
        CommandCenterAppCommand::CloseProject,
        has_project,
    );

    for (id, title, subtitle, keywords, shortcut, priority, command) in [
        (
            "command.toggle_terminal",
            "Comută terminalul",
            "Arată sau ascunde panoul Terminal",
            "terminal bottom panel panou",
            Some("Ctrl+Backtick"),
            860,
            CommandCenterAppCommand::ToggleTerminal,
        ),
        (
            "command.show_problems",
            "Arată problemele",
            "Deschide diagnosticele proiectului în panoul inferior",
            "problems diagnostics errors warnings probleme",
            Some("Ctrl+Shift+M"),
            850,
            CommandCenterAppCommand::ShowProblems,
        ),
        (
            "command.show_output",
            "Arată jurnalul",
            "Deschide logul operațional proiectat de kernel-ul Rust",
            "output log observability kernel rust",
            None,
            845,
            CommandCenterAppCommand::ShowOutput,
        ),
        (
            "command.show_timeline",
            "Arată cronologia",
            "Deschide editorul Motion în panoul inferior",
            "timeline motion animation anime js",
            None,
            840,
            CommandCenterAppCommand::ShowTimeline,
        ),
        (
            "command.split_vertical",
            "Vizual + Cod alăturat",
            "Deschide același document în două suprafețe sincronizate",
            "split vertical columns vizual code cod alaturat",
            Some("Ctrl+Backslash"),
            835,
            CommandCenterAppCommand::SplitVertical,
        ),
        (
            "command.split_horizontal",
            "Vizual + Cod stivuit",
            "Așază suprafața Cod sub preview-ul vizual",
            "split horizontal rows vizual code cod stivuit",
            None,
            830,
            CommandCenterAppCommand::SplitHorizontal,
        ),
        (
            "command.close_split",
            "Închide vizualizarea divizată",
            "Revine la o singură suprafață pentru documentul activ",
            "close split collapse inchide editor",
            None,
            825,
            CommandCenterAppCommand::CloseSplit,
        ),
        (
            "command.canvas_fit",
            "Canvas: Potrivire",
            "Potrivește previzualizarea în spațiul disponibil",
            "canvas viewport responsive fit fluid",
            None,
            820,
            CommandCenterAppCommand::CanvasFit,
        ),
        (
            "command.canvas_desktop",
            "Canvas: Desktop 1440",
            "Fixează viewport-ul vizual la 1440px",
            "canvas viewport responsive desktop 1440",
            None,
            815,
            CommandCenterAppCommand::CanvasDesktop,
        ),
        (
            "command.canvas_tablet",
            "Canvas: Tabletă 768",
            "Fixează viewport-ul vizual la 768px",
            "canvas viewport responsive tablet 768",
            None,
            810,
            CommandCenterAppCommand::CanvasTablet,
        ),
        (
            "command.canvas_mobile",
            "Canvas: Telefon 390",
            "Fixează viewport-ul vizual la 390px",
            "canvas viewport responsive mobile telefon 390",
            None,
            805,
            CommandCenterAppCommand::CanvasMobile,
        ),
        (
            "command.toggle_left_sidebar",
            "Comută bara laterală",
            "Arată sau ascunde navigarea activității",
            "sidebar left panel lateral stanga",
            Some("Ctrl+B"),
            800,
            CommandCenterAppCommand::ToggleLeftSidebar,
        ),
        (
            "command.toggle_inspector",
            "Comută Inspectorul",
            "Arată sau ascunde Inspectorul contextual",
            "inspector right panel dreapta",
            None,
            790,
            CommandCenterAppCommand::ToggleInspector,
        ),
        (
            "command.toggle_theme",
            "Schimbă tema",
            "Comută între tema luminoasă și întunecată",
            "theme light dark tema lumina intunecat",
            None,
            600,
            CommandCenterAppCommand::ToggleTheme,
        ),
        (
            "command.open_settings",
            "Deschide setările",
            "Preferințe și informații Pană Studio",
            "settings preferences configurare setari aplicatie",
            None,
            740,
            CommandCenterAppCommand::OpenSettings,
        ),
        (
            "command.show_visual",
            "Arată suprafața Vizual",
            "Deschide previzualizarea documentului activ",
            "visual preview canvas document",
            None,
            700,
            CommandCenterAppCommand::ShowVisual,
        ),
        (
            "command.show_code",
            "Arată codul sursă",
            "Deschide codul documentului activ",
            "code source cod sursa document",
            None,
            700,
            CommandCenterAppCommand::ShowCode,
        ),
    ] {
        push_app_command(
            &mut candidates,
            id,
            title,
            subtitle,
            keywords,
            shortcut,
            !matches!(
                command,
                CommandCenterAppCommand::ToggleTheme | CommandCenterAppCommand::OpenSettings
            ),
            priority,
            command,
            has_project,
        );
    }

    for (activity, title, subtitle, keywords, priority) in [
        (
            WorkbenchActivity::Editor,
            "Editor",
            "Documente, preview și cod sursă",
            "edit visual code document",
            880,
        ),
        (
            WorkbenchActivity::Templates,
            "Șabloane",
            "Pagini, layout-uri, fragmente și componente Tera",
            "templates tera layouts partials components sabloane",
            830,
        ),
        (
            WorkbenchActivity::Components,
            "Componente",
            "Parțiale, componente și liste Tera",
            "components tera partials loops componente",
            820,
        ),
        (
            WorkbenchActivity::DesignSystem,
            "Sistem de design",
            "Token-uri, clase, tipografie și identitate vizuală",
            "design tokens scss classes typography",
            810,
        ),
        (
            WorkbenchActivity::Assets,
            "Resurse",
            "Imagini, fonturi și resurse statice",
            "assets images fonts resurse imagini",
            800,
        ),
        (
            WorkbenchActivity::Content,
            "Conținut",
            "Pagini, frontmatter și colecții",
            "content pages markdown frontmatter continut",
            790,
        ),
        (
            WorkbenchActivity::ContentModels,
            "Modele de conținut",
            "Scheme, câmpuri personalizate și atribuirea lor la secțiuni",
            "content models custom fields scheme campuri personalizate",
            790,
        ),
        (
            WorkbenchActivity::Taxonomies,
            "Taxonomii",
            "Definiții, termeni, rute și impact asupra conținutului",
            "taxonomies taxonomy terms routes taxonomii termeni rute",
            789,
        ),
        (
            WorkbenchActivity::Data,
            "Date",
            "Surse locale structurate și relațiile load_data",
            "data date toml json yaml csv xml load_data structured",
            788,
        ),
        (
            WorkbenchActivity::Versioning,
            "Control versiuni",
            "Modificări, commit-uri, ramuri și sincronizare Git",
            "git versioning versions branches commit remote versiuni ramuri",
            785,
        ),
        (
            WorkbenchActivity::Audit,
            "Probleme și audit",
            "Diagnostic unificat pentru proiect și jurnal",
            "audit problems diagnostics errors warning probleme",
            780,
        ),
        (
            WorkbenchActivity::Publish,
            "Publicare",
            "Verificare, construire și livrare",
            "publish deploy build publicare",
            770,
        ),
        (
            WorkbenchActivity::ProjectSettings,
            "Setările proiectului",
            "Configurație Zola și opțiuni portabile ale proiectului",
            "project settings zola configuration setari proiect configuratie",
            765,
        ),
    ] {
        let id = format!("activity.{activity:?}").to_lowercase();
        candidates.push(Candidate {
            title_diagnostic: Some(command_center_copy(&id, "title")),
            subtitle_diagnostic: Some(command_center_copy(&id, "subtitle")),
            id,
            kind: CommandCenterItemKind::Activity,
            title: title.to_string(),
            subtitle: subtitle.to_string(),
            keywords: keywords.to_string(),
            shortcut: None,
            enabled: has_project,
            disabled_diagnostic: (!has_project)
                .then(|| LocalizedDiagnostic::new("command-center-open-project-first")),
            priority,
            action: CommandCenterAction::SetActivity { activity },
        });
    }
    candidates
}

#[allow(clippy::too_many_arguments)]
fn push_app_command(
    candidates: &mut Vec<Candidate>,
    id: &str,
    title: &str,
    subtitle: &str,
    keywords: &str,
    shortcut: Option<&str>,
    requires_project: bool,
    priority: i32,
    command: CommandCenterAppCommand,
    has_project: bool,
) {
    candidates.push(Candidate {
        id: id.to_string(),
        kind: CommandCenterItemKind::Command,
        title: title.to_string(),
        title_diagnostic: Some(command_center_copy(id, "title")),
        subtitle: subtitle.to_string(),
        subtitle_diagnostic: Some(command_center_copy(id, "subtitle")),
        keywords: keywords.to_string(),
        shortcut: shortcut.map(str::to_string),
        enabled: !requires_project || has_project,
        disabled_diagnostic: (requires_project && !has_project)
            .then(|| LocalizedDiagnostic::new("command-center-open-project-first")),
        priority,
        action: CommandCenterAction::AppCommand { command },
    });
}

const COMMAND_CENTER_COPY_CODES: &[(&str, &str, &str)] = &[
    (
        "command.open_project",
        "command-center-command-open-project-title",
        "command-center-command-open-project-subtitle",
    ),
    (
        "command.close_application",
        "command-center-command-close-application-title",
        "command-center-command-close-application-subtitle",
    ),
    (
        "command.save",
        "command-center-command-save-title",
        "command-center-command-save-subtitle",
    ),
    (
        "command.undo",
        "command-center-command-undo-title",
        "command-center-command-undo-subtitle",
    ),
    (
        "command.redo",
        "command-center-command-redo-title",
        "command-center-command-redo-subtitle",
    ),
    (
        "command.validate",
        "command-center-command-validate-title",
        "command-center-command-validate-subtitle",
    ),
    (
        "command.run_external",
        "command-center-command-run-external-title",
        "command-center-command-run-external-subtitle",
    ),
    (
        "command.refresh_session",
        "command-center-command-refresh-session-title",
        "command-center-command-refresh-session-subtitle",
    ),
    (
        "command.rescan_project",
        "command-center-command-rescan-project-title",
        "command-center-command-rescan-project-subtitle",
    ),
    (
        "command.close_project",
        "command-center-command-close-project-title",
        "command-center-command-close-project-subtitle",
    ),
    (
        "command.toggle_terminal",
        "command-center-command-toggle-terminal-title",
        "command-center-command-toggle-terminal-subtitle",
    ),
    (
        "command.show_problems",
        "command-center-command-show-problems-title",
        "command-center-command-show-problems-subtitle",
    ),
    (
        "command.show_output",
        "command-center-command-show-output-title",
        "command-center-command-show-output-subtitle",
    ),
    (
        "command.show_timeline",
        "command-center-command-show-timeline-title",
        "command-center-command-show-timeline-subtitle",
    ),
    (
        "command.split_vertical",
        "command-center-command-split-vertical-title",
        "command-center-command-split-vertical-subtitle",
    ),
    (
        "command.split_horizontal",
        "command-center-command-split-horizontal-title",
        "command-center-command-split-horizontal-subtitle",
    ),
    (
        "command.close_split",
        "command-center-command-close-split-title",
        "command-center-command-close-split-subtitle",
    ),
    (
        "command.canvas_fit",
        "command-center-command-canvas-fit-title",
        "command-center-command-canvas-fit-subtitle",
    ),
    (
        "command.canvas_desktop",
        "command-center-command-canvas-desktop-title",
        "command-center-command-canvas-desktop-subtitle",
    ),
    (
        "command.canvas_tablet",
        "command-center-command-canvas-tablet-title",
        "command-center-command-canvas-tablet-subtitle",
    ),
    (
        "command.canvas_mobile",
        "command-center-command-canvas-mobile-title",
        "command-center-command-canvas-mobile-subtitle",
    ),
    (
        "command.toggle_left_sidebar",
        "command-center-command-toggle-left-sidebar-title",
        "command-center-command-toggle-left-sidebar-subtitle",
    ),
    (
        "command.toggle_inspector",
        "command-center-command-toggle-inspector-title",
        "command-center-command-toggle-inspector-subtitle",
    ),
    (
        "command.toggle_theme",
        "command-center-command-toggle-theme-title",
        "command-center-command-toggle-theme-subtitle",
    ),
    (
        "command.open_settings",
        "command-center-command-open-settings-title",
        "command-center-command-open-settings-subtitle",
    ),
    (
        "command.open_about",
        "command-center-command-open-about-title",
        "command-center-command-open-about-subtitle",
    ),
    (
        "command.show_visual",
        "command-center-command-show-visual-title",
        "command-center-command-show-visual-subtitle",
    ),
    (
        "command.show_code",
        "command-center-command-show-code-title",
        "command-center-command-show-code-subtitle",
    ),
    (
        "activity.editor",
        "command-center-activity-editor-title",
        "command-center-activity-editor-subtitle",
    ),
    (
        "activity.templates",
        "command-center-activity-templates-title",
        "command-center-activity-templates-subtitle",
    ),
    (
        "activity.components",
        "command-center-activity-components-title",
        "command-center-activity-components-subtitle",
    ),
    (
        "activity.designsystem",
        "command-center-activity-designsystem-title",
        "command-center-activity-designsystem-subtitle",
    ),
    (
        "activity.assets",
        "command-center-activity-assets-title",
        "command-center-activity-assets-subtitle",
    ),
    (
        "activity.content",
        "command-center-activity-content-title",
        "command-center-activity-content-subtitle",
    ),
    (
        "activity.contentmodels",
        "command-center-activity-contentmodels-title",
        "command-center-activity-contentmodels-subtitle",
    ),
    (
        "activity.taxonomies",
        "command-center-activity-taxonomies-title",
        "command-center-activity-taxonomies-subtitle",
    ),
    (
        "activity.data",
        "command-center-activity-data-title",
        "command-center-activity-data-subtitle",
    ),
    (
        "activity.versioning",
        "command-center-activity-versioning-title",
        "command-center-activity-versioning-subtitle",
    ),
    (
        "activity.audit",
        "command-center-activity-audit-title",
        "command-center-activity-audit-subtitle",
    ),
    (
        "activity.publish",
        "command-center-activity-publish-title",
        "command-center-activity-publish-subtitle",
    ),
    (
        "activity.projectsettings",
        "command-center-activity-project-settings-title",
        "command-center-activity-project-settings-subtitle",
    ),
];

fn command_center_copy(id: &str, field: &str) -> LocalizedDiagnostic {
    let (_, title, subtitle) = COMMAND_CENTER_COPY_CODES
        .iter()
        .find(|(candidate_id, _, _)| *candidate_id == id)
        .unwrap_or_else(|| panic!("Command Center copy mapping missing for {id}"));
    LocalizedDiagnostic::new(match field {
        "title" => *title,
        "subtitle" => *subtitle,
        _ => panic!("unsupported Command Center copy field {field}"),
    })
}

fn append_project_candidates(candidates: &mut Vec<Candidate>, model: &ProjectModel) {
    let graph = &model.source_graph;
    let mut specialized_paths = HashSet::new();
    for page in &graph.pages {
        specialized_paths.insert(page.file.clone());
        candidates.push(document_candidate(
            format!("page.{}", page.id),
            CommandCenterItemKind::Page,
            non_empty(&page.title, file_name(&page.file)),
            format!("Pagină · {} · {}", page.url, page.file),
            format!("page content markdown {} {}", page.url, page.file),
            page.file.clone(),
            WorkbenchSurface::Code,
            650,
            None,
            Some(
                LocalizedDiagnostic::new("command-center-resource-page-subtitle")
                    .with_argument("url", page.url.clone())
                    .with_argument("path", page.file.clone()),
            ),
        ));
    }
    for template in &graph.templates {
        specialized_paths.insert(template.file.clone());
        let kind = if template.is_partial {
            CommandCenterItemKind::Component
        } else {
            CommandCenterItemKind::File
        };
        candidates.push(document_candidate(
            format!("template.{}", template.id),
            kind,
            template.name.clone(),
            format!(
                "{} · {}",
                if template.is_partial {
                    "Partial Tera"
                } else {
                    "Template Tera"
                },
                template.file
            ),
            format!(
                "tera template partial component {} {} {}",
                template.file,
                template.blocks.join(" "),
                template
                    .component_definitions
                    .iter()
                    .map(|component| component.name.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            template.file.clone(),
            WorkbenchSurface::Visual,
            if template.is_partial { 620 } else { 600 },
            None,
            Some(
                LocalizedDiagnostic::new(if template.is_partial {
                    "command-center-resource-tera-partial-subtitle"
                } else {
                    "command-center-resource-tera-template-subtitle"
                })
                .with_argument("path", template.file.clone()),
            ),
        ));
        for component in &template.component_definitions {
            let component_name = &component.name;
            candidates.push(document_candidate(
                format!("symbol.component.{}.{}", template.id, component_name),
                CommandCenterItemKind::Symbol,
                format!("Componentă {component_name}"),
                format!("{} · componentă Tera", template.file),
                format!("componentă tera {component_name} {}", template.file),
                template.file.clone(),
                WorkbenchSurface::Code,
                540,
                Some(
                    LocalizedDiagnostic::new("command-center-resource-component-title")
                        .with_argument("name", component_name.clone()),
                ),
                Some(
                    LocalizedDiagnostic::new("command-center-resource-component-subtitle")
                        .with_argument("path", template.file.clone()),
                ),
            ));
        }
        for block_name in &template.blocks {
            candidates.push(document_candidate(
                format!("symbol.block.{}.{}", template.id, block_name),
                CommandCenterItemKind::Symbol,
                format!("Block {block_name}"),
                format!("{} · block Tera", template.file),
                format!("block tera {block_name} {}", template.file),
                template.file.clone(),
                WorkbenchSurface::Code,
                520,
                Some(
                    LocalizedDiagnostic::new("command-center-resource-block-title")
                        .with_argument("name", block_name.clone()),
                ),
                Some(
                    LocalizedDiagnostic::new("command-center-resource-block-subtitle")
                        .with_argument("path", template.file.clone()),
                ),
            ));
        }
    }
    for style in &graph.styles {
        specialized_paths.insert(style.file.clone());
        candidates.push(document_candidate(
            format!("style.{}", style.id),
            CommandCenterItemKind::Style,
            file_name(&style.file).to_string(),
            format!("Stil SCSS/CSS · {}", style.file),
            format!("style css scss design token {}", style.file),
            style.file.clone(),
            WorkbenchSurface::Code,
            570,
            None,
            Some(
                LocalizedDiagnostic::new("command-center-resource-style-subtitle")
                    .with_argument("path", style.file.clone()),
            ),
        ));
    }
    for script in &graph.scripts {
        specialized_paths.insert(script.file.clone());
        candidates.push(document_candidate(
            format!("script.{}", script.id),
            CommandCenterItemKind::File,
            file_name(&script.file).to_string(),
            format!("JavaScript · {}", script.file),
            format!("javascript script js {}", script.logical_path),
            script.file.clone(),
            WorkbenchSurface::Code,
            550,
            None,
            Some(
                LocalizedDiagnostic::new("command-center-resource-javascript-subtitle")
                    .with_argument("path", script.file.clone()),
            ),
        ));
    }
    for asset in &graph.assets {
        specialized_paths.insert(asset.file.clone());
        candidates.push(Candidate {
            id: format!("asset.{}", asset.id),
            kind: CommandCenterItemKind::Asset,
            title: file_name(&asset.file).to_string(),
            title_diagnostic: None,
            subtitle: format!("Asset · {}", asset.logical_path),
            subtitle_diagnostic: Some(
                LocalizedDiagnostic::new("command-center-resource-asset-subtitle")
                    .with_argument("path", asset.logical_path.clone()),
            ),
            keywords: format!(
                "asset image font static {} {}",
                asset.file, asset.logical_path
            ),
            shortcut: None,
            enabled: true,
            disabled_diagnostic: None,
            priority: 500,
            action: CommandCenterAction::SetActivity {
                activity: WorkbenchActivity::Assets,
            },
        });
    }
    for data in &graph.data_files {
        specialized_paths.insert(data.file.clone());
        if data.capabilities.can_open_in_code {
            candidates.push(document_candidate(
                format!("data.{}", data.id),
                CommandCenterItemKind::File,
                file_name(&data.file).to_string(),
                format!("Date · {}", data.file),
                format!(
                    "data json toml yaml {} {}",
                    data.logical_path,
                    data.load_paths.join(" ")
                ),
                data.file.clone(),
                WorkbenchSurface::Code,
                500,
                None,
                Some(
                    LocalizedDiagnostic::new("command-center-resource-data-subtitle")
                        .with_argument("path", data.file.clone()),
                ),
            ));
        } else {
            candidates.push(Candidate {
                id: format!("data.{}", data.id),
                kind: CommandCenterItemKind::File,
                title: file_name(&data.file).to_string(),
                title_diagnostic: None,
                subtitle: format!("Date read-only · {}", data.file),
                subtitle_diagnostic: Some(
                    LocalizedDiagnostic::new("command-center-resource-data-readonly-subtitle")
                        .with_argument("path", data.file.clone()),
                ),
                keywords: format!(
                    "data read-only {} {}",
                    data.logical_path,
                    data.load_paths.join(" ")
                ),
                shortcut: None,
                enabled: true,
                disabled_diagnostic: None,
                priority: 500,
                action: CommandCenterAction::SetActivity {
                    activity: WorkbenchActivity::Data,
                },
            });
        }
    }
    for (index, diagnostic) in graph.diagnostics.iter().enumerate() {
        let action = diagnostic.file.as_ref().map_or(
            CommandCenterAction::SetActivity {
                activity: WorkbenchActivity::Audit,
            },
            |file| CommandCenterAction::OpenDocument {
                relative_path: file.clone(),
                surface: WorkbenchSurface::Code,
            },
        );
        candidates.push(Candidate {
            id: format!("diagnostic.{index}"),
            kind: CommandCenterItemKind::Diagnostic,
            title: format!(
                "{} {}",
                diagnostic.diagnostic.code,
                serde_json::to_string(&diagnostic.diagnostic.arguments).unwrap_or_default()
            ),
            title_diagnostic: Some(diagnostic.diagnostic.clone()),
            subtitle: diagnostic
                .file
                .clone()
                .unwrap_or_else(|| "ProjectModel".to_string()),
            subtitle_diagnostic: None,
            keywords: format!(
                "diagnostic audit warning error {}",
                diagnostic.diagnostic.code
            ),
            shortcut: None,
            enabled: true,
            disabled_diagnostic: None,
            priority: 580,
            action,
        });
    }
    for file in &model.files {
        if specialized_paths.contains(&file.relative_path) {
            continue;
        }
        let (kind, label) = match file.kind {
            ProjectModelFileKind::Content => (CommandCenterItemKind::Page, "Conținut"),
            ProjectModelFileKind::Style => (CommandCenterItemKind::Style, "Stil"),
            _ => (CommandCenterItemKind::File, "Fișier"),
        };
        candidates.push(document_candidate(
            format!("file.{}", file.relative_path),
            kind,
            file_name(&file.relative_path).to_string(),
            format!("{label} · {}", file.relative_path),
            format!("file source {label} {}", file.relative_path),
            file.relative_path.clone(),
            WorkbenchSurface::Code,
            460,
            None,
            Some(
                LocalizedDiagnostic::new(match file.kind {
                    ProjectModelFileKind::Content => "command-center-resource-content-subtitle",
                    ProjectModelFileKind::Style => "command-center-resource-style-subtitle",
                    _ => "command-center-resource-file-subtitle",
                })
                .with_argument("path", file.relative_path.clone()),
            ),
        ));
    }
}

#[allow(clippy::too_many_arguments)]
fn document_candidate(
    id: String,
    kind: CommandCenterItemKind,
    title: String,
    subtitle: String,
    keywords: String,
    relative_path: String,
    surface: WorkbenchSurface,
    priority: i32,
    title_diagnostic: Option<LocalizedDiagnostic>,
    subtitle_diagnostic: Option<LocalizedDiagnostic>,
) -> Candidate {
    Candidate {
        id,
        kind,
        title,
        title_diagnostic,
        subtitle,
        subtitle_diagnostic,
        keywords,
        shortcut: None,
        enabled: true,
        disabled_diagnostic: None,
        priority,
        action: CommandCenterAction::OpenDocument {
            relative_path,
            surface,
        },
    }
}

fn candidate_score(candidate: &Candidate, tokens: &[&str]) -> Option<i32> {
    if tokens.is_empty() {
        return Some(candidate.priority);
    }
    let title = normalize(&candidate.title);
    let subtitle = normalize(&candidate.subtitle);
    let keywords = normalize(&candidate.keywords);
    let mut score = candidate.priority / 10;
    for token in tokens {
        let token_score = field_score(&title, token, 8)
            .max(field_score(&subtitle, token, 4))
            .max(field_score(&keywords, token, 2));
        if token_score == 0 {
            return None;
        }
        score += token_score;
    }
    Some(score + i32::from(candidate.enabled) * 20)
}

fn field_score(field: &str, token: &str, weight: i32) -> i32 {
    if field == token {
        return 100 * weight;
    }
    if field.starts_with(token) {
        return 80 * weight;
    }
    if field.split_whitespace().any(|word| word.starts_with(token)) {
        return 65 * weight;
    }
    if field.contains(token) {
        return 45 * weight;
    }
    subsequence_score(field, token).map_or(0, |score| score * weight)
}

fn subsequence_score(field: &str, token: &str) -> Option<i32> {
    let mut position = 0usize;
    let chars = field.chars().collect::<Vec<_>>();
    let mut gaps = 0usize;
    for needle in token.chars() {
        let found = chars[position..]
            .iter()
            .position(|candidate| *candidate == needle)?;
        gaps += found;
        position += found + 1;
    }
    Some((24i32 - gaps.min(20) as i32).max(4))
}

fn scope_accepts(scope: CommandCenterScope, kind: CommandCenterItemKind) -> bool {
    match scope {
        CommandCenterScope::All => true,
        CommandCenterScope::Commands => matches!(
            kind,
            CommandCenterItemKind::Command | CommandCenterItemKind::Activity
        ),
        CommandCenterScope::Files => matches!(
            kind,
            CommandCenterItemKind::File
                | CommandCenterItemKind::Page
                | CommandCenterItemKind::Component
                | CommandCenterItemKind::Style
                | CommandCenterItemKind::Asset
        ),
        CommandCenterScope::Symbols => matches!(
            kind,
            CommandCenterItemKind::Symbol | CommandCenterItemKind::Diagnostic
        ),
    }
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .map(|character| match character {
            'ă' | 'â' => 'a',
            'î' => 'i',
            'ș' | 'ş' => 's',
            'ț' | 'ţ' => 't',
            other => other,
        })
        .collect()
}

fn file_name(path: &str) -> &str {
    path.rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(path)
}

fn non_empty(value: &str, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::{
        project_model::model::ProjectModelFile,
        source_graph::model::{SourceGraph, SourceGraphTemplate, SourceOrigin},
    };

    fn request(query: &str, scope: CommandCenterScope) -> CommandCenterSearchRequest {
        CommandCenterSearchRequest {
            query: query.to_string(),
            scope,
            limit: Some(100),
            expected_project_root: None,
            expected_session_id: None,
        }
    }

    fn model_with_index_template() -> ProjectModel {
        ProjectModel {
            project_root: PathBuf::from("/project"),
            zola_root: PathBuf::from("/project"),
            revision: "model-revision".to_string(),
            files: Vec::new(),
            workspace_paths: Default::default(),
            source_graph: SourceGraph {
                node_index: Default::default(),
                project_root: "/project".to_string(),
                zola_root: "/project".to_string(),
                active_theme: None,
                pages: Vec::new(),
                templates: vec![SourceGraphTemplate {
                    id: "template:index".to_string(),
                    file: "templates/index.html".to_string(),
                    name: "index.html".to_string(),
                    origin: SourceOrigin::Local,
                    theme_name: None,
                    is_partial: false,
                    extends: None,
                    includes: Vec::new(),
                    include_groups: Vec::new(),
                    get_pages: Vec::new(),
                    get_sections: Vec::new(),
                    internal_links: Vec::new(),
                    asset_urls: Vec::new(),
                    asset_hashes: Vec::new(),
                    literal_asset_references: Vec::new(),
                    asset_reference_eligible: 0,
                    asset_reference_unanalysable: 0,
                    data_loads: Vec::new(),
                    image_metadata: Vec::new(),
                    image_resizes: Vec::new(),
                    blocks: vec!["hero".to_string()],
                    component_definitions: Vec::new(),
                    component_calls: Vec::new(),
                    semantics: None,
                    markdown_projections: Vec::new(),
                    node_id: "node:index".to_string(),
                }],
                styles: Vec::new(),
                scripts: Vec::new(),
                assets: Vec::new(),
                data_files: Vec::new(),
                structured_documents: Vec::new(),
                component_graph: Default::default(),
                block_graph: Default::default(),
                content_models: Default::default(),
                listing_items: Default::default(),
                dynamic_widget_graph: Default::default(),
                markdown_projections: Vec::new(),
                nodes: Vec::new(),
                relations: Vec::new(),
                asset_reference_coverage: Default::default(),
                diagnostics: Vec::new(),
            },
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn empty_query_returns_ranked_commands_and_activities_only() {
        let response = search_command_center_index(
            request("", CommandCenterScope::All),
            Some("/project"),
            Some("session"),
            None,
        )
        .unwrap();
        assert!(!response.results.is_empty());
        assert!(response.results.iter().all(|item| matches!(
            item.kind,
            CommandCenterItemKind::Command | CommandCenterItemKind::Activity
        )));
        assert_eq!(response.results[0].id, "command.save");
    }

    #[test]
    fn every_static_copy_code_formats_in_both_bundled_locales() {
        for (_, title_code, subtitle_code) in COMMAND_CENTER_COPY_CODES {
            for locale in ["en-US", "ro"] {
                assert!(
                    crate::localization::format_message(locale, title_code, None).is_ok(),
                    "{locale} cannot format {title_code}"
                );
                assert!(
                    crate::localization::format_message(locale, subtitle_code, None).is_ok(),
                    "{locale} cannot format {subtitle_code}"
                );
            }
        }
    }

    #[test]
    fn search_is_romanian_diacritic_insensitive() {
        let response = search_command_center_index(
            request("setari aplicatie", CommandCenterScope::Commands),
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(response.results[0].id, "command.open_settings");
    }

    #[test]
    fn git_search_opens_the_canonical_versioning_activity() {
        let response = search_command_center_index(
            request("git", CommandCenterScope::All),
            Some("/project"),
            Some("session"),
            None,
        )
        .unwrap();
        let versioning = response
            .results
            .iter()
            .find(|item| {
                matches!(
                    &item.action,
                    CommandCenterAction::SetActivity {
                        activity: WorkbenchActivity::Versioning,
                    }
                )
            })
            .expect("versioning activity");
        assert_eq!(
            versioning
                .title_diagnostic
                .as_ref()
                .map(|copy| copy.code.as_str()),
            Some("command-center-activity-versioning-title")
        );
        assert!(versioning.enabled);
    }

    #[test]
    fn project_bound_layout_commands_are_disabled_without_a_session() {
        let response = search_command_center_index(
            request("terminal", CommandCenterScope::Commands),
            None,
            None,
            None,
        )
        .unwrap();
        let terminal = response
            .results
            .iter()
            .find(|item| item.id == "command.toggle_terminal")
            .expect("terminal command");
        assert!(!terminal.enabled);
        assert!(terminal.disabled_diagnostic.is_some());
    }

    #[test]
    fn command_scope_excludes_resource_kinds() {
        let candidate = document_candidate(
            "file.index".to_string(),
            CommandCenterItemKind::File,
            "index.html".to_string(),
            "Fișier · templates/index.html".to_string(),
            "file index".to_string(),
            "templates/index.html".to_string(),
            WorkbenchSurface::Code,
            500,
            None,
            None,
        );
        assert!(!scope_accepts(CommandCenterScope::Commands, candidate.kind));
        assert!(scope_accepts(CommandCenterScope::Files, candidate.kind));
    }

    #[test]
    fn project_model_indexes_templates_and_tera_symbols_as_typed_actions() {
        let model = model_with_index_template();
        let file_response = search_command_center_index(
            request("index", CommandCenterScope::Files),
            Some("/project"),
            Some("session"),
            Some(&model),
        )
        .unwrap();
        assert_eq!(file_response.results.len(), 1);
        assert!(matches!(
            &file_response.results[0].action,
            CommandCenterAction::OpenDocument {
                relative_path,
                surface: WorkbenchSurface::Visual,
            } if relative_path == "templates/index.html"
        ));

        let symbol_response = search_command_center_index(
            request("hero", CommandCenterScope::Symbols),
            Some("/project"),
            Some("session"),
            Some(&model),
        )
        .unwrap();
        assert_eq!(symbol_response.results.len(), 1);
        assert_eq!(
            symbol_response.results[0].kind,
            CommandCenterItemKind::Symbol
        );
        assert!(matches!(
            &symbol_response.results[0].action,
            CommandCenterAction::OpenDocument {
                relative_path,
                surface: WorkbenchSurface::Code,
            } if relative_path == "templates/index.html"
        ));
    }

    #[test]
    fn fallback_markdown_resources_open_in_the_canonical_code_surface() {
        let mut model = model_with_index_template();
        model.files.push(ProjectModelFile {
            relative_path: "content/notes.md".to_string(),
            kind: ProjectModelFileKind::Content,
            contents: "+++#\n+++#\n".to_string(),
            size_bytes: 10,
            revision: "notes-revision".to_string(),
            source_hash: crate::kernel::file_buffer_store::hash_text("+++#\n+++#\n"),
            from_draft: false,
        });

        let response = search_command_center_index(
            request("notes", CommandCenterScope::Files),
            Some("/project"),
            Some("session"),
            Some(&model),
        )
        .unwrap();

        assert!(response.results.iter().any(|result| matches!(
            &result.action,
            CommandCenterAction::OpenDocument {
                relative_path,
                surface: WorkbenchSurface::Code,
            } if relative_path == "content/notes.md"
        )));
    }

    #[test]
    fn fuzzy_subsequence_matches_without_outweighing_prefixes() {
        let exact = field_score("design system", "des", 8);
        let fuzzy = field_score("diagnostic", "dgn", 8);
        assert!(exact > fuzzy);
        assert!(fuzzy > 0);
    }

    #[test]
    fn oversized_queries_are_rejected() {
        let query = "x".repeat(COMMAND_CENTER_MAX_QUERY_BYTES + 1);
        let error =
            search_command_center_index(request(&query, CommandCenterScope::All), None, None, None)
                .unwrap_err();
        assert_eq!(error.code, "command-center-query-too-long");
    }
}
