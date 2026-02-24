//! LLM abstraction and Ollama client.
//!
//! Supports listing models and chat completion (streaming optional) against a local Ollama instance.

mod ollama;

pub use ollama::{
    ChatMessage, OllamaClient, OllamaError, OllamaModel, ToolCall, ToolCallFunction,
    ToolDefinition, ToolFunctionDefinition,
};