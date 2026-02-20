use std::str::FromStr;

use chrono::{DateTime, Utc};
use derive_more::derive::Display;
use derive_setters::Setters;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Context, Error, Metrics, Result, TokenCount};

#[derive(Debug, Default, Display, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct ConversationId(Uuid);

impl Copy for ConversationId {}

impl ConversationId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn into_string(&self) -> String {
        self.0.to_string()
    }

    pub fn parse(value: impl ToString) -> Result<Self> {
        Ok(Self(
            Uuid::parse_str(&value.to_string()).map_err(Error::ConversationId)?,
        ))
    }
}

impl FromStr for ConversationId {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::parse(s)
    }
}

#[derive(Debug, Setters, Serialize, Deserialize, Clone)]
#[setters(into)]
pub struct Conversation {
    pub id: ConversationId,
    pub title: Option<String>,
    pub context: Option<Context>,
    pub metrics: Metrics,
    pub metadata: MetaData,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub context_history: Vec<ContextSnapshot>,
}

/// Represents a snapshot of the conversation context at a specific point in time.
/// Used to implement undo functionality by storing previous states.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContextSnapshot {
    /// The context state at this snapshot
    pub context: Context,
    /// When this snapshot was created
    pub timestamp: DateTime<Utc>,
    /// A brief summary of what this snapshot represents (e.g., the user's message)
    pub summary: String,
}

#[derive(Debug, Setters, Serialize, Deserialize, Clone)]
#[setters(into)]
pub struct MetaData {
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

impl MetaData {
    pub fn new(created_at: DateTime<Utc>) -> Self {
        Self { created_at, updated_at: None }
    }
}

impl Conversation {
    pub fn new(id: ConversationId) -> Self {
        let created_at = Utc::now();
        let metrics = Metrics::default().started_at(created_at);
        Self {
            id,
            metrics,
            metadata: MetaData::new(created_at),
            title: None,
            context: None,
            context_history: Vec::new(),
        }
    }
    /// Creates a new conversation with a new conversation ID.
    ///
    /// This is a convenience constructor that automatically generates a unique
    /// conversation ID, making it easy to create new conversations without
    /// having to manually create the ID.
    pub fn generate() -> Self {
        Self::new(ConversationId::generate())
    }

    /// Generates an HTML representation of the conversation
    ///
    /// This method uses Handlebars to render the conversation as HTML
    /// from the template file, including all agents, events, and variables.
    ///
    /// # Errors
    /// - If the template file cannot be found or read
    /// - If the Handlebars template registration fails
    /// - If the template rendering fails
    pub fn to_html(&self) -> String {
        // Instead of using Handlebars, we now use our Element DSL
        crate::conversation_html::render_conversation_html(self)
    }

    /// Returns a vector of user messages, selecting the first message from
    /// each consecutive sequence of user messages.
    pub fn first_user_messages(&self) -> Vec<&crate::ContextMessage> {
        self.context
            .as_ref()
            .map(|ctx| ctx.first_user_messages())
            .unwrap_or_default()
    }

    /// Returns the total token usage across all messages in the conversation.
    ///
    /// This is a convenience method that aggregates usage from the context,
    /// if available.
    pub fn accumulated_usage(&self) -> Option<crate::Usage> {
        self.context.as_ref().and_then(|ctx| ctx.accumulate_usage())
    }

    pub fn usage(&self) -> Option<crate::Usage> {
        self.context
            .iter()
            .flat_map(|ctx| ctx.messages.iter())
            .flat_map(|msg| msg.usage.into_iter())
            .last()
    }

    pub fn token_count(&self) -> Option<TokenCount> {
        self.context.as_ref().map(|ctx| ctx.token_count())
    }

    pub fn accumulated_cost(&self) -> Option<f64> {
        self.accumulated_usage().and_then(|usage| usage.cost)
    }

    /// Creates a snapshot of the current context state.
    ///
    /// # Arguments
    /// * `summary` - A brief description of this snapshot (e.g., the user's message)
    ///
    /// # Returns
    /// `Some(ContextSnapshot)` if there is a context to snapshot, `None` otherwise
    pub fn create_snapshot(&self, summary: String) -> Option<ContextSnapshot> {
        self.context.as_ref().map(|ctx| ContextSnapshot {
            context: ctx.clone(),
            timestamp: Utc::now(),
            summary,
        })
    }

