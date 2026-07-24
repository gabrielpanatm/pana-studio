use std::collections::BTreeMap;

use serde::Serialize;
use tera::{
    ast::{
        Expr, ExprVal, FilterSection, Forloop, FunctionCall, If, In, MacroCall, MacroDefinition,
        Node, Set, StringConcat, Test,
    },
    Template,
};

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeraSemanticDocument {
    pub nodes: Vec<TeraSemanticNode>,
}

impl TeraSemanticDocument {
    pub(crate) fn from_template(template: &Template) -> Self {
        Self {
            nodes: template
                .ast
                .iter()
                .map(TeraSemanticNode::from_ast)
                .collect(),
        }
    }

    pub(crate) fn walk(&self) -> Vec<&TeraSemanticNode> {
        let mut result = Vec::new();
        for node in &self.nodes {
            node.walk_into(&mut result);
        }
        result
    }

    pub(crate) fn template_facts(&self) -> TeraTemplateFacts {
        let mut facts = TeraTemplateFacts::default();
        for node in self.walk() {
            match node {
                TeraSemanticNode::Extends { template } => {
                    if facts.extends.is_none() {
                        facts.extends = Some(template.clone());
                    }
                }
                TeraSemanticNode::Include {
                    templates,
                    ignore_missing,
                } => {
                    push_unique_all(&mut facts.includes, templates);
                    facts.include_groups.push(TeraIncludeFact {
                        targets: templates.clone(),
                        ignore_missing: *ignore_missing,
                    });
                }
                TeraSemanticNode::Import { template, .. } => {
                    push_unique(&mut facts.imports, template);
                }
                TeraSemanticNode::Block { name, .. } => push_unique(&mut facts.blocks, name),
                TeraSemanticNode::MacroDefinition { name, .. } => {
                    push_unique(&mut facts.macros, name);
                }
                _ => {}
            }
        }
        facts
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TeraTemplateFacts {
    pub(crate) extends: Option<String>,
    pub(crate) includes: Vec<String>,
    pub(crate) include_groups: Vec<TeraIncludeFact>,
    pub(crate) imports: Vec<String>,
    pub(crate) blocks: Vec<String>,
    pub(crate) macros: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TeraIncludeFact {
    pub(crate) targets: Vec<String>,
    pub(crate) ignore_missing: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TeraSemanticNode {
    Super,
    Text {
        value: String,
    },
    Variable {
        expression: TeraSemanticExpression,
    },
    MacroDefinition {
        name: String,
        arguments: BTreeMap<String, Option<TeraSemanticExpression>>,
        body: Vec<TeraSemanticNode>,
    },
    Extends {
        template: String,
    },
    Include {
        templates: Vec<String>,
        ignore_missing: bool,
    },
    Import {
        template: String,
        namespace: String,
    },
    Set {
        key: String,
        global: bool,
        value: TeraSemanticExpression,
    },
    Raw {
        value: String,
    },
    FilterSection {
        filter: TeraSemanticCall,
        body: Vec<TeraSemanticNode>,
    },
    Block {
        name: String,
        body: Vec<TeraSemanticNode>,
    },
    For {
        key: Option<String>,
        value: String,
        container: TeraSemanticExpression,
        body: Vec<TeraSemanticNode>,
        empty_body: Option<Vec<TeraSemanticNode>>,
    },
    If {
        branches: Vec<TeraSemanticBranch>,
        otherwise: Option<Vec<TeraSemanticNode>>,
    },
    Break,
    Continue,
    Comment {
        value: String,
    },
}

impl TeraSemanticNode {
    fn from_ast(node: &Node) -> Self {
        match node {
            Node::Super => Self::Super,
            Node::Text(value) => Self::Text {
                value: value.clone(),
            },
            Node::VariableBlock(_, expression) => Self::Variable {
                expression: expression.into(),
            },
            Node::MacroDefinition(_, definition, _) => Self::from_macro_definition(definition),
            Node::Extends(_, template) => Self::Extends {
                template: template.clone(),
            },
            Node::Include(_, templates, ignore_missing) => Self::Include {
                templates: templates.clone(),
                ignore_missing: *ignore_missing,
            },
            Node::ImportMacro(_, template, namespace) => Self::Import {
                template: template.clone(),
                namespace: namespace.clone(),
            },
            Node::Set(_, set) => Self::from_set(set),
            Node::Raw(_, value, _) => Self::Raw {
                value: value.clone(),
            },
            Node::FilterSection(_, section, _) => Self::from_filter_section(section),
            Node::Block(_, block, _) => Self::Block {
                name: block.name.clone(),
                body: block.body.iter().map(Self::from_ast).collect(),
            },
            Node::Forloop(_, forloop, _) => Self::from_forloop(forloop),
            Node::If(if_node, _) => Self::from_if(if_node),
            Node::Break(_) => Self::Break,
            Node::Continue(_) => Self::Continue,
            Node::Comment(_, value) => Self::Comment {
                value: value.clone(),
            },
        }
    }

    fn from_macro_definition(definition: &MacroDefinition) -> Self {
        Self::MacroDefinition {
            name: definition.name.clone(),
            arguments: sorted_optional_expressions(&definition.args),
            body: definition.body.iter().map(Self::from_ast).collect(),
        }
    }

    fn from_set(set: &Set) -> Self {
        Self::Set {
            key: set.key.clone(),
            global: set.global,
            value: (&set.value).into(),
        }
    }

    fn from_filter_section(section: &FilterSection) -> Self {
        Self::FilterSection {
            filter: (&section.filter).into(),
            body: section.body.iter().map(Self::from_ast).collect(),
        }
    }

    fn from_forloop(forloop: &Forloop) -> Self {
        Self::For {
            key: forloop.key.clone(),
            value: forloop.value.clone(),
            container: (&forloop.container).into(),
            body: forloop.body.iter().map(Self::from_ast).collect(),
            empty_body: forloop
                .empty_body
                .as_ref()
                .map(|body| body.iter().map(Self::from_ast).collect()),
        }
    }

    fn from_if(if_node: &If) -> Self {
        Self::If {
            branches: if_node
                .conditions
                .iter()
                .map(|(_, condition, body)| TeraSemanticBranch {
                    condition: condition.into(),
                    body: body.iter().map(Self::from_ast).collect(),
                })
                .collect(),
            otherwise: if_node
                .otherwise
                .as_ref()
                .map(|(_, body)| body.iter().map(Self::from_ast).collect()),
        }
    }

    fn walk_into<'a>(&'a self, result: &mut Vec<&'a TeraSemanticNode>) {
        result.push(self);
        match self {
            Self::MacroDefinition { body, .. }
            | Self::FilterSection { body, .. }
            | Self::Block { body, .. } => walk_nodes(body, result),
            Self::For {
                body, empty_body, ..
            } => {
                walk_nodes(body, result);
                if let Some(empty_body) = empty_body {
                    walk_nodes(empty_body, result);
                }
            }
            Self::If {
                branches,
                otherwise,
            } => {
                for branch in branches {
                    walk_nodes(&branch.body, result);
                }
                if let Some(otherwise) = otherwise {
                    walk_nodes(otherwise, result);
                }
            }
            _ => {}
        }
    }
}

fn walk_nodes<'a>(nodes: &'a [TeraSemanticNode], result: &mut Vec<&'a TeraSemanticNode>) {
    for node in nodes {
        node.walk_into(result);
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeraSemanticBranch {
    pub condition: TeraSemanticExpression,
    pub body: Vec<TeraSemanticNode>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeraSemanticExpression {
    pub value: TeraSemanticValue,
    pub negated: bool,
    pub filters: Vec<TeraSemanticCall>,
}

impl From<&Expr> for TeraSemanticExpression {
    fn from(expression: &Expr) -> Self {
        Self {
            value: (&expression.val).into(),
            negated: expression.negated,
            filters: expression.filters.iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum TeraSemanticValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Identifier(String),
    Math {
        operator: String,
        left: Box<TeraSemanticExpression>,
        right: Box<TeraSemanticExpression>,
    },
    Logic {
        operator: String,
        left: Box<TeraSemanticExpression>,
        right: Box<TeraSemanticExpression>,
    },
    Test {
        identifier: String,
        name: String,
        negated: bool,
        arguments: Vec<TeraSemanticExpression>,
    },
    MacroCall(TeraSemanticCall),
    FunctionCall(TeraSemanticCall),
    Array(Vec<TeraSemanticExpression>),
    StringConcat(Vec<TeraSemanticValue>),
    In {
        negated: bool,
        needle: Box<TeraSemanticExpression>,
        haystack: Box<TeraSemanticExpression>,
    },
}

impl From<&ExprVal> for TeraSemanticValue {
    fn from(value: &ExprVal) -> Self {
        match value {
            ExprVal::String(value) => Self::String(value.clone()),
            ExprVal::Int(value) => Self::Integer(*value),
            ExprVal::Float(value) => Self::Float(*value),
            ExprVal::Bool(value) => Self::Boolean(*value),
            ExprVal::Ident(value) => Self::Identifier(value.clone()),
            ExprVal::Math(expression) => Self::Math {
                operator: expression.operator.to_string(),
                left: Box::new(expression.lhs.as_ref().into()),
                right: Box::new(expression.rhs.as_ref().into()),
            },
            ExprVal::Logic(expression) => Self::Logic {
                operator: expression.operator.to_string(),
                left: Box::new(expression.lhs.as_ref().into()),
                right: Box::new(expression.rhs.as_ref().into()),
            },
            ExprVal::Test(test) => Self::from_test(test),
            ExprVal::MacroCall(call) => Self::MacroCall(call.into()),
            ExprVal::FunctionCall(call) => Self::FunctionCall(call.into()),
            ExprVal::Array(values) => {
                Self::Array(values.iter().map(TeraSemanticExpression::from).collect())
            }
            ExprVal::StringConcat(concat) => Self::from_concat(concat),
            ExprVal::In(expression) => Self::from_in(expression),
        }
    }
}

impl TeraSemanticValue {
    fn from_test(test: &Test) -> Self {
        Self::Test {
            identifier: test.ident.clone(),
            name: test.name.clone(),
            negated: test.negated,
            arguments: test.args.iter().map(Into::into).collect(),
        }
    }

    fn from_concat(concat: &StringConcat) -> Self {
        Self::StringConcat(concat.values.iter().map(Into::into).collect())
    }

    fn from_in(expression: &In) -> Self {
        Self::In {
            negated: expression.negated,
            needle: Box::new(expression.lhs.as_ref().into()),
            haystack: Box::new(expression.rhs.as_ref().into()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeraSemanticCall {
    pub namespace: Option<String>,
    pub name: String,
    pub arguments: BTreeMap<String, TeraSemanticExpression>,
}

impl From<&FunctionCall> for TeraSemanticCall {
    fn from(call: &FunctionCall) -> Self {
        Self {
            namespace: None,
            name: call.name.clone(),
            arguments: sorted_expressions(&call.args),
        }
    }
}

impl From<&MacroCall> for TeraSemanticCall {
    fn from(call: &MacroCall) -> Self {
        Self {
            namespace: Some(call.namespace.clone()),
            name: call.name.clone(),
            arguments: sorted_expressions(&call.args),
        }
    }
}

fn sorted_expressions(
    values: &std::collections::HashMap<String, Expr>,
) -> BTreeMap<String, TeraSemanticExpression> {
    values
        .iter()
        .map(|(key, value)| (key.clone(), value.into()))
        .collect()
}

fn sorted_optional_expressions(
    values: &std::collections::HashMap<String, Option<Expr>>,
) -> BTreeMap<String, Option<TeraSemanticExpression>> {
    values
        .iter()
        .map(|(key, value)| (key.clone(), value.as_ref().map(Into::into)))
        .collect()
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|candidate| candidate == value) {
        values.push(value.to_string());
    }
}

fn push_unique_all(values: &mut Vec<String>, candidates: &[String]) {
    for candidate in candidates {
        push_unique(values, candidate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_the_embedded_tera_ast_without_losing_semantics() {
        let source = r#"{% import "macros/cards.html" as cards %}
{% macro render(item, class="card") %}
{% set_global seen = seen + 1 %}
{% if item.title is defined and item.visible %}
{{ cards::title(value=item.title | upper, class=class) }}
{% else %}
{% set fallback = load_data(path="date/fallback.toml") %}
{{ fallback.title }}
{% endif %}
{% endmacro %}
"#;
        let template = Template::new("semantic.html", None, source).expect("valid Tera");
        let semantic = TeraSemanticDocument::from_template(&template);
        let walked = semantic.walk();

        assert!(walked.iter().any(|node| matches!(
            node,
            TeraSemanticNode::Import {
                template,
                namespace
            } if template == "macros/cards.html" && namespace == "cards"
        )));
        assert!(walked.iter().any(|node| matches!(
            node,
            TeraSemanticNode::Set {
                key,
                global: true,
                ..
            } if key == "seen"
        )));
        assert!(walked
            .iter()
            .any(|node| matches!(node, TeraSemanticNode::If { .. })));

        let macro_node = walked
            .iter()
            .find_map(|node| match node {
                TeraSemanticNode::MacroDefinition {
                    name, arguments, ..
                } => Some((name, arguments)),
                _ => None,
            })
            .expect("macro definition");
        assert_eq!(macro_node.0, "render");
        assert!(macro_node.1.contains_key("item"));
        assert!(macro_node.1.contains_key("class"));

        let facts = semantic.template_facts();
        assert_eq!(facts.imports, vec!["macros/cards.html"]);
        assert_eq!(facts.macros, vec!["render"]);
    }
}
