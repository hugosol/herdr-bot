//! Herdr integration shared by Chat, workflow, and report delivery.
//!
//! Callers use this module's control and identity interface; CLI transport and
//! native identity evidence stay private. Placement selects positions without
//! creating or navigating them. Business authority and persistence stay with
//! the calling modules.

mod client;
mod identity;
pub(crate) mod placement;

pub use client::{CreatedTab, HerdrClient, HerdrError, NavigationTarget, Workspace};
pub use identity::NativeSession;
pub(crate) use identity::read_native_id;
