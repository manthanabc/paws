use chrono::Utc;
use derive_setters::Setters;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PointId(Uuid);

impl PointId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn into_uuid(self) -> Uuid {
        self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Point<C> {
    pub id: PointId,
    pub content: C,
    pub embedding: Vec<f32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl<C> Point<C> {
    /// Embedding can be created from a part or more of the actual content.
    pub fn new(content: C, embedding: Vec<f32>) -> Self {
        let now = Utc::now();
        Self {
            id: PointId::generate(),
            content,
            embedding,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn try_map<D, E>(
        self,
        f: impl FnOnce(C) -> std::result::Result<D, E>,
    ) -> std::result::Result<Point<D>, E> {
        Ok(Point {
            content: f(self.content)?,
            id: self.id,
            embedding: self.embedding,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

#[derive(Debug, Clone, Setters)]
#[setters(strip_option, into)]
pub struct Query {
    pub embedding: Vec<f32>,
    pub limit: Option<u64>,
    pub distance: Option<f32>,
}

impl Query {
    pub fn new(embedding: Vec<f32>) -> Self {
        Self { embedding, limit: None, distance: None }
    }
}
