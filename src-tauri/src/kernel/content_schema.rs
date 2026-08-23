use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentFieldKind {
    Text,
    Textarea,
    Markdown,
    Number,
    Boolean,
    Date,
    Select,
    Url,
    Color,
    Image,
    Group,
    Repeater,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentFieldChoice {
    pub value: String,
    pub label: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentFieldDefinition {
    #[serde(default)]
    pub id: String,
    pub key: String,
    pub label: String,
    pub kind: ContentFieldKind,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub help: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_value: Option<serde_json::Value>,
    #[serde(default)]
    pub choices: Vec<ContentFieldChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(default)]
    pub fields: Vec<ContentFieldDefinition>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentModelDefinition {
    pub schema_version: u32,
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub fields: Vec<ContentFieldDefinition>,
    #[serde(skip, default)]
    pub file: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomFieldTemplateUsage {
    pub model_id: String,
    pub field_id: String,
    pub field_key: String,
    pub template_file: String,
    pub expression: String,
    pub offset: usize,
}
