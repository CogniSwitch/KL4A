//! Serialization primitives only, no domain types. See docs/port/PORT_PLAN.md §3.1 (sopkb-fmt)
//! and §6.2 (Phase 1).

pub mod char_offset;
pub mod json_canonical;
pub mod ordered_map;
pub mod py_float;
pub mod py_str;
pub mod yaml_pyemit;

pub use char_offset::{byte_to_char_offset, CharIndex};
pub use json_canonical::to_canonical_json;
pub use ordered_map::OrderedMap;
pub use py_float::py_float;
pub use py_str::py_title_case;
pub use yaml_pyemit::{emit as emit_yaml, parse as parse_yaml, YamlValue};
