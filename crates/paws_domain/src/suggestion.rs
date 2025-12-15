use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Suggestion {
    pub use_case: String,
    pub suggestion: String,
}
