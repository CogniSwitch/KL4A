//! agent_consumption's pure derivations + agent_tools' pure reads. Must stay side-effect-free apart from file reads. See docs/port/PORT_PLAN.md §3.1 (sopkb-derive) and §6.5 (Phase 4).

pub mod concepts;
pub mod constants;
pub mod context;
pub mod matching;
pub mod reads;
pub mod relations;
pub mod rules;
pub mod tokens;
