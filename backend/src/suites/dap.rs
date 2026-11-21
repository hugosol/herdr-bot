//! Read deployment declarations, then pass only selected native configs to OMP.
use std::{collections::BTreeMap, io, path::Path};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{files::FileAccess, paths::Roots, tasks::TaskError};

use super::{SuiteError, Suites, mcp_paths::resolve_command};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    #[serde(deserialize_with = "unique_adapters")]
    adapters: BTreeMap<String, Definition>,
}

fn unique_adapters<'de, D: serde::Deserializer<'de>>(parser: D) -> Result<BTreeMap<String, Definition>, D::Error> {
    struct Adapters;
    impl<'de> serde::de::Visitor<'de> for Adapters {
        type Value = BTreeMap<String, Definition>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("DAP adapter ID 到定义的对象")
        }

        fn visit_map<M: serde::de::MapAccess<'de>>(self, mut entries: M) -> Result<Self::Value, M::Error> {
            let mut adapters = BTreeMap::new();
            while let Some(id) = entries.next_key::<String>()? {
                match adapters.entry(id) {
                    std::collections::btree_map::Entry::Occupied(entry) => {
                        return Err(serde::de::Error::custom(format!("适配器 ID 重复：{}", entry.key())));
                    }
                    std::collections::btree_map::Entry::Vacant(entry) => { entry.insert(entries.next_value()?); }
                }
            }
            Ok(adapters)
        }
    }
    parser.deserialize_map(Adapters)
}

#[derive(Serialize)]
struct NativeDocument<'a> {
    adapters: BTreeMap<&'a str, Value>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Definition {
    name: String,
    description: String,
    config: Value,
}

#[derive(Serialize)]
pub(super) struct CatalogEntry {
    id: String,
    #[serde(flatten)]
    definition: Definition,
}

impl Definition {
    fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() || self.description.trim().is_empty() {
            return Err("名称和说明不能为空".into());
        }
        let config = self.config.as_object().ok_or("config 必须是原生配置对象")?;
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
}

impl Suites {
    fn read_dap(&self, access: &FileAccess) -> Result<BTreeMap<String, Definition>, SuiteError> {
        let fail = |message: &dyn std::fmt::Display| {
            SuiteError::storage(format!("集中 DAP 目录 {}：{message}", self.dap_file.display()))
        };
        let raw = match access.read_to_string(&self.dap_file) {
            Ok(raw) => raw,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
            Err(error) => return Err(fail(&error)),
        };
        let mut parser = serde_json::Deserializer::from_str(&raw);
        let document: Document = serde_path_to_error::deserialize(&mut parser).map_err(|error| fail(&error))?;
        parser.end().map_err(|error| fail(&error))?;
        for (id, definition) in &document.adapters {
            if id.trim().is_empty() || id.trim() != id {
                return Err(fail(&"适配器 ID 不能为空或含首尾空白"));
            }
            definition.validate().map_err(|error| fail(&format!("适配器「{id}」：{error}")))?;
        }
        Ok(document.adapters)
    }

    pub(super) fn dap_catalog(&self) -> Result<Vec<CatalogEntry>, SuiteError> {
        self.files.with(std::slice::from_ref(&self.dap_file), |access| {
            self.read_dap(access).map(|catalog| catalog.into_iter().map(|(id, definition)| {
                CatalogEntry { id, definition }
            }).collect())
        }).map_err(SuiteError::storage)?
    }

    pub(super) fn validate_dap_references(&self, access: &FileAccess, suite: &str, selected: &[String]) -> Result<(), SuiteError> {
        if selected.is_empty() { return Ok(()); }
        let catalog = self.read_dap(access)?;
        for id in selected {
            if !catalog.contains_key(id) {
                return Err(SuiteError::invalid(format!("Suite「{suite}」的集中 DAP 引用失效：{id}；请显式修正引用。")));
            }
        }
        Ok(())
    }

    pub(super) fn select_omp_dap(&self, suite: &str, selected: &[String], roots: &Roots) -> Result<Option<Vec<u8>>, TaskError> {
        if selected.is_empty() { return Ok(None); }
        let fail = |error: String| TaskError::InvalidSuite(format!("Suite「{suite}」DAP 装配失败：{error}；没有回退为空环境。"));
        self.files.with(std::slice::from_ref(&self.dap_file), |access| {
            let mut catalog = self.read_dap(access).map_err(|error| fail(error.message))?;
            let base = self.dap_file.parent().unwrap_or(Path::new("."));
            let mut adapters = BTreeMap::new();
            for id in selected {
                let Definition { mut config, .. } = catalog.remove(id).ok_or_else(|| fail(format!("集中 DAP 引用失效：{id}")))?;
                // Only the adapter executable is rebased. Its args, port template,
                // project markers and launch/attach payload remain native literals.
                resolve_command(&mut config, roots, base).map_err(|error| fail(format!("适配器「{id}」：{error}")))?;
                adapters.insert(id.as_str(), config);
            }
            serde_json::to_vec_pretty(&NativeDocument { adapters }).map(Some).map_err(|error| fail(error.to_string()))
        }).map_err(|error| fail(error.to_string()))?
    }
}
