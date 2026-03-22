//! Hold references to configured provider clients and return [`Provider`] trait objects for dispatch.

use crate::providers::{HfClient, LmsClient, NimClient, OllamaClient, OpenAiClient, Provider, VllmClient};

use super::choice::ProviderChoice;

/// References to provider clients built at gateway startup. Use [`ProviderClients::as_dyn`]
/// to run [`crate::agent::run_turn`] or [`crate::agent::run_turn_with_messages`] without matching on [`ProviderChoice`].
#[derive(Clone, Copy)]
pub struct ProviderClients<'a> {
    pub ollama: &'a OllamaClient,
    pub lms: &'a LmsClient,
    pub vllm: &'a VllmClient,
    pub nim: &'a NimClient,
    pub openai: &'a OpenAiClient,
    pub hf: &'a HfClient,
}

impl<'a> ProviderClients<'a> {
    /// Returns a trait object for the given provider (single dispatch point for orchestration).
    pub fn as_dyn(&self, choice: ProviderChoice) -> &'a dyn Provider {
        match choice {
            ProviderChoice::Ollama => self.ollama as &dyn Provider,
            ProviderChoice::Lms => self.lms as &dyn Provider,
            ProviderChoice::Vllm => self.vllm as &dyn Provider,
            ProviderChoice::Nim => self.nim as &dyn Provider,
            ProviderChoice::OpenAi => self.openai as &dyn Provider,
            ProviderChoice::Hf => self.hf as &dyn Provider,
        }
    }
}