    /// Saves a snapshot of the current context to the history.
    /// Limits the history to a maximum of 50 snapshots to prevent unbounded growth.
    ///
    /// # Arguments
    /// * `summary` - A brief description of this snapshot
    pub fn save_snapshot(&mut self, summary: String) {
        if let Some(snapshot) = self.create_snapshot(summary) {
            const MAX_HISTORY: usize = 50;
            
            // Add the new snapshot
            self.context_history.push(snapshot);
            
            // Keep only the last MAX_HISTORY snapshots
            if self.context_history.len() > MAX_HISTORY {
                self.context_history.drain(0..self.context_history.len() - MAX_HISTORY);
            }
        }
    }

    /// Reverts the conversation to the previous snapshot state.
    ///
    /// # Returns
    /// `true` if undo was successful, `false` if there's no history to undo
    pub fn undo(&mut self) -> bool {
        if let Some(snapshot) = self.context_history.pop() {
            self.context = Some(snapshot.context);
            true
        } else {
            false
        }
    }

    /// Checks if undo is available.
    ///
    /// # Returns
    /// `true` if there are snapshots in the history that can be undone
    pub fn can_undo(&self) -> bool {
        !self.context_history.is_empty()
    }

    /// Returns the summary of the last snapshot, if available.
    pub fn last_snapshot_summary(&self) -> Option<&str> {
        self.context_history.last().map(|s| s.summary.as_str())
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{Context, Role, TextMessage};

    #[test]
    fn test_new_conversation_has_empty_history() {
        let fixture = Conversation::generate();
        
        let actual = fixture.context_history.len();
        let expected = 0;
        
        assert_eq!(actual, expected);
        assert!(!fixture.can_undo());
    }

    #[test]
    fn test_save_snapshot_creates_history() {
        let mut fixture = Conversation::generate();
        let context = Context::default()
            .add_message(TextMessage::new(Role::User, "Hello"));
        fixture.context = Some(context);

        fixture.save_snapshot("First message".to_string());

        assert_eq!(fixture.context_history.len(), 1);
        assert!(fixture.can_undo());
        assert_eq!(fixture.last_snapshot_summary(), Some("First message"));
    }

    #[test]
    fn test_undo_reverts_to_previous_state() {
        let mut fixture = Conversation::generate();
        
        // Initial state with first message
        let context1 = Context::default()
            .add_message(TextMessage::new(Role::User, "Hello"));
        fixture.context = Some(context1.clone());
        fixture.save_snapshot("First message".to_string());

        // Second state with additional message
        let context2 = context1
            .add_message(TextMessage::new(Role::Assistant, "Hi there"));
        fixture.context = Some(context2);

        // Undo should revert to first state
        let undo_result = fixture.undo();

        assert!(undo_result);
        let actual = fixture.context.as_ref().unwrap().messages.len();
        let expected = 1;
        assert_eq!(actual, expected);
        assert!(!fixture.can_undo());
    }

    #[test]
    fn test_undo_without_history_returns_false() {
        let mut fixture = Conversation::generate();

        let actual = fixture.undo();
        let expected = false;

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_snapshot_history_limited_to_50() {
        let mut fixture = Conversation::generate();
        let context = Context::default()
            .add_message(TextMessage::new(Role::User, "Test"));
        fixture.context = Some(context);

        // Add 60 snapshots
        for i in 0..60 {
            fixture.save_snapshot(format!("Snapshot {}", i));
        }

        // Should only keep last 50
        let actual = fixture.context_history.len();
        let expected = 50;
        assert_eq!(actual, expected);

        // First snapshot should be "Snapshot 10" (60 - 50)
        let actual_first = fixture.context_history.first().unwrap().summary.as_str();
        let expected_first = "Snapshot 10";
        assert_eq!(actual_first, expected_first);
    }

    #[test]
    fn test_multiple_undo_operations() {
        let mut fixture = Conversation::generate();
        
        // Create 3 states
        let context1 = Context::default()
            .add_message(TextMessage::new(Role::User, "Message 1"));
        fixture.context = Some(context1.clone());
        fixture.save_snapshot("State 1".to_string());

        let context2 = context1
            .add_message(TextMessage::new(Role::Assistant, "Response 1"));
        fixture.context = Some(context2);
        fixture.save_snapshot("State 2".to_string());

        let context3 = Context::default()
            .add_message(TextMessage::new(Role::User, "Message 1"))
            .add_message(TextMessage::new(Role::Assistant, "Response 1"))
            .add_message(TextMessage::new(Role::User, "Message 2"));
        fixture.context = Some(context3);

        // First undo: from 3 messages to 2
        assert!(fixture.undo());
        assert_eq!(fixture.context.as_ref().unwrap().messages.len(), 2);

        // Second undo: from 2 messages to 1
        assert!(fixture.undo());
        assert_eq!(fixture.context.as_ref().unwrap().messages.len(), 1);

        // No more history
        assert!(!fixture.can_undo());
        assert!(!fixture.undo());
    }
}
