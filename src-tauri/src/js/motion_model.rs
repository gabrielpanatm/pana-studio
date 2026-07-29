use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::localization::LocalizedDiagnostic;

pub const MOTION_SCHEMA_VERSION: u32 = 2;
pub const MOTION_ANIME_VERSION: &str = "4.4.1";

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
#[serde(rename_all = "camelCase")]
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
            anime_version: MOTION_ANIME_VERSION.to_string(),
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
        if value.is_null() {
            return Ok(Self::default());
        }
        let schema_version = value
            .get("schemaVersion")
            .and_then(Value::as_u64)
            .unwrap_or(1);
        let document = if schema_version >= MOTION_SCHEMA_VERSION as u64 {
            serde_json::from_value(value)
                .map_err(|error| format!("Configurația Motion v2 este invalidă: {error}"))?
        } else {
            migrate_legacy_motion(value)?
        };
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
        if self.anime_version != MOTION_ANIME_VERSION {
            diagnostics.push(MotionDiagnostic::error(
                "motion.anime_version",
                LocalizedDiagnostic::new("motion-diagnostic-anime-version")
                    .with_argument("expected", MOTION_ANIME_VERSION)
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
#[serde(tag = "type", rename_all = "camelCase")]
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
#[serde(tag = "type", rename_all = "camelCase")]
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
#[serde(tag = "type", rename_all = "camelCase")]
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

fn migrate_legacy_motion(value: Value) -> Result<MotionDocument, String> {
    let items = value
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let by_id = items
        .iter()
        .filter_map(|item| {
            item.get("id")
                .and_then(Value::as_str)
                .map(|id| (id.to_string(), item.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    let referenced_animation_ids = items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("timeline"))
        .flat_map(|timeline| {
            timeline
                .get("steps")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        })
        .filter(|step| step.get("type").and_then(Value::as_str) == Some("animation"))
        .filter_map(|step| {
            step.get("targetItemId")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect::<BTreeSet<_>>();

    let mut document = MotionDocument::default();
    for timeline in items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("timeline"))
    {
        if let Some(interaction) = migrate_legacy_timeline(timeline, &by_id) {
            document.interactions.push(interaction);
        }
    }
    for animation in items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("animation"))
    {
        let id = legacy_string(animation, "id", "");
        if referenced_animation_ids.contains(&id) {
            continue;
        }
        if let Some(interaction) = migrate_legacy_animation(animation, None, 0.0, None) {
            document.interactions.push(interaction);
        }
    }
    for item in &items {
        match item.get("type").and_then(Value::as_str) {
            Some("draggable") => document
                .behaviors
                .push(MotionBehavior::Draggable(migrate_legacy_draggable(item))),
            Some("layout") => document
                .behaviors
                .push(MotionBehavior::Layout(migrate_legacy_layout(item))),
            Some("custom") => document.custom_code.push(MotionCustomCode {
                id: legacy_string(item, "id", "custom"),
                name: legacy_string(item, "name", "Cod personalizat"),
                enabled: legacy_bool(item, "enabled", true),
                code: legacy_string(item, "code", ""),
            }),
            Some("interaction") => {
                if let Some(interaction) = migrate_legacy_basic_interaction(item) {
                    document.interactions.push(interaction);
                }
            }
            _ => {}
        }
    }
    Ok(document)
}

fn migrate_legacy_timeline(
    timeline: &Value,
    by_id: &BTreeMap<String, Value>,
) -> Option<MotionInteraction> {
    let timeline_id = legacy_string(timeline, "id", "interaction");
    let steps = timeline.get("steps").and_then(Value::as_array)?;
    let mut labels = BTreeMap::new();
    for label in timeline
        .get("labels")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
    {
        let name = legacy_string(&label, "name", "");
        let position = legacy_string(&label, "position", "0");
        labels.insert(name, position.parse::<f64>().unwrap_or(0.0));
    }
    let mut previous_start = 0.0;
    let mut previous_end = 0.0;
    let mut cursor_end = 0.0;
    let mut actions = Vec::new();
    let mut markers = Vec::new();
    let mut first_animation: Option<Value> = None;

    for (index, step) in steps.iter().enumerate() {
        let start = legacy_position(
            step.get("position").and_then(Value::as_str),
            previous_start,
            previous_end,
            cursor_end,
            &labels,
        );
        let duration = step.get("duration").and_then(Value::as_f64).unwrap_or(0.0);
        match step.get("type").and_then(Value::as_str) {
            Some("animation") => {
                let target_id = legacy_string(step, "targetItemId", "");
                if let Some(animation) = by_id.get(&target_id) {
                    first_animation.get_or_insert_with(|| animation.clone());
                    if let Some(mut migrated) =
                        migrate_legacy_animate_action(animation, Some(step), start, index)
                    {
                        migrated.duration = if duration > 0.0 {
                            duration
                        } else {
                            migrated.duration
                        };
                        previous_start = start;
                        previous_end = start + migrated.duration;
                        cursor_end = cursor_end.max(previous_end);
                        actions.push(MotionAction::Animate(migrated));
                    }
                }
            }
            Some("set") => {
                let target_id = legacy_string(step, "targetItemId", "");
                if let Some(target_item) = by_id.get(&target_id) {
                    actions.push(MotionAction::Set(MotionSetAction {
                        id: legacy_string(step, "id", &format!("set-{index}")),
                        name: legacy_string(step, "label", "Setează"),
                        enabled: true,
                        target: migrate_legacy_target(target_item),
                        start,
                        values: migrate_legacy_set_values(target_item),
                    }));
                    previous_start = start;
                    previous_end = start;
                }
            }
            Some("callback") => {
                let callback = step.get("callback").unwrap_or(&Value::Null);
                actions.push(MotionAction::Call(MotionCallAction {
                    id: legacy_string(step, "id", &format!("call-{index}")),
                    name: legacy_string(step, "label", "Callback"),
                    enabled: callback
                        .get("enabled")
                        .and_then(Value::as_bool)
                        .unwrap_or(true),
                    start,
                    code: legacy_string(callback, "code", ""),
                }));
                previous_start = start;
                previous_end = start;
            }
            Some("label") => {
                markers.push(MotionMarker {
                    id: legacy_string(step, "id", &format!("marker-{index}")),
                    name: legacy_string(step, "label", &format!("Marker {}", index + 1)),
                    at: start,
                });
            }
            Some("timer") => {
                previous_start = start;
                previous_end = start + duration;
                cursor_end = cursor_end.max(previous_end);
            }
            _ => {}
        }
    }

    if actions.is_empty() {
        return None;
    }
    let trigger_source = first_animation.as_ref().unwrap_or(timeline);
    let trigger = migrate_legacy_trigger(trigger_source);
    let domain = if matches!(
        trigger,
        MotionTrigger::Scroll {
            mode: MotionScrollMode::Scrub,
            ..
        } | MotionTrigger::Pointer { .. }
    ) {
        MotionTimelineDomain::Progress
    } else {
        MotionTimelineDomain::Time
    };
    if domain == MotionTimelineDomain::Progress {
        normalize_actions_to_progress(&mut actions);
        normalize_markers_to_progress(&mut markers, cursor_end);
    }
    if actions.is_empty() {
        return None;
    }
    let playback = if domain == MotionTimelineDomain::Progress {
        MotionPlayback::default()
    } else {
        migrate_legacy_playback(timeline)
    };
    Some(MotionInteraction {
        id: timeline_id,
        name: legacy_string(timeline, "name", "Interacțiune"),
        enabled: legacy_bool(timeline, "enabled", true),
        trigger,
        trigger_target: migrate_legacy_target(trigger_source),
        conditions: MotionConditions::default(),
        playback,
        domain,
        actions,
        markers,
    })
}

fn migrate_legacy_animation(
    animation: &Value,
    step: Option<&Value>,
    start: f64,
    index: Option<usize>,
) -> Option<MotionInteraction> {
    let action = migrate_legacy_animate_action(animation, step, start, index.unwrap_or_default())?;
    let trigger = migrate_legacy_trigger(animation);
    let domain = if matches!(
        trigger,
        MotionTrigger::Scroll {
            mode: MotionScrollMode::Scrub,
            ..
        } | MotionTrigger::Pointer { .. }
    ) {
        MotionTimelineDomain::Progress
    } else {
        MotionTimelineDomain::Time
    };
    let mut actions = vec![MotionAction::Animate(action)];
    if domain == MotionTimelineDomain::Progress {
        normalize_actions_to_progress(&mut actions);
    }
    if actions.is_empty() {
        return None;
    }
    Some(MotionInteraction {
        id: legacy_string(animation, "id", "interaction"),
        name: legacy_string(animation, "name", "Interacțiune"),
        enabled: legacy_bool(animation, "enabled", true),
        trigger,
        trigger_target: migrate_legacy_target(animation),
        conditions: MotionConditions::default(),
        playback: MotionPlayback::default(),
        domain,
        actions,
        markers: Vec::new(),
    })
}

fn migrate_legacy_animate_action(
    animation: &Value,
    step: Option<&Value>,
    start: f64,
    index: usize,
) -> Option<MotionAnimateAction> {
    let properties = animation
        .get("properties")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .iter()
        .enumerate()
        .filter_map(|(property_index, property)| migrate_legacy_property(property, property_index))
        .collect::<Vec<_>>();
    let keyframes = animation
        .get("keyframes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .iter()
        .enumerate()
        .map(|(frame_index, frame)| MotionKeyframe {
            id: legacy_string(frame, "id", &format!("frame-{frame_index}")),
            offset: legacy_percent(frame.get("at")).unwrap_or({
                if frame_index == 0 {
                    0.0
                } else {
                    100.0
                }
            }),
            ease: legacy_string(frame, "ease", ""),
            properties: frame
                .get("properties")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .iter()
                .enumerate()
                .filter_map(|(property_index, property)| {
                    migrate_legacy_property(property, property_index)
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    if properties.is_empty() && keyframes.is_empty() {
        return None;
    }
    let playback = animation.get("playback").unwrap_or(&Value::Null);
    let duration = step
        .and_then(|value| value.get("duration"))
        .and_then(Value::as_f64)
        .filter(|duration| *duration > 0.0)
        .or_else(|| playback.get("duration").and_then(Value::as_f64))
        .filter(|duration| *duration > 0.0)
        .unwrap_or_else(default_duration_ms);
    let has_from = properties.iter().any(|property| property.from.is_some());
    Some(MotionAnimateAction {
        id: step
            .map(|value| legacy_string(value, "id", &format!("action-{index}")))
            .unwrap_or_else(|| format!("{}-action", legacy_string(animation, "id", "animation"))),
        name: step
            .map(|value| legacy_string(value, "label", "Animație"))
            .unwrap_or_else(|| legacy_string(animation, "name", "Animație")),
        enabled: legacy_bool(animation, "enabled", true),
        target: migrate_legacy_target(animation),
        start,
        duration,
        mode: if has_from {
            MotionAnimationMode::FromTo
        } else {
            MotionAnimationMode::To
        },
        ease: properties
            .first()
            .and_then(|_| {
                animation
                    .get("properties")
                    .and_then(Value::as_array)
                    .and_then(|properties| properties.first())
                    .and_then(|property| property.get("tween"))
                    .and_then(|tween| tween.get("ease"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .filter(|ease| !ease.trim().is_empty())
            .unwrap_or_else(default_ease),
        properties,
        keyframes,
        stagger: migrate_legacy_stagger(animation.get("stagger")),
        repeat: MotionActionRepeat {
            count: playback
                .get("loop")
                .and_then(Value::as_u64)
                .unwrap_or_default() as u32,
            infinite: playback.get("loop").and_then(Value::as_i64) == Some(-1),
            alternate: playback
                .get("alternate")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            delay_ms: playback
                .get("loopDelay")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
        },
        specialization: migrate_legacy_specialization(animation),
    })
}

fn migrate_legacy_property(property: &Value, index: usize) -> Option<MotionProperty> {
    let name = legacy_string(property, "property", "");
    if name.is_empty() {
        return None;
    }
    let value = property.get("value").unwrap_or(&Value::Null);
    let unit = legacy_string(value, "unit", "");
    let kind = legacy_value_kind(legacy_string(value, "mode", "literal").as_str());
    let to_raw = value
        .get("to")
        .or_else(|| value.get("value"))
        .map(value_to_string)
        .unwrap_or_default();
    let from_raw = value
        .get("from")
        .map(value_to_string)
        .filter(|value| !value.is_empty());
    Some(MotionProperty {
        id: legacy_string(property, "id", &format!("property-{index}")),
        name,
        category: match legacy_string(property, "category", "transform").as_str() {
            "css" => MotionPropertyCategory::Style,
            "cssVariable" => MotionPropertyCategory::CssVariable,
            "htmlAttribute" => MotionPropertyCategory::HtmlAttribute,
            "svgAttribute" => MotionPropertyCategory::SvgAttribute,
            "object" => MotionPropertyCategory::Object,
            _ => MotionPropertyCategory::Transform,
        },
        from: from_raw.map(|value| MotionValue {
            kind,
            value,
            unit: unit.clone(),
        }),
        to: MotionValue {
            kind,
            value: to_raw,
            unit,
        },
    })
}

fn legacy_value_kind(mode: &str) -> MotionValueKind {
    match mode {
        "color" => MotionValueKind::Color,
        "cssVariable" => MotionValueKind::CssVariable,
        "relative" => MotionValueKind::Relative,
        "literal" | "fromTo" | "random" => MotionValueKind::Number,
        _ => MotionValueKind::Text,
    }
}

fn migrate_legacy_target(item: &Value) -> MotionTarget {
    let target = item.get("target").unwrap_or(&Value::Null);
    let data_anim = legacy_string(target, "dataAnim", "");
    let selector = legacy_string(target, "selector", "").trim().to_string();
    let mode = legacy_string(target, "mode", "");
    if !data_anim.is_empty() {
        MotionTarget::for_data_anim(data_anim)
    } else if !selector.is_empty() {
        MotionTarget {
            kind: MotionTargetKind::Selector,
            data_anim: String::new(),
            selector,
            relation: MotionTargetRelation::SelfElement,
            scope: MotionTargetScope::All,
        }
    } else if mode == "selected" || mode == "dataAnim" {
        MotionTarget::default()
    } else {
        MotionTarget {
            kind: MotionTargetKind::Document,
            ..MotionTarget::default()
        }
    }
}

fn migrate_legacy_trigger(item: &Value) -> MotionTrigger {
    match legacy_string(item, "trigger", "load").as_str() {
        "click" => MotionTrigger::Click {
            first_click: MotionTriggerCommand::Restart,
            second_click: MotionTriggerCommand::None,
            prevent_default: false,
        },
        "hover" => MotionTrigger::Hover {
            enter: MotionTriggerCommand::Restart,
            leave: MotionTriggerCommand::Reverse,
        },
        "scroll" if legacy_bool(item, "scrollScrub", false) => MotionTrigger::Scroll {
            mode: MotionScrollMode::Scrub,
            start: default_scroll_start(),
            end: default_scroll_end(),
            smooth_ms: 0.0,
            once: false,
        },
        "scroll" => MotionTrigger::InView {
            threshold: default_in_view_threshold(),
            once: !legacy_bool(item, "scrollRepeat", false),
        },
        _ => MotionTrigger::default(),
    }
}

fn migrate_legacy_playback(item: &Value) -> MotionPlayback {
    let playback = item.get("playback").unwrap_or(&Value::Null);
    let loop_value = playback.get("loop");
    MotionPlayback {
        delay_ms: playback
            .get("delay")
            .and_then(Value::as_f64)
            .unwrap_or_default(),
        repeat: loop_value.and_then(Value::as_u64).unwrap_or_default() as u32,
        infinite: loop_value.and_then(Value::as_i64) == Some(-1)
            || loop_value.and_then(Value::as_bool) == Some(true),
        loop_delay_ms: playback
            .get("loopDelay")
            .and_then(Value::as_f64)
            .unwrap_or_default(),
        alternate: playback
            .get("alternate")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        reversed: playback
            .get("reversed")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        playback_rate: playback
            .get("playbackRate")
            .and_then(Value::as_f64)
            .filter(|value| *value > 0.0)
            .unwrap_or_else(default_playback_rate),
        playback_ease: legacy_string(playback, "playbackEase", ""),
    }
}

fn migrate_legacy_stagger(value: Option<&Value>) -> Option<MotionStagger> {
    let stagger = value?;
    if !legacy_bool(stagger, "enabled", false) {
        return None;
    }
    let total = stagger
        .get("total")
        .and_then(Value::as_f64)
        .unwrap_or_default();
    Some(MotionStagger {
        amount: if total > 0.0 {
            total
        } else {
            stagger
                .get("each")
                .and_then(Value::as_f64)
                .unwrap_or_default()
        },
        mode: if total > 0.0 {
            MotionStaggerMode::Total
        } else {
            MotionStaggerMode::Each
        },
        from: legacy_string(stagger, "from", ""),
        reversed: legacy_bool(stagger, "reversed", false),
        ease: legacy_string(stagger, "ease", ""),
    })
}

fn migrate_legacy_specialization(item: &Value) -> Option<MotionSpecialization> {
    match legacy_string(item, "textEffect", "").as_str() {
        "lines" => Some(MotionSpecialization::SplitText {
            mode: MotionSplitTextMode::Lines,
        }),
        "words" => Some(MotionSpecialization::SplitText {
            mode: MotionSplitTextMode::Words,
        }),
        "chars" => Some(MotionSpecialization::SplitText {
            mode: MotionSplitTextMode::Chars,
        }),
        _ => None,
    }
}

fn migrate_legacy_set_values(item: &Value) -> Vec<MotionSetValue> {
    item.get("properties")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|property| {
            let property = migrate_legacy_property(property, 0)?;
            Some(MotionSetValue::Property {
                name: property.name,
                value: property.to,
            })
        })
        .collect()
}

fn migrate_legacy_draggable(item: &Value) -> MotionDraggableBehavior {
    MotionDraggableBehavior {
        id: legacy_string(item, "id", "draggable"),
        name: legacy_string(item, "name", "Draggable"),
        enabled: legacy_bool(item, "enabled", true),
        target: migrate_legacy_target(item),
        axis: match legacy_string(item, "axes", "both").as_str() {
            "x" => MotionDragAxis::X,
            "y" => MotionDragAxis::Y,
            _ => MotionDragAxis::Both,
        },
        container: legacy_string(item, "container", ""),
        snap: item
            .get("snap")
            .and_then(|value| {
                value
                    .as_f64()
                    .or_else(|| value.as_str().and_then(|source| source.parse().ok()))
            })
            .unwrap_or_default(),
        friction: item
            .get("friction")
            .and_then(Value::as_f64)
            .unwrap_or_else(default_drag_friction),
        cursor: legacy_bool(item, "cursor", true),
    }
}

fn migrate_legacy_layout(item: &Value) -> MotionLayoutBehavior {
    MotionLayoutBehavior {
        id: legacy_string(item, "id", "layout"),
        name: legacy_string(item, "name", "Layout"),
        enabled: legacy_bool(item, "enabled", true),
        target: migrate_legacy_target(item),
        children_selector: legacy_string(item, "children", ""),
        properties: legacy_string(item, "properties", "")
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect(),
        duration_ms: item
            .get("playback")
            .and_then(|playback| playback.get("duration"))
            .and_then(Value::as_f64)
            .unwrap_or_else(default_duration_ms),
        ease: default_ease(),
    }
}

fn migrate_legacy_basic_interaction(item: &Value) -> Option<MotionInteraction> {
    let action = legacy_string(item, "action", "");
    let class_name = legacy_string(item, "value", "");
    let target = MotionTarget {
        kind: MotionTargetKind::Selector,
        data_anim: String::new(),
        selector: legacy_string(item, "targetSelector", ""),
        relation: MotionTargetRelation::SelfElement,
        scope: MotionTargetScope::All,
    };
    let values = match action.as_str() {
        "addClass" => vec![MotionSetValue::AddClass { name: class_name }],
        "removeClass" => vec![MotionSetValue::RemoveClass { name: class_name }],
        "toggleClass" => vec![MotionSetValue::ToggleClass { name: class_name }],
        "show" => vec![MotionSetValue::Property {
            name: "display".to_string(),
            value: MotionValue {
                kind: MotionValueKind::Text,
                value: String::new(),
                unit: String::new(),
            },
        }],
        "hide" => vec![MotionSetValue::Property {
            name: "display".to_string(),
            value: MotionValue {
                kind: MotionValueKind::Text,
                value: "none".to_string(),
                unit: String::new(),
            },
        }],
        _ => return None,
    };
    if target.selector.trim().is_empty() {
        return None;
    }
    Some(MotionInteraction {
        id: legacy_string(item, "id", "interaction"),
        name: legacy_string(item, "name", "Interacțiune"),
        enabled: legacy_bool(item, "enabled", true),
        trigger: MotionTrigger::Custom {
            event: legacy_string(item, "event", "click"),
            prevent_default: false,
        },
        trigger_target: migrate_legacy_target(item),
        conditions: MotionConditions::default(),
        playback: MotionPlayback::default(),
        domain: MotionTimelineDomain::Time,
        actions: vec![MotionAction::Set(MotionSetAction {
            id: format!("{}-action", legacy_string(item, "id", "interaction")),
            name: "Setează".to_string(),
            enabled: true,
            target,
            start: 0.0,
            values,
        })],
        markers: Vec::new(),
    })
}

fn legacy_position(
    position: Option<&str>,
    previous_start: f64,
    previous_end: f64,
    cursor_end: f64,
    labels: &BTreeMap<String, f64>,
) -> f64 {
    let position = position.unwrap_or("").trim();
    if position.is_empty() {
        return cursor_end;
    }
    if let Ok(value) = position.parse::<f64>() {
        return value.max(0.0);
    }
    if let Some(value) = labels.get(position) {
        return *value;
    }
    if position == "<" {
        return previous_end;
    }
    if position == "<<" {
        return previous_start;
    }
    if let Some(offset) = position.strip_prefix("+=") {
        return cursor_end + offset.parse::<f64>().unwrap_or(0.0);
    }
    if let Some(offset) = position.strip_prefix("-=") {
        return (cursor_end - offset.parse::<f64>().unwrap_or(0.0)).max(0.0);
    }
    if let Some(offset) = position.strip_prefix("<<+=") {
        return previous_start + offset.parse::<f64>().unwrap_or(0.0);
    }
    if let Some(offset) = position.strip_prefix("<<-=") {
        return (previous_start - offset.parse::<f64>().unwrap_or(0.0)).max(0.0);
    }
    if let Some(offset) = position.strip_prefix("<+=") {
        return previous_end + offset.parse::<f64>().unwrap_or(0.0);
    }
    if let Some(offset) = position.strip_prefix("<-=") {
        return (previous_end - offset.parse::<f64>().unwrap_or(0.0)).max(0.0);
    }
    cursor_end
}

fn normalize_actions_to_progress(actions: &mut Vec<MotionAction>) {
    actions.retain_mut(|action| match action {
        MotionAction::Animate(action) => {
            action.repeat = MotionActionRepeat::default();
            true
        }
        MotionAction::Set(action) => {
            action
                .values
                .retain(|value| matches!(value, MotionSetValue::Property { .. }));
            !action.values.is_empty()
        }
        MotionAction::Media(_) | MotionAction::Call(_) => false,
        MotionAction::Nested(_) => true,
    });
    let max_end = actions
        .iter()
        .map(|action| action.start() + action.duration())
        .fold(0.0_f64, f64::max)
        .max(1.0);
    for action in actions {
        match action {
            MotionAction::Animate(action) => {
                action.start = action.start / max_end * 100.0;
                action.duration = action.duration / max_end * 100.0;
            }
            MotionAction::Set(action) => action.start = action.start / max_end * 100.0,
            MotionAction::Media(action) => action.start = action.start / max_end * 100.0,
            MotionAction::Call(action) => action.start = action.start / max_end * 100.0,
            MotionAction::Nested(action) => {
                action.start = action.start / max_end * 100.0;
                action.duration = action.duration / max_end * 100.0;
            }
        }
    }
}

fn normalize_markers_to_progress(markers: &mut [MotionMarker], duration: f64) {
    let duration = duration.max(1.0);
    for marker in markers {
        marker.at = marker.at / duration * 100.0;
    }
}

fn legacy_string(value: &Value, key: &str, fallback: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(fallback)
        .to_string()
}

fn legacy_bool(value: &Value, key: &str, fallback: bool) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(fallback)
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Null => String::new(),
        value => value.to_string(),
    }
}

fn legacy_percent(value: Option<&Value>) -> Option<f64> {
    let value = value?;
    if let Some(number) = value.as_f64() {
        return Some(number);
    }
    value
        .as_str()?
        .trim()
        .strip_suffix('%')?
        .parse::<f64>()
        .ok()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn property() -> Value {
        json!({
            "id": "property-opacity",
            "property": "opacity",
            "category": "css",
            "value": {
                "mode": "fromTo",
                "from": "0",
                "to": "1",
                "value": "",
                "unit": ""
            }
        })
    }

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
    fn migrates_standalone_v1_animation_to_interaction() {
        let document = MotionDocument::from_value(json!({
            "schemaVersion": 1,
            "animeVersion": "4.4.1",
            "activeItemId": "animation-a",
            "items": [{
                "id": "animation-a",
                "type": "animation",
                "name": "Hero",
                "enabled": true,
                "trigger": "load",
                "target": { "mode": "dataAnim", "dataAnim": "hero", "selector": "" },
                "properties": [property()],
                "keyframes": [],
                "playback": { "duration": 800 }
            }]
        }))
        .expect("migrated");

        assert_eq!(document.schema_version, 2);
        assert_eq!(document.interactions.len(), 1);
        assert_eq!(document.interactions[0].actions.len(), 1);
        assert!(document.behaviors.is_empty());
    }

    #[test]
    fn migrates_timeline_without_duplicating_referenced_animation() {
        let document = MotionDocument::from_value(json!({
            "schemaVersion": 1,
            "animeVersion": "4.4.1",
            "items": [
                {
                    "id": "animation-a",
                    "type": "animation",
                    "name": "Hero",
                    "enabled": true,
                    "trigger": "load",
                    "target": { "mode": "dataAnim", "dataAnim": "hero", "selector": "" },
                    "properties": [property()],
                    "keyframes": [],
                    "playback": { "duration": 800 }
                },
                {
                    "id": "timeline-a",
                    "type": "timeline",
                    "name": "Intro",
                    "enabled": true,
                    "steps": [{
                        "id": "step-a",
                        "type": "animation",
                        "position": "0",
                        "duration": 800,
                        "targetItemId": "animation-a"
                    }]
                }
            ]
        }))
        .expect("migrated");

        assert_eq!(document.interactions.len(), 1);
        assert_eq!(document.interactions[0].id, "timeline-a");
        assert_eq!(document.interactions[0].actions.len(), 1);
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
            "animeVersion": "4.4.1",
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
            "animeVersion": "4.4.1",
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
            "animeVersion": "4.4.1",
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
