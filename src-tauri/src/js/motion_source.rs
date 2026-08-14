use serde::{Deserialize, Serialize};

use super::{
    motion_model::{MotionBehavior, MotionCustomCode, MotionInteraction, MOTION_SCHEMA_VERSION},
    MotionDocument, MotionRuntimeContract, PageJsConfig,
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PortableMotionDocument {
    schema_version: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    interactions: Vec<MotionInteraction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    behaviors: Vec<MotionBehavior>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    custom_code: Vec<MotionCustomCode>,
}

impl PortableMotionDocument {
    fn from_runtime(document: &MotionDocument) -> Self {
        Self {
            schema_version: document.schema_version,
            interactions: document.interactions.clone(),
            behaviors: document.behaviors.clone(),
            custom_code: document.custom_code.clone(),
        }
    }

    fn into_runtime(self) -> Result<MotionDocument, String> {
        if self.schema_version != MOTION_SCHEMA_VERSION {
            return Err(format!(
                "Documentul Motion portabil folosește schema {}, iar aplicația cere {}.",
                self.schema_version, MOTION_SCHEMA_VERSION
            ));
        }
        let document = MotionDocument {
            schema_version: self.schema_version,
            anime_version: MotionRuntimeContract::current().anime_version,
            interactions: self.interactions,
            behaviors: self.behaviors,
            custom_code: self.custom_code,
        };
        document.validate()?;
        Ok(document)
    }
}

pub fn parse_motion_source(source: &str) -> Result<PageJsConfig, String> {
    let portable = serde_json::from_str::<PortableMotionDocument>(source)
        .map_err(|error| format!("Sursa Motion portabilă nu este JSON valid: {error}"))?;
    let document = portable.into_runtime()?;
    if document.is_empty() {
        return Err(
            "Sursa Motion portabilă este goală; fișierul trebuie eliminat, nu păstrat gol."
                .to_string(),
        );
    }
    Ok(PageJsConfig {
        motion: Some(document),
    })
}

pub fn serialize_motion_source(config: &PageJsConfig) -> Result<Option<String>, String> {
    let Some(document) = config
        .motion
        .as_ref()
        .filter(|document| !document.is_empty())
    else {
        return Ok(None);
    };
    document.validate()?;
    let portable = PortableMotionDocument::from_runtime(document);
    let mut source = serde_json::to_string_pretty(&portable)
        .map_err(|error| format!("Sursa Motion portabilă nu poate fi serializată: {error}"))?;
    source.push('\n');
    Ok(Some(source))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::js::{MotionAction, MotionTarget, MotionTargetKind};

    #[test]
    fn portable_source_omits_runtime_version_and_roundtrips() {
        let mut document = MotionDocument::default();
        document.interactions.push(MotionInteraction {
            id: "hero".to_string(),
            name: "Hero".to_string(),
            enabled: true,
            trigger: crate::js::motion_model::MotionTrigger::default(),
            trigger_target: MotionTarget {
                kind: MotionTargetKind::Document,
                ..MotionTarget::default()
            },
            conditions: Default::default(),
            playback: Default::default(),
            domain: Default::default(),
            actions: vec![MotionAction::Call(
                crate::js::motion_model::MotionCallAction {
                    id: "call".to_string(),
                    name: "Call".to_string(),
                    enabled: true,
                    start: 0.0,
                    code: "window.__portable=true".to_string(),
                },
            )],
            markers: Vec::new(),
        });
        let config = PageJsConfig {
            motion: Some(document),
        };

        let source = serialize_motion_source(&config).unwrap().unwrap();
        assert!(!source.contains("animeVersion"));
        assert_eq!(parse_motion_source(&source).unwrap(), config);
    }

    #[test]
    fn empty_motion_has_no_portable_file() {
        assert_eq!(
            serialize_motion_source(&PageJsConfig::default()).unwrap(),
            None
        );
    }
}
