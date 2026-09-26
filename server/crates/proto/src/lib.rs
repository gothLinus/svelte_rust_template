//! The API's Protocol Buffers messages, generated at build time from the `.proto` files in
//! `/proto`, the contract the frontend is generated from too (`just gen-types`).
//!
//! Change a message there, not here. The HTTP layer (`api::wire`) converts between these and the
//! application's DTOs.

pub mod v1 {
    include!(concat!(env!("OUT_DIR"), "/api.v1.rs"));
    include!(concat!(env!("OUT_DIR"), "/redacted.rs"));
}

pub use prost::Message;
pub use prost_types::Timestamp;
