//! Domain-specific HTTP conversions, separate from domain implementations.
pub(crate) mod chats;
pub(crate) mod chief;
pub(crate) mod chief_delegation;
pub(crate) mod chief_reports;
pub(crate) mod runs;
pub(crate) mod scheduler;
pub(crate) mod suites;
pub(crate) mod system;
pub(crate) mod tasks;
pub(crate) mod zones;
pub(crate) mod zone_git;

/// Keeps HTTP conversion implementations out of the domain modules without
/// combining their distinct error types or response contracts.
pub(crate) struct HttpError<E>(pub(crate) E);
