use std::collections::{BTreeMap, HashSet};
use std::fmt::Display;

use serde::Serialize;
use serde_json::Value;

use crate::ToolDefinition;

pub struct ToolUsagePrompt<'a> {
    tools: &'a Vec<ToolDefinition>,
}

impl<'a> From<&'a Vec<ToolDefinition>> for ToolUsagePrompt<'a> {
    fn from(value: &'a Vec<ToolDefinition>) -> Self {
        Self { tools: value }
    }
}

impl Display for ToolUsagePrompt<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for tool in self.tools.iter() {
            // Extract required fields
            let required = tool
                .input_schema
                .get("required")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect::<HashSet<_>>()
                })
                .unwrap_or_default();

            // Extract properties
            let parameters = tool
                .input_schema
                .get("properties")
                .and_then(|v| v.as_object())
                .map(|props| {
                    props
                        .iter()
                        .filter_map(|(name, prop_schema)| {
                            let desc = prop_schema
                                .get("description")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();

                            let type_of = prop_schema.get("type").cloned();

                            Some((
                                name.clone(),
                                Parameter {
                                    description: desc,
                                    type_of,
                                    is_required: required.contains(name),
                                },
                            ))
                        })
                        .collect::<BTreeMap<_, _>>()
                })
                .unwrap_or_default();

            let schema = Schema {
                name: tool.name.to_string(),
                arguments: parameters,
                description: tool.description.clone(),
            };

            writeln!(f, "<tool>{schema}</tool>")?;
        }

        Ok(())
    }
}

#[derive(Serialize)]
struct Schema {
    name: String,
    description: String,
    arguments: BTreeMap<String, Parameter>,
}

#[derive(Serialize)]
struct Parameter {
    description: String,
    #[serde(rename = "type")]
    type_of: Option<Value>,
    is_required: bool,
}

impl Display for Schema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", serde_json::to_string(self).unwrap())
    }
}

#[cfg(test)]
mod tests {

    use insta::assert_snapshot;
    use strum::IntoEnumIterator;

    use super::*;
    use crate::ToolCatalog;

    #[test]
    fn test_tool_usage() {
        let tools = ToolCatalog::iter()
            .map(|v| v.definition())
            .collect::<Vec<_>>();
        let prompt = ToolUsagePrompt::from(&tools);
        assert_snapshot!(prompt);
    }
}
