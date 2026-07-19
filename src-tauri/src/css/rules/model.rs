use serde::Serialize;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CssProperty {
    pub property: String,
    pub value: String,
}
