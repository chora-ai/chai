//! Hold provider clients indexed by provider id for dynamic dispatch.

use crate::providers::Provider;
use std::collections::HashMap;
use std::sync::Arc;

use super::choice::ProviderChoice;

/// Provider clients built at gateway startup, indexed by provider id.
/// Use [`ProviderClients::get`] to run [`crate::agent::run_turn`] or
/// [`crate::agent::run_turn_with_messages`] without matching on a provider enum.
#[derive(Clone, Default)]
pub struct ProviderClients {
    clients: HashMap<String, Arc<dyn Provider>>,
}

impl ProviderClients {
    /// Register a provider client under the given id.
    pub fn insert(&mut self, id: impl Into<String>, client: Arc<dyn Provider>) {
        self.clients.insert(id.into(), client);
    }

    /// Returns a trait object for the given provider id.
    pub fn get(&self, choice: &ProviderChoice) -> Option<&dyn Provider> {
        self.clients.get(choice.as_str()).map(|c| c.as_ref())
    }

    /// Returns a trait object for the given provider id string.
    pub fn get_by_id(&self, id: &str) -> Option<&dyn Provider> {
        self.clients.get(id).map(|c| c.as_ref())
    }

    /// Returns true if a provider with the given id is registered.
    pub fn has(&self, id: &str) -> bool {
        self.clients.contains_key(id)
    }

    /// Returns the set of registered provider ids.
    pub fn ids(&self) -> Vec<String> {
        self.clients.keys().cloned().collect()
    }
}
