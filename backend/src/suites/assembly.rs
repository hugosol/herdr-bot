//! One private, exclusively created tree per launch. The publication boundary
//! is returning native launch arguments, not creating the directory. No live
//! tree is rewritten or reclaimed while an agent may still consume it.
use crate::{paths::Roots, tasks::{Agent, TaskError}};
use super::{Suite, Suites, skills};

impl Suites {
    pub(crate) fn prepare_environment(&self, suite: Option<&Suite>, agent: Agent, roots: &Roots, shared_extensions: &[std::path::PathBuf]) -> Result<Vec<String>, TaskError> {
        let (pi_args, pi_config) = if agent == Agent::Pi {
            self.select_pi(suite, roots)?
        } else { (Vec::new(), None) };
        let Some(suite) = suite else { return Ok(pi_args) };
        // Resolve and validate every resource before creating any output.
        let skills = self.select_skills(suite, roots)?;
        let extensions = self.select_extensions(suite, agent, roots, shared_extensions)?;
        let mcp = self.select_mcp(suite, agent, roots)?;
        let directory = self.generated_directory.join(&suite.id);
        let assembly = directory.join(crate::chats::new_creation_id());
        let fail = |error: String| TaskError::InvalidSuite(format!(
            "Suite「{}」资源装配失败：{error}；没有回退为空环境、复制替代或自动提权。", suite.id));
        let mut scopes = vec![directory];
        scopes.extend(skills.iter().map(|skill| skill.directory.clone()));
        self.files.with(&scopes, |access| {
            access.create_dir_all(assembly.parent().unwrap())
                .map_err(|error| fail(format!("{}：{error}", assembly.parent().unwrap().display())))?;
            // A collision is an error, never permission to clean an existing
            // tree. Ownership begins only after this create succeeds.
            access.create_dir(&assembly).map_err(|error| fail(error.to_string()))?;
            let result = (|| {
                let mut args = skills::assemble(access, &assembly, suite, agent, &skills)
                    .map_err(|error| format!("技能装配失败：{error}"))?;
                for entry in &extensions {
                    args.extend(["--extension".into(), entry.to_string_lossy().into_owned()]);
                }
                if let Some(bytes) = &mcp {
                    let native = assembly.join("mcp");
                    access.create_dir(&native).map_err(|error| error.to_string())?;
                    access.create(native.join(".mcp.json"), bytes).map_err(|error| error.to_string())?;
                    args.extend(["--extension".into(), native.to_string_lossy().into_owned()]);
                }
                args.extend(pi_args);
                if let Some(bytes) = &pi_config {
                    let config = assembly.join("pi-suite.json");
                    access.create(&config, bytes).map_err(|error| error.to_string())?;
                    args.extend(["--pi-suite-config".into(), config.to_string_lossy().into_owned()]);
                }
                Ok(args)
            })();
            result.map_err(|error| match access.remove_dir_all(&assembly) {
                // This root is an ordinary directory created above; recursive
                // removal unlinks child symlinks, never their resource targets.
                Ok(()) => fail(error),
                Err(cleanup) => fail(format!("{error}；清理本次产物另失败：{cleanup}")),
            })
        }).map_err(|error| fail(error.to_string()))?
        // The FileStore lease is gone before the caller starts Herdr.
    }
}
