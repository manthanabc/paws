use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Default, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(transparent)]
pub struct Template<V> {
    pub template: String,
    _marker: std::marker::PhantomData<V>,
}

/// Template type that wraps a string template and a phantom type for
/// validation.
///
/// The JsonSchema implementation always returns the schema for a String,
/// regardless of the generic type `T`. This is intentional because templates
/// are serialized and deserialized as strings. The generic type `T` is used for
/// type safety at compile time but does not affect the schema representation.
impl<T> JsonSchema for Template<T> {
    fn schema_name() -> Cow<'static, str> {
        String::schema_name()
    }

    fn json_schema(r#gen: &mut SchemaGenerator) -> Schema {
        String::json_schema(r#gen)
    }
}

impl<V> Template<V> {
    pub fn new(template: impl ToString) -> Self {
        Self {
            template: template.to_string(),
            _marker: std::marker::PhantomData,
        }
    }
}

impl<S: AsRef<str>> From<S> for Template<Value> {
    fn from(value: S) -> Self {
        Template::new(value.as_ref())
    }
}
