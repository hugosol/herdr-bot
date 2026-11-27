//! Initial Chat message assembly from verified launch facts. Callers own role
//! selection, file resolution, admission, and submission.
mod compose;
mod context;
mod dictionary;
mod roles;

pub(crate) use context::{ChiefDelegationContext, ManagerContext, NodeContext, NodeRole, RunFiles};
pub use dictionary::{Field, PromptDictionary, Snapshot};
