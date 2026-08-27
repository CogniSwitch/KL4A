//! agent_chat: SYSTEM_PROMPT, run_agent_chat, transcript read/append/cap. Default provider: azure-llm (docs/port/DECISIONS.md Q6b). See docs/port/PORT_PLAN.md §3.1 (sopkb-agent) and §6.9 (Phase 8).

mod chat;
mod context_answer;
mod prompt;
mod react;
mod run;
mod transcript;

pub use chat::{chat_turns_for, handle_chat_turn, run_chat_turn, run_chat_turn_with_transport, ChatTurn, CHAT_TRANSCRIPT_CAP};
pub use context_answer::render_context_answer;
pub use prompt::{build_agent_messages, SYSTEM_PROMPT};
pub use react::{
    handle_react_chat, run_react_chat, run_react_chat_with_history_and_transport, run_react_chat_with_transport, MAX_REACT_ITERATIONS,
};
pub use run::{run_agent_chat, run_agent_chat_with_transport};
pub use transcript::{
    append_agent_chat_entry, append_agent_chat_entry_capped, delete_chat, handle_agent_chat, read_agent_transcript, DEFAULT_TRANSCRIPT_CAP,
};
