use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::kernel::generated_assets::ANIME_JS_RUNTIME_CONTRACT;
use crate::localization::LocalizedDiagnostic;

pub const MOTION_SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MotionRuntimeContract {
    pub schema_version: u32,
    pub anime_version: String,
}

impl MotionRuntimeContract {
    pub fn current() -> Self {
        Self {
            schema_version: MOTION_SCHEMA_VERSION,
            anime_version: ANIME_JS_RUNTIME_CONTRACT.version.to_string(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_name() -> String {
    "Interacțiune".to_string()
}

fn default_action_name() -> String {
    "Animație".to_string()
}

fn default_ease() -> String {
    "out(3)".to_string()
}

fn default_playback_rate() -> f64 {
    1.0
}

fn default_duration_ms() -> f64 {
    600.0
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MotionDocument {
    pub schema_version: u32,
    pub anime_version: String,
    #[serde(default)]
    pub interactions: Vec<MotionInteraction>,
    #[serde(default)]
    pub behaviors: Vec<MotionBehavior>,
    #[serde(default)]
    pub custom_code: Vec<MotionCustomCode>,
}

impl Default for MotionDocument {
    fn default() -> Self {
        Self {
            schema_version: MOTION_SCHEMA_VERSION,
            anime_version: ANIME_JS_RUNTIME_CONTRACT.version.to_string(),
            interactions: Vec::new(),
            behaviors: Vec::new(),
            custom_code: Vec::new(),
        }
    }
}

impl MotionDocument {
    pub fn is_empty(&self) -> bool {
        self.interactions.is_empty() && self.behaviors.is_empty() && self.custom_code.is_empty()
    }

    pub fn from_value(value: Value) -> Result<Self, String> {
        let document: Self = serde_json::from_value(value)
            .map_err(|error| format!("Configurația Motion este invalidă: {error}"))?;
        document.validate()?;
        Ok(document)
    }

    pub fn validate(&self) -> Result<(), String> {
        let diagnostics = self.diagnostics();
        let error = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == MotionDiagnosticSeverity::Error)
            .map(|diagnostic| &diagnostic.diagnostic)
            .next();
        match error {
            None => Ok(()),
            Some(diagnostic) => {
                Err(serde_json::to_string(diagnostic).unwrap_or_else(|_| diagnostic.code.clone()))
            }
        }
    }

    pub fn diagnostics(&self) -> Vec<MotionDiagnostic> {
        let mut diagnostics = Vec::new();
        if self.schema_version != MOTION_SCHEMA_VERSION {
            diagnostics.push(MotionDiagnostic::error(
                "motion.schema_version",
                LocalizedDiagnostic::new("motion-diagnostic-schema-version")
                    .with_argument("expected", MOTION_SCHEMA_VERSION)
                    .with_argument("actual", self.schema_version),
            ));
        }
        if self.anime_version != ANIME_JS_RUNTIME_CONTRACT.version {
            diagnostics.push(MotionDiagnostic::error(
                "motion.anime_version",
                LocalizedDiagnostic::new("motion-diagnostic-anime-version")
                    .with_argument("expected", ANIME_JS_RUNTIME_CONTRACT.version)
                    .with_argument("actual", self.anime_version.clone()),
            ));
        }

        let mut ids = BTreeSet::new();
        for interaction in &self.interactions {
            validate_id(
                &interaction.id,
                "motion.interaction.id",
                &mut ids,
                &mut diagnostics,
            );
            if interaction.name.trim().is_empty() {
                diagnostics.push(MotionDiagnostic::error(
                    interaction_path(interaction, "name"),
                    LocalizedDiagnostic::new("motion-diagnostic-interaction-name-required"),
                ));
            }
            validate_target(
                &interaction.trigger_target,
                &interaction_path(interaction, "triggerTarget"),
                &mut diagnostics,
            );
            validate_trigger(
                &interaction.trigger,
                &interaction_path(interaction, "trigger"),
                &mut diagnostics,
            );
            validate_interaction_contract(interaction, &mut diagnostics);
            let mut media_ids = BTreeSet::new();
            for condition in &interaction.conditions.media_queries {
                validate_id(
                    &condition.id,
                    &interaction_path(interaction, "conditions.mediaQueries.id"),
                    &mut media_ids,
                    &mut diagnostics,
                );
                if condition.query.trim().is_empty() {
                    diagnostics.push(MotionDiagnostic::error(
                        interaction_path(interaction, "conditions.mediaQueries.query"),
                        LocalizedDiagnostic::new("motion-diagnostic-media-query-required"),
                    ));
                }
            }
            validate_playback(
                &interaction.playback,
                &interaction_path(interaction, "playback"),
                &mut diagnostics,
            );
            if interaction.actions.is_empty() {
                diagnostics.push(MotionDiagnostic::error(
                    interaction_path(interaction, "actions"),
                    LocalizedDiagnostic::new("motion-diagnostic-action-required"),
                ));
            }
            let mut action_ids = BTreeSet::new();
            for action in &interaction.actions {
                validate_action(action, interaction, &mut action_ids, &mut diagnostics);
            }
            let mut marker_ids = BTreeSet::new();
            for marker in &interaction.markers {
                validate_id(
                    &marker.id,
                    &interaction_path(interaction, "markers.id"),
                    &mut marker_ids,
                    &mut diagnostics,
                );
                validate_non_negative_finite(
                    marker.at,
                    &interaction_path(interaction, "markers.at"),
                    &mut diagnostics,
                );
                if interaction.domain == MotionTimelineDomain::Progress && marker.at > 100.0 {
                    diagnostics.push(MotionDiagnostic::error(
                        interaction_path(interaction, "markers.at"),
                        LocalizedDiagnostic::new("motion-diagnostic-progress-marker-range"),
                    ));
                }
                if marker.name.trim().is_empty() {
                    diagnostics.push(MotionDiagnostic::error(
                        interaction_path(interaction, "markers.name"),
                        LocalizedDiagnostic::new("motion-diagnostic-marker-name-required"),
                    ));
                }
            }
            for action in &interaction.actions {
                if let MotionAction::Nested(nested) = action {
                    if nested.interaction_id == interaction.id {
                        diagnostics.push(MotionDiagnostic::error(
                            action_path(interaction, action, "interactionId"),
                            LocalizedDiagnostic::new("motion-diagnostic-nested-self"),
                        ));
                    }
                    if !self
                        .interactions
                        .iter()
                        .any(|candidate| candidate.id == nested.interaction_id)
                    {
                        diagnostics.push(MotionDiagnostic::error(
                            action_path(interaction, action, "interactionId"),
                            LocalizedDiagnostic::new("motion-diagnostic-nested-missing")
                                .with_argument("id", nested.interaction_id.clone()),
                        ));
                    }
                }
            }
        }

        if has_nested_cycle(&self.interactions) {
            diagnostics.push(MotionDiagnostic::error(
                "motion.interactions.actions.interactionId",
                LocalizedDiagnostic::new("motion-diagnostic-nested-cycle"),
            ));
        }
        for interaction in self
            .interactions
            .iter()
            .filter(|interaction| interaction.domain == MotionTimelineDomain::Progress)
        {
            for action in &interaction.actions {
                let MotionAction::Nested(nested) = action else {
                    continue;
                };
                let mut visited = BTreeSet::new();
                if interaction_has_scrub_side_effects(
                    &nested.interaction_id,
                    &self.interactions,
                    &mut visited,
                ) {
                    diagnostics.push(MotionDiagnostic::error(
                        action_path(interaction, action, "interactionId"),
                        LocalizedDiagnostic::new("motion-diagnostic-nested-scrub-side-effects"),
                    ));
                }
            }
        }

        for behavior in &self.behaviors {
            validate_id(
                behavior.id(),
                "motion.behavior.id",
                &mut ids,
                &mut diagnostics,
            );
            validate_target(
                behavior.target(),
                &format!("motion.behaviors.{}.target", behavior.id()),
                &mut diagnostics,
            );
            if behavior.name().trim().is_empty() {
                diagnostics.push(MotionDiagnostic::error(
                    format!("motion.behaviors.{}.name", behavior.id()),
                    LocalizedDiagnostic::new("motion-diagnostic-behavior-name-required"),
                ));
            }
            match behavior {
                MotionBehavior::Draggable(behavior) => {
                    validate_non_negative_finite(
                        behavior.snap,
                        &format!("motion.behaviors.{}.snap", behavior.id),
                        &mut diagnostics,
                    );
                    if !behavior.friction.is_finite() || !(0.0..=1.0).contains(&behavior.friction) {
                        diagnostics.push(MotionDiagnostic::error(
                            format!("motion.behaviors.{}.friction", behavior.id),
                            LocalizedDiagnostic::new("motion-diagnostic-friction-range"),
                        ));
                    }
                }
                MotionBehavior::Layout(behavior) => {
                    if !behavior.duration_ms.is_finite() || behavior.duration_ms <= 0.0 {
                        diagnostics.push(MotionDiagnostic::error(
                            format!("motion.behaviors.{}.durationMs", behavior.id),
                            LocalizedDiagnostic::new("motion-diagnostic-layout-duration"),
                        ));
                    }
                }
            }
        }
        for custom in &self.custom_code {
            validate_id(
                &custom.id,
                "motion.customCode.id",
                &mut ids,
                &mut diagnostics,
            );
            if custom.name.trim().is_empty() {
                diagnostics.push(MotionDiagnostic::error(
                    format!("motion.customCode.{}.name", custom.id),
                    LocalizedDiagnostic::new("motion-diagnostic-custom-code-name-required"),
                ));
            }
            if custom.code.trim().is_empty() {
                diagnostics.push(MotionDiagnostic::warning(
                    format!("motion.customCode.{}.code", custom.id),
                    LocalizedDiagnostic::new("motion-diagnostic-custom-code-empty"),
                ));
            }
        }
        diagnostics
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MotionInteraction {
    pub id: String,
    #[serde(default = "default_name")]
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub trigger: MotionTrigger,
    pub trigger_target: MotionTarget,
    #[serde(default)]
    pub conditions: MotionConditions,
    #[serde(default)]
    pub playback: MotionPlayback,
    #[serde(default)]
    pub domain: MotionTimelineDomain,
    #[serde(default)]
    pub actions: Vec<MotionAction>,
    #[serde(default)]
    pub markers: Vec<MotionMarker>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum MotionTrigger {
    Load {
        #[serde(default)]
        phase: MotionLoadPhase,
    },
    InView {
        #[serde(default = "default_in_view_threshold")]
        threshold: f64,
        #[serde(default)]
        once: bool,
    },
    Click {
        #[serde(default)]
        first_click: MotionTriggerCommand,
        #[serde(default)]
        second_click: MotionTriggerCommand,
        #[serde(default)]
        prevent_default: bool,
    },
    Hover {
        #[serde(default)]
        enter: MotionTriggerCommand,
        #[serde(default = "default_reverse_command")]
        leave: MotionTriggerCommand,
    },
    Scroll {
        #[serde(default)]
        mode: MotionScrollMode,
        #[serde(default = "default_scroll_start")]
        start: String,
        #[serde(default = "default_scroll_end")]
        end: String,
        #[serde(default)]
        smooth_ms: f64,
        #[serde(default = "default_true")]
        once: bool,
    },
    Pointer {
        #[serde(default)]
        axis: MotionPointerAxis,
        #[serde(default = "default_pointer_smooth")]
        smooth_ms: f64,
        #[serde(default)]
        rest: f64,
    },
    Custom {
        event: String,
        #[serde(default)]
        prevent_default: bool,
    },
}

impl Default for MotionTrigger {
    fn default() -> Self {
        Self::Load {
            phase: MotionLoadPhase::DomReady,
        }
    }
}

fn default_in_view_threshold() -> f64 {
    0.15
}

fn default_scroll_start() -> String {
    "bottom top".to_string()
}

fn default_scroll_end() -> String {
    "top bottom".to_string()
}

fn default_pointer_smooth() -> f64 {
    50.0
}

fn default_reverse_command() -> MotionTriggerCommand {
    MotionTriggerCommand::Reverse
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MotionLoadPhase {
    #[default]
    DomReady,
    WindowLoad,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MotionTriggerCommand {
    #[default]
    Restart,
    Play,
    Pause,
    Reverse,
    Toggle,
    Reset,
    None,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MotionScrollMode {
    #[default]
    Trigger,
    Scrub,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MotionPointerAxis {
    #[default]
    X,
    Y,
    Both,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MotionConditions {
    #[serde(default)]
    pub media_queries: Vec<MotionMediaCondition>,
    #[serde(default)]
    pub reduced_motion: MotionReducedMotion,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MotionMediaCondition {
    pub id: String,
    pub query: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MotionReducedMotion {
    #[default]
    Reduce,
    SkipToEnd,
    Disable,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MotionPlayback {
    #[serde(default)]
    pub delay_ms: f64,
    #[serde(default)]
    pub repeat: u32,
    #[serde(default)]
    pub infinite: bool,
    #[serde(default)]
    pub loop_delay_ms: f64,
    #[serde(default)]
    pub alternate: bool,
    #[serde(default)]
    pub reversed: bool,
    #[serde(default = "default_playback_rate")]
    pub playback_rate: f64,
    #[serde(default)]
    pub playback_ease: String,
}

impl Default for MotionPlayback {
    fn default() -> Self {
        Self {
            delay_ms: 0.0,
            repeat: 0,
            infinite: false,
            loop_delay_ms: 0.0,
            alternate: false,
            reversed: false,
            playback_rate: default_playback_rate(),
            playback_ease: String::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MotionTimelineDomain {
    #[default]
    Time,
    Progress,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MotionMarker {
    pub id: String,
    pub name: String,
    pub at: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum MotionAction {
    Animate(MotionAnimateAction),
    Set(MotionSetAction),
    Media(MotionMediaAction),
    Call(MotionCallAction),
    Nested(MotionNestedAction),
}

impl MotionAction {
    pub fn id(&self) -> &str {
        match self {
            Self::Animate(action) => &action.id,
            Self::Set(action) => &action.id,
            Self::Media(action) => &action.id,
            Self::Call(action) => &action.id,
            Self::Nested(action) => &action.id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Animate(action) => &action.name,
            Self::Set(action) => &action.name,
            Self::Media(action) => &action.name,
            Self::Call(action) => &action.name,
            Self::Nested(action) => &action.name,
        }
    }

    pub fn enabled(&self) -> bool {
        match self {
            Self::Animate(action) => action.enabled,
            Self::Set(action) => action.enabled,
            Self::Media(action) => action.enabled,
            Self::Call(action) => action.enabled,
            Self::Nested(action) => action.enabled,
        }
    }

    pub fn start(&self) -> f64 {
        match self {
            Self::Animate(action) => action.start,
            Self::Set(action) => action.start,
            Self::Media(action) => action.start,
            Self::Call(action) => action.start,
            Self::Nested(action) => action.start,
        }
    }

    pub fn duration(&self) -> f64 {
        match self {
            Self::Animate(action) => action.duration,
            Self::Nested(action) => action.duration,
            _ => 0.0,
        }
    }

    pub fn target(&self) -> Option<&MotionTarget> {
        match self {
            Self::Animate(action) => Some(&action.target),
            Self::Set(action) => Some(&action.target),
            Self::Media(action) => Some(&action.target),
            Self::Call(_) | Self::Nested(_) => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MotionAnimateAction {
    pub id: String,
    #[serde(default = "default_action_name")]
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub target: MotionTarget,
    #[serde(default)]
    pub start: f64,
    #[serde(default = "default_duration_ms")]
    pub duration: f64,
    #[serde(default)]
    pub mode: MotionAnimationMode,
    #[serde(default = "default_ease")]
    pub ease: String,
    #[serde(default)]
    pub properties: Vec<MotionProperty>,
    #[serde(default)]
    pub keyframes: Vec<MotionKeyframe>,
    #[serde(default)]
    pub stagger: Option<MotionStagger>,
    #[serde(default)]
    pub repeat: MotionActionRepeat,
    #[serde(default)]
    pub specialization: Option<MotionSpecialization>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MotionAnimationMode {
    From,
    #[default]
    To,
    FromTo,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MotionProperty {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub category: MotionPropertyCategory,
    #[serde(default)]
    pub from: Option<MotionValue>,
    pub to: MotionValue,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MotionPropertyCategory {
    #[default]
    Transform,
    Style,
    CssVariable,
    HtmlAttribute,
    SvgAttribute,
    Object,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MotionValue {
    #[serde(default)]
    pub kind: MotionValueKind,
    pub value: String,
    #[serde(default)]
    pub unit: String,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MotionValueKind {
    #[default]
    Number,
    Text,
    Color,
    CssVariable,
    Relative,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MotionKeyframe {
    pub id: String,
    pub offset: f64,
    #[serde(default)]
    pub ease: String,
    #[serde(default)]
    pub properties: Vec<MotionProperty>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MotionStagger {
    #[serde(default)]
    pub amount: f64,
    #[serde(default)]
    pub mode: MotionStaggerMode,
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub reversed: bool,
    #[serde(default)]
    pub ease: String,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MotionStaggerMode {
    #[default]
    Each,
    Total,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MotionActionRepeat {
    #[serde(default)]
    pub count: u32,
    #[serde(default)]
    pub infinite: bool,
    #[serde(default)]
    pub alternate: bool,
    #[serde(default)]
    pub delay_ms: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum MotionSpecialization {
    SplitText {
        #[serde(default)]
        mode: MotionSplitTextMode,
    },
    SvgPath {
        path: String,
        #[serde(default)]
        auto_rotate: bool,
    },
    SvgMorph {
        source: String,
        #[serde(default = "default_svg_precision")]
        precision: f64,
    },
    SvgDraw,
}

fn default_svg_precision() -> f64 {
    0.33
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MotionSplitTextMode {
    Lines,
    Words,
    #[default]
    Chars,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MotionSetAction {
    pub id: String,
    #[serde(default = "default_set_name")]
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub target: MotionTarget,
    #[serde(default)]
    pub start: f64,
    #[serde(default)]
    pub values: Vec<MotionSetValue>,
}

fn default_set_name() -> String {
    "Setează".to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum MotionSetValue {
    Property { name: String, value: MotionValue },
    Attribute { name: String, value: String },
    AddClass { name: String },
    RemoveClass { name: String },
    ToggleClass { name: String },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MotionMediaAction {
    pub id: String,
    #[serde(default = "default_media_name")]
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub target: MotionTarget,
    #[serde(default)]
    pub start: f64,
    #[serde(default)]
    pub command: MotionMediaCommand,
}

fn default_media_name() -> String {
    "Media".to_string()
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MotionMediaCommand {
    #[default]
    Play,
    Pause,
    Toggle,
    Reset,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MotionCallAction {
    pub id: String,
    #[serde(default = "default_call_name")]
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub start: f64,
    #[serde(default)]
    pub code: String,
}

fn default_call_name() -> String {
    "Callback".to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MotionNestedAction {
    pub id: String,
    #[serde(default = "default_nested_name")]
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub start: f64,
    #[serde(default)]
    pub duration: f64,
    pub interaction_id: String,
}

fn default_nested_name() -> String {
    "Interacțiune inclusă".to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MotionTarget {
    #[serde(default)]
    pub kind: MotionTargetKind,
    #[serde(default)]
    pub data_anim: String,
    #[serde(default)]
    pub selector: String,
    #[serde(default)]
    pub relation: MotionTargetRelation,
    #[serde(default)]
    pub scope: MotionTargetScope,
}

impl MotionTarget {
    pub fn for_data_anim(data_anim: impl Into<String>) -> Self {
        Self {
            kind: MotionTargetKind::Element,
            data_anim: data_anim.into(),
            selector: String::new(),
            relation: MotionTargetRelation::SelfElement,
            scope: MotionTargetScope::All,
        }
    }
}

impl Default for MotionTarget {
    fn default() -> Self {
        Self {
            kind: MotionTargetKind::Trigger,
            data_anim: String::new(),
            selector: String::new(),
            relation: MotionTargetRelation::SelfElement,
            scope: MotionTargetScope::All,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MotionTargetKind {
    Element,
    Selector,
    #[default]
    Trigger,
    Relative,
    Viewport,
    Document,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MotionTargetRelation {
    #[default]
    SelfElement,
    Children,
    Descendants,
    Parent,
    Ancestors,
    Siblings,
    NextSibling,
    PreviousSibling,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MotionTargetScope {
    #[default]
    All,
    Each,
    First,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum MotionBehavior {
    Draggable(MotionDraggableBehavior),
    Layout(MotionLayoutBehavior),
}

impl MotionBehavior {
    pub fn id(&self) -> &str {
        match self {
            Self::Draggable(behavior) => &behavior.id,
            Self::Layout(behavior) => &behavior.id,
        }
    }

    pub fn target(&self) -> &MotionTarget {
        match self {
            Self::Draggable(behavior) => &behavior.target,
            Self::Layout(behavior) => &behavior.target,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Draggable(behavior) => &behavior.name,
            Self::Layout(behavior) => &behavior.name,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MotionDraggableBehavior {
    pub id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub target: MotionTarget,
    #[serde(default)]
    pub axis: MotionDragAxis,
    #[serde(default)]
    pub container: String,
    #[serde(default)]
    pub snap: f64,
    #[serde(default = "default_drag_friction")]
    pub friction: f64,
    #[serde(default = "default_true")]
    pub cursor: bool,
}

fn default_drag_friction() -> f64 {
    0.8
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MotionDragAxis {
    X,
    Y,
    #[default]
    Both,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MotionLayoutBehavior {
    pub id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub target: MotionTarget,
    #[serde(default)]
    pub children_selector: String,
    #[serde(default)]
    pub properties: Vec<String>,
    #[serde(default)]
    pub duration_ms: f64,
    #[serde(default = "default_ease")]
    pub ease: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MotionCustomCode {
    pub id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub code: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MotionDiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MotionDiagnostic {
    pub severity: MotionDiagnosticSeverity,
    pub path: String,
    pub diagnostic: LocalizedDiagnostic,
}

impl MotionDiagnostic {
    fn error(path: impl Into<String>, diagnostic: LocalizedDiagnostic) -> Self {
        Self {
            severity: MotionDiagnosticSeverity::Error,
            path: path.into(),
            diagnostic,
        }
    }

    fn warning(path: impl Into<String>, diagnostic: LocalizedDiagnostic) -> Self {
        Self {
            severity: MotionDiagnosticSeverity::Warning,
            path: path.into(),
            diagnostic,
        }
    }
}

fn validate_id(
    id: &str,
    path: &str,
    ids: &mut BTreeSet<String>,
    diagnostics: &mut Vec<MotionDiagnostic>,
) {
    if id.trim().is_empty() {
        diagnostics.push(MotionDiagnostic::error(
            path,
            LocalizedDiagnostic::new("motion-diagnostic-id-required"),
        ));
    } else if !ids.insert(id.to_string()) {
        diagnostics.push(MotionDiagnostic::error(
            path,
            LocalizedDiagnostic::new("motion-diagnostic-id-duplicate").with_argument("id", id),
        ));
    }
}

fn validate_target(target: &MotionTarget, path: &str, diagnostics: &mut Vec<MotionDiagnostic>) {
    match target.kind {
        MotionTargetKind::Element if target.data_anim.trim().is_empty() => {
            diagnostics.push(MotionDiagnostic::error(
                path,
                LocalizedDiagnostic::new("motion-diagnostic-target-data-anim-required"),
            ));
        }
        MotionTargetKind::Selector | MotionTargetKind::Relative
            if target.selector.trim().is_empty() =>
        {
            diagnostics.push(MotionDiagnostic::error(
                path,
                LocalizedDiagnostic::new("motion-diagnostic-target-selector-required"),
            ));
        }
        _ => {}
    }
}

fn validate_trigger(trigger: &MotionTrigger, path: &str, diagnostics: &mut Vec<MotionDiagnostic>) {
    match trigger {
        MotionTrigger::InView { threshold, .. }
            if !threshold.is_finite() || !(0.0..=1.0).contains(threshold) =>
        {
            diagnostics.push(MotionDiagnostic::error(
                format!("{path}.threshold"),
                LocalizedDiagnostic::new("motion-diagnostic-in-view-threshold"),
            ));
        }
        MotionTrigger::Scroll {
            start,
            end,
            smooth_ms,
            ..
        } => {
            if start.trim().is_empty() || end.trim().is_empty() {
                diagnostics.push(MotionDiagnostic::error(
                    path,
                    LocalizedDiagnostic::new("motion-diagnostic-scroll-thresholds-required"),
                ));
            }
            validate_non_negative_finite(*smooth_ms, &format!("{path}.smoothMs"), diagnostics);
        }
        MotionTrigger::Pointer {
            smooth_ms, rest, ..
        } => {
            validate_non_negative_finite(*smooth_ms, &format!("{path}.smoothMs"), diagnostics);
            if !rest.is_finite() || !(0.0..=1.0).contains(rest) {
                diagnostics.push(MotionDiagnostic::error(
                    format!("{path}.rest"),
                    LocalizedDiagnostic::new("motion-diagnostic-pointer-rest-range"),
                ));
            }
        }
        MotionTrigger::Custom { event, .. } if event.trim().is_empty() => {
            diagnostics.push(MotionDiagnostic::error(
                format!("{path}.event"),
                LocalizedDiagnostic::new("motion-diagnostic-custom-event-required"),
            ));
        }
        _ => {}
    }
}

fn validate_interaction_contract(
    interaction: &MotionInteraction,
    diagnostics: &mut Vec<MotionDiagnostic>,
) {
    let requires_progress = matches!(
        interaction.trigger,
        MotionTrigger::Scroll {
            mode: MotionScrollMode::Scrub,
            ..
        } | MotionTrigger::Pointer { .. }
    );
    let expected_domain = if requires_progress {
        MotionTimelineDomain::Progress
    } else {
        MotionTimelineDomain::Time
    };
    if interaction.domain != expected_domain {
        diagnostics.push(MotionDiagnostic::error(
            interaction_path(interaction, "domain"),
            if requires_progress {
                LocalizedDiagnostic::new("motion-diagnostic-progress-domain-required")
            } else {
                LocalizedDiagnostic::new("motion-diagnostic-time-domain-required")
            },
        ));
    }

    if matches!(
        interaction.trigger_target.kind,
        MotionTargetKind::Trigger | MotionTargetKind::Relative
    ) {
        diagnostics.push(MotionDiagnostic::error(
            interaction_path(interaction, "triggerTarget.kind"),
            LocalizedDiagnostic::new("motion-diagnostic-trigger-target-self-relative"),
        ));
    }
    if matches!(
        interaction.trigger,
        MotionTrigger::InView { .. }
            | MotionTrigger::Hover { .. }
            | MotionTrigger::Scroll { .. }
            | MotionTrigger::Pointer { .. }
    ) && matches!(
        interaction.trigger_target.kind,
        MotionTargetKind::Document | MotionTargetKind::Viewport
    ) {
        diagnostics.push(MotionDiagnostic::error(
            interaction_path(interaction, "triggerTarget.kind"),
            LocalizedDiagnostic::new("motion-diagnostic-trigger-element-required"),
        ));
    }

    if interaction.domain == MotionTimelineDomain::Progress {
        let playback = &interaction.playback;
        if playback.delay_ms > 0.0
            || playback.repeat > 0
            || playback.infinite
            || playback.loop_delay_ms > 0.0
            || playback.alternate
        {
            diagnostics.push(MotionDiagnostic::error(
                interaction_path(interaction, "playback"),
                LocalizedDiagnostic::new("motion-diagnostic-scrub-playback"),
            ));
        }
    }
}

fn interaction_has_scrub_side_effects(
    id: &str,
    interactions: &[MotionInteraction],
    visited: &mut BTreeSet<String>,
) -> bool {
    if !visited.insert(id.to_string()) {
        return false;
    }
    let Some(interaction) = interactions.iter().find(|interaction| interaction.id == id) else {
        return false;
    };
    interaction.actions.iter().any(|action| match action {
        MotionAction::Media(_) | MotionAction::Call(_) => true,
        MotionAction::Set(set) => set
            .values
            .iter()
            .any(|value| !matches!(value, MotionSetValue::Property { .. })),
        MotionAction::Nested(nested) => {
            interaction_has_scrub_side_effects(&nested.interaction_id, interactions, visited)
        }
        MotionAction::Animate(_) => false,
    })
}

fn has_nested_cycle(interactions: &[MotionInteraction]) -> bool {
    let graph = interactions
        .iter()
        .map(|interaction| {
            let nested = interaction
                .actions
                .iter()
                .filter_map(|action| match action {
                    MotionAction::Nested(action) => Some(action.interaction_id.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            (interaction.id.clone(), nested)
        })
        .collect::<BTreeMap<_, _>>();
    let mut visited = BTreeSet::new();
    let mut visiting = BTreeSet::new();
    graph
        .keys()
        .any(|id| nested_cycle_from(id, &graph, &mut visiting, &mut visited))
}

fn nested_cycle_from(
    id: &str,
    graph: &BTreeMap<String, Vec<String>>,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
) -> bool {
    if visited.contains(id) {
        return false;
    }
    if !visiting.insert(id.to_string()) {
        return true;
    }
    let has_cycle = graph
        .get(id)
        .into_iter()
        .flatten()
        .filter(|candidate| graph.contains_key(*candidate))
        .any(|candidate| nested_cycle_from(candidate, graph, visiting, visited));
    visiting.remove(id);
    visited.insert(id.to_string());
    has_cycle
}

fn validate_playback(
    playback: &MotionPlayback,
    path: &str,
    diagnostics: &mut Vec<MotionDiagnostic>,
) {
    validate_non_negative_finite(playback.delay_ms, &format!("{path}.delayMs"), diagnostics);
    validate_non_negative_finite(
        playback.loop_delay_ms,
        &format!("{path}.loopDelayMs"),
        diagnostics,
    );
    if !playback.playback_rate.is_finite() || playback.playback_rate <= 0.0 {
        diagnostics.push(MotionDiagnostic::error(
            format!("{path}.playbackRate"),
            LocalizedDiagnostic::new("motion-diagnostic-playback-rate"),
        ));
    }
}

fn validate_action(
    action: &MotionAction,
    interaction: &MotionInteraction,
    ids: &mut BTreeSet<String>,
    diagnostics: &mut Vec<MotionDiagnostic>,
) {
    validate_id(
        action.id(),
        &action_path(interaction, action, "id"),
        ids,
        diagnostics,
    );
    if action.name().trim().is_empty() {
        diagnostics.push(MotionDiagnostic::error(
            action_path(interaction, action, "name"),
            LocalizedDiagnostic::new("motion-diagnostic-action-name-required"),
        ));
    }
    validate_non_negative_finite(
        action.start(),
        &action_path(interaction, action, "start"),
        diagnostics,
    );
    validate_non_negative_finite(
        action.duration(),
        &action_path(interaction, action, "duration"),
        diagnostics,
    );
    if matches!(action, MotionAction::Animate(_)) && action.duration() <= 0.0 {
        diagnostics.push(MotionDiagnostic::error(
            action_path(interaction, action, "duration"),
            LocalizedDiagnostic::new("motion-diagnostic-animate-duration"),
        ));
    }
    if interaction.domain == MotionTimelineDomain::Progress
        && action.start() + action.duration() > 100.000_001
    {
        diagnostics.push(MotionDiagnostic::error(
            action_path(interaction, action, "timing"),
            LocalizedDiagnostic::new("motion-diagnostic-progress-timing-range"),
        ));
    }
    if let Some(target) = action.target() {
        validate_target(
            target,
            &action_path(interaction, action, "target"),
            diagnostics,
        );
    }
    if let MotionAction::Animate(animation) = action {
        if animation.properties.is_empty() && animation.keyframes.is_empty() {
            diagnostics.push(MotionDiagnostic::error(
                action_path(interaction, action, "properties"),
                LocalizedDiagnostic::new("motion-diagnostic-animate-content-required"),
            ));
        }
        let mut property_ids = BTreeSet::new();
        for property in &animation.properties {
            validate_motion_property(
                property,
                &action_path(interaction, action, "properties"),
                &mut property_ids,
                diagnostics,
            );
            if matches!(
                animation.mode,
                MotionAnimationMode::From | MotionAnimationMode::FromTo
            ) && property.from.is_none()
            {
                diagnostics.push(MotionDiagnostic::error(
                    action_path(interaction, action, "properties.from"),
                    LocalizedDiagnostic::new("motion-diagnostic-from-value-required"),
                ));
            }
        }
        let mut frame_ids = BTreeSet::new();
        let mut frame_offsets = BTreeSet::new();
        for frame in &animation.keyframes {
            validate_id(
                &frame.id,
                &action_path(interaction, action, "keyframes.id"),
                &mut frame_ids,
                diagnostics,
            );
            if !frame.offset.is_finite() || !(0.0..=100.0).contains(&frame.offset) {
                diagnostics.push(MotionDiagnostic::error(
                    action_path(interaction, action, "keyframes.offset"),
                    LocalizedDiagnostic::new("motion-diagnostic-keyframe-offset-range"),
                ));
            }
            let offset_key = format!("{:.6}", frame.offset);
            if !frame_offsets.insert(offset_key) {
                diagnostics.push(MotionDiagnostic::error(
                    action_path(interaction, action, "keyframes.offset"),
                    LocalizedDiagnostic::new("motion-diagnostic-keyframe-offset-duplicate"),
                ));
            }
            if frame.properties.is_empty() {
                diagnostics.push(MotionDiagnostic::error(
                    action_path(interaction, action, "keyframes.properties"),
                    LocalizedDiagnostic::new("motion-diagnostic-keyframe-property-required"),
                ));
            }
            let mut frame_property_ids = BTreeSet::new();
            for property in &frame.properties {
                validate_motion_property(
                    property,
                    &action_path(interaction, action, "keyframes.properties"),
                    &mut frame_property_ids,
                    diagnostics,
                );
            }
        }
        validate_non_negative_finite(
            animation.repeat.delay_ms,
            &action_path(interaction, action, "repeat.delayMs"),
            diagnostics,
        );
        let repeated_duration = animation.duration * (f64::from(animation.repeat.count) + 1.0)
            + animation.repeat.delay_ms * f64::from(animation.repeat.count);
        if interaction.domain == MotionTimelineDomain::Progress
            && animation.start + repeated_duration > 100.000_001
        {
            diagnostics.push(MotionDiagnostic::error(
                action_path(interaction, action, "repeat"),
                LocalizedDiagnostic::new("motion-diagnostic-progress-repeat-range"),
            ));
        }
        if animation.repeat.infinite && interaction.domain == MotionTimelineDomain::Progress {
            diagnostics.push(MotionDiagnostic::error(
                action_path(interaction, action, "repeat.infinite"),
                LocalizedDiagnostic::new("motion-diagnostic-progress-infinite-repeat"),
            ));
        }
        if interaction.domain == MotionTimelineDomain::Progress
            && (animation.repeat.count > 0
                || animation.repeat.infinite
                || animation.repeat.alternate
                || animation.repeat.delay_ms > 0.0)
        {
            diagnostics.push(MotionDiagnostic::error(
                action_path(interaction, action, "repeat"),
                LocalizedDiagnostic::new("motion-diagnostic-progress-single-interval"),
            ));
        }
    }
    if let MotionAction::Set(set) = action {
        if set.values.is_empty() {
            diagnostics.push(MotionDiagnostic::error(
                action_path(interaction, action, "values"),
                LocalizedDiagnostic::new("motion-diagnostic-set-value-required"),
            ));
        }
        if interaction.domain == MotionTimelineDomain::Progress
            && set
                .values
                .iter()
                .any(|value| !matches!(value, MotionSetValue::Property { .. }))
        {
            diagnostics.push(MotionDiagnostic::error(
                action_path(interaction, action, "values"),
                LocalizedDiagnostic::new("motion-diagnostic-scrub-reversible-set-only"),
            ));
        }
    }
    if interaction.domain == MotionTimelineDomain::Progress
        && matches!(action, MotionAction::Media(_) | MotionAction::Call(_))
    {
        diagnostics.push(MotionDiagnostic::error(
            action_path(interaction, action, "type"),
            LocalizedDiagnostic::new("motion-diagnostic-progress-side-effects"),
        ));
    }
    if let MotionAction::Nested(nested) = action {
        if nested.duration <= 0.0 {
            diagnostics.push(MotionDiagnostic::error(
                action_path(interaction, action, "duration"),
                LocalizedDiagnostic::new("motion-diagnostic-nested-duration"),
            ));
        }
    }
}

fn validate_motion_property(
    property: &MotionProperty,
    path: &str,
    ids: &mut BTreeSet<String>,
    diagnostics: &mut Vec<MotionDiagnostic>,
) {
    validate_id(&property.id, &format!("{path}.id"), ids, diagnostics);
    if property.name.trim().is_empty() {
        diagnostics.push(MotionDiagnostic::error(
            format!("{path}.name"),
            LocalizedDiagnostic::new("motion-diagnostic-property-name-required"),
        ));
    }
}

fn validate_non_negative_finite(value: f64, path: &str, diagnostics: &mut Vec<MotionDiagnostic>) {
    if !value.is_finite() || value < 0.0 {
        diagnostics.push(MotionDiagnostic::error(
            path,
            LocalizedDiagnostic::new("motion-diagnostic-non-negative-finite"),
        ));
    }
}

fn interaction_path(interaction: &MotionInteraction, field: &str) -> String {
    format!("motion.interactions.{}.{}", interaction.id, field)
}

fn action_path(interaction: &MotionInteraction, action: &MotionAction, field: &str) -> String {
    format!(
        "motion.interactions.{}.actions.{}.{}",
        interaction.id,
        action.id(),
        field
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn v2_roundtrip_is_typed_and_validated() {
        let document = MotionDocument {
            interactions: vec![MotionInteraction {
                id: "hero-load".to_string(),
                name: "Hero load".to_string(),
                enabled: true,
                trigger: MotionTrigger::default(),
                trigger_target: MotionTarget::for_data_anim("hero-title"),
                conditions: MotionConditions::default(),
                playback: MotionPlayback::default(),
                domain: MotionTimelineDomain::Time,
                actions: vec![MotionAction::Animate(MotionAnimateAction {
                    id: "fade".to_string(),
                    name: "Fade".to_string(),
                    enabled: true,
                    target: MotionTarget::for_data_anim("hero-title"),
                    start: 0.0,
                    duration: 600.0,
                    mode: MotionAnimationMode::FromTo,
                    ease: default_ease(),
                    properties: vec![MotionProperty {
                        id: "opacity".to_string(),
                        name: "opacity".to_string(),
                        category: MotionPropertyCategory::Style,
                        from: Some(MotionValue {
                            kind: MotionValueKind::Number,
                            value: "0".to_string(),
                            unit: String::new(),
                        }),
                        to: MotionValue {
                            kind: MotionValueKind::Number,
                            value: "1".to_string(),
                            unit: String::new(),
                        },
                    }],
                    keyframes: Vec::new(),
                    stagger: None,
                    repeat: MotionActionRepeat::default(),
                    specialization: None,
                })],
                markers: Vec::new(),
            }],
            ..MotionDocument::default()
        };
        document.validate().expect("valid Motion v2");
        let value = serde_json::to_value(&document).expect("serialize");
        let decoded = MotionDocument::from_value(value).expect("deserialize");
        assert_eq!(decoded, document);
    }

    #[test]
    fn legacy_motion_schema_is_rejected_without_migration() {
        let error = MotionDocument::from_value(json!({
            "schemaVersion": 1,
            "animeVersion": MotionRuntimeContract::current().anime_version,
            "items": []
        }))
        .expect_err("schema v1 must not be migrated");

        assert!(error.contains("Motion") || error.contains("schema"));
    }

    #[test]
    fn rejects_empty_interaction() {
        let document = MotionDocument {
            interactions: vec![MotionInteraction {
                id: "empty".to_string(),
                name: "Empty".to_string(),
                enabled: true,
                trigger: MotionTrigger::default(),
                trigger_target: MotionTarget::for_data_anim("hero"),
                conditions: MotionConditions::default(),
                playback: MotionPlayback::default(),
                domain: MotionTimelineDomain::Time,
                actions: Vec::new(),
                markers: Vec::new(),
            }],
            ..MotionDocument::default()
        };
        assert!(document.validate().is_err());
    }

    #[test]
    fn rejects_indirect_nested_interaction_cycle() {
        let error = MotionDocument::from_value(json!({
            "schemaVersion": 2,
            "animeVersion": MotionRuntimeContract::current().anime_version,
            "interactions": [
                {
                    "id": "a",
                    "name": "A",
                    "trigger": { "type": "load" },
                    "triggerTarget": { "kind": "document" },
                    "actions": [{
                        "type": "nested",
                        "id": "a-to-b",
                        "name": "B",
                        "duration": 100,
                        "interactionId": "b"
                    }]
                },
                {
                    "id": "b",
                    "name": "B",
                    "trigger": { "type": "load" },
                    "triggerTarget": { "kind": "document" },
                    "actions": [{
                        "type": "nested",
                        "id": "b-to-a",
                        "name": "A",
                        "duration": 100,
                        "interactionId": "a"
                    }]
                }
            ]
        }))
        .expect_err("cycle must fail");

        assert!(error.contains("motion-diagnostic-nested-cycle"));
    }

    #[test]
    fn rejects_side_effects_and_loops_in_progress_domain() {
        let document = serde_json::from_value::<MotionDocument>(json!({
            "schemaVersion": 2,
            "animeVersion": MotionRuntimeContract::current().anime_version,
            "interactions": [{
                "id": "pointer",
                "name": "Pointer",
                "trigger": { "type": "pointer" },
                "triggerTarget": { "kind": "element", "dataAnim": "hero" },
                "domain": "progress",
                "playback": { "infinite": true },
                "actions": [{
                    "type": "call",
                    "id": "unsafe-call",
                    "name": "Unsafe call",
                    "code": "window.sideEffect = true;"
                }]
            }]
        }))
        .expect("typed motion document");
        let diagnostics = document.diagnostics();

        assert!(diagnostics
            .iter()
            .any(|item| { item.diagnostic.code == "motion-diagnostic-progress-side-effects" }));
        assert!(diagnostics
            .iter()
            .any(|item| item.diagnostic.code == "motion-diagnostic-scrub-playback"));
    }

    #[test]
    fn trigger_domain_and_target_must_match_the_trigger_kind() {
        let document = serde_json::from_value::<MotionDocument>(json!({
            "schemaVersion": 2,
            "animeVersion": MotionRuntimeContract::current().anime_version,
            "interactions": [{
                "id": "in-view",
                "name": "In view",
                "trigger": { "type": "inView" },
                "triggerTarget": { "kind": "document" },
                "domain": "progress",
                "actions": [{
                    "type": "set",
                    "id": "show",
                    "name": "Show",
                    "target": { "kind": "selector", "selector": "main" },
                    "values": [{
                        "type": "property",
                        "name": "opacity",
                        "value": { "kind": "number", "value": "1" }
                    }]
                }]
            }]
        }))
        .expect("typed motion document");
        let diagnostics = document.diagnostics();

        assert!(diagnostics
            .iter()
            .any(|item| item.diagnostic.code == "motion-diagnostic-time-domain-required"));
        assert!(diagnostics
            .iter()
            .any(|item| item.diagnostic.code == "motion-diagnostic-trigger-element-required"));
    }
}
