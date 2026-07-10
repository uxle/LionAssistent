//! `common` -- shared types and the central error system for the Local
//! Linux AI Assistant workspace (PRD SS8, DIG SS4).
//!
//! Every crate in the workspace may depend on `common`. `common` itself
//! must not depend on any other workspace crate, to avoid circular
//! dependencies (Apis SS1 API design principles).

pub mod error;
pub mod types;

pub use error::{AppError, AppResult};
pub use types::{now, Id, PermissionLevel, Timestamp};
