//! Approved S2/S3/S4 seams: real HTTP, external controlled host and disk records.
//! P8/P10/P13/P14/P15/P16: a prompt-first launch or a copied manager input
//! would violate independent literal expectations at the host boundary.
#[path = "../common/mod.rs"]
mod common;
#[path = "dispatch/portability.rs"]
mod portability;
#[path = "recovery/recovery.rs"]
mod recovery;
#[path = "lifecycle/retry.rs"]
mod retry;
#[path = "lifecycle/nested_retry.rs"]
mod nested_retry;
#[path = "history/records.rs"]
mod records;
#[path = "history/integrity.rs"]
mod integrity;
#[path = "security/security.rs"]
mod security;
#[path = "dispatch/pi.rs"]
mod pi;
#[path = "dispatch/roles.rs"]
mod roles;
#[path = "reports/report_policy.rs"]
mod report_policy;
#[path = "../scheduler/maps/map_deletion.rs"]
mod map_deletion;
#[path = "../scheduler/maps/map_title.rs"]
mod map_title;
#[path = "history/history_integrity.rs"]
mod history_integrity;
#[path = "scripts/rust_create.rs"]
mod rust_create;
#[path = "scripts/rust_save.rs"]
mod rust_save;
#[path = "scripts/rust_lifecycle.rs"]
mod rust_lifecycle;
#[path = "../scheduler/bots/bot_targets.rs"]
mod bot_targets;
#[path = "lifecycle/nesting.rs"]
mod nesting;
#[path = "lifecycle/nested_completion.rs"]
mod nested_completion;
#[path = "lifecycle/local_lifecycle.rs"]
mod local_lifecycle;
#[path = "lifecycle/partial_startup.rs"]
mod partial_startup;
#[path = "lifecycle/auto_close.rs"]
mod auto_close;
#[path = "inputs/named_inputs.rs"]
mod named_inputs;
#[path = "presets/plan_workflow.rs"]
mod plan_workflow;
#[path = "../scheduler/bots/shared_business.rs"]
mod shared_business;
#[path = "presets/create_bot_wrapper.rs"]
mod create_bot_wrapper;
#[path = "scripts/create_migration.rs"]
mod create_migration;
#[path = "presets/revise_bot_guides.rs"]
mod revise_bot_guides;
#[path = "presets/outer_create.rs"]
mod outer_create;
#[path = "inputs/input_resolution.rs"]
mod input_resolution;
#[path = "inputs/input_preparation.rs"]
mod input_preparation;
#[path = "inputs/file_consumption.rs"]
mod file_consumption;
#[path = "reports/report_outputs.rs"]
mod report_outputs;
#[path = "reports/report_restart.rs"]
mod report_restart;
#[path = "recovery/resume_restart.rs"]
mod resume_restart;
#[path = "lifecycle/completion_gate.rs"]
mod completion_gate;

// Crate-scope includes retain libtest names, including --exact child selectors.
include!("support/dispatch.rs");
include!("dispatch/core.rs");
include!("collect/core.rs");
include!("lifecycle/core.rs");
include!("reports/core.rs");
