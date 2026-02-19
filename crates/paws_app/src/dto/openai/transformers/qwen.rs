use paws_domain::{ModelId, Transformer};

use crate::dto::openai::Request;

/// makes the Request compatible with the Qwen API.
pub struct QwenTransformer;

impl Transformer for QwenTransformer {
    type Value = Request;

    fn transform(&mut self, mut request: Self::Value) -> Self::Value {
        // Map internal model IDs to DashScope model names
        if let Some(model) = &request.model {
            match model.as_str() {
                "coder-model" => {
                    request.model = Some(ModelId::from("qwen3-coder-plus"));
                }
                "vision-model" => {
                    request.model = Some(ModelId::from("qwen-vl-plus"));
                }
                _ => {}
            }
        }

        request
    }
}
