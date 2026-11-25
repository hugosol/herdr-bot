//! Validate known native DAP fields without discarding future fields.
use serde_json::Value;
use super::ResourceDefinition;

pub(super) fn validate(definition: &ResourceDefinition) -> Result<(), String> {
    if definition.name.trim().is_empty() || definition.description.trim().is_empty() {
        return Err("名称和说明不能为空".into());
    }
    let config = &definition.config;
    if !config.get("command").and_then(Value::as_str).is_some_and(|value| !value.trim().is_empty()) {
        return Err("config.command 必须是非空字符串".into());
    }
    for field in ["args", "languages", "fileTypes", "rootMarkers"] {
        if config.get(field).is_some_and(|value| {
            !value.as_array().is_some_and(|entries| {
                entries.iter().all(|entry| entry.as_str().is_some_and(|entry| !entry.is_empty()))
            })
        }) {
            return Err(format!("config.{field} 必须是非空字符串的数组"));
        }
    }
    if config.get("connectMode").is_some_and(|value| !matches!(value.as_str(), Some("stdio" | "tcp" | "socket"))) {
        return Err("config.connectMode 必须是 stdio、tcp 或 socket".into());
    }
    for field in ["launchDefaults", "attachDefaults"] {
        if config.get(field).is_some_and(|value| !value.is_object()) {
            return Err(format!("config.{field} 必须是对象"));
        }
    }
    if config.get("acceptsDirectoryProgram").is_some_and(|value| !value.is_boolean()) {
        return Err("config.acceptsDirectoryProgram 必须是布尔值".into());
    }
    if config.get("attachDefaults").and_then(|value| value.get("skipAttachRequest"))
        .is_some_and(|value| !value.is_boolean())
    {
        return Err("config.attachDefaults.skipAttachRequest 必须是布尔值".into());
    }
    Ok(())
}
