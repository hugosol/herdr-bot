//! One private, exclusively created tree per launch. The publication boundary
//! is returning native launch arguments, not creating the directory. No live
//! tree is rewritten or reclaimed while an agent may still consume it.
use crate::{paths::Roots, tasks::{Agent, TaskError}};
use super::{Suite, Suites, skills};

impl Suites {
    pub(crate) fn prepare_environment(&self, suite: Option<&Suite>, agent: Agent, roots: &Roots) -> Result<Vec<String>, TaskError> {
        let Some(suite) = suite else {
            return Ok(if agent == Agent::Pi { super::pi::default_args() } else { Vec::new() });
        };
        suite.require_harness(agent)?;
        let id = suite.id();
        // Resolve and validate every resource before creating any output.
        let pi = match suite {
            Suite::Pi { prompts, codemode, mcp, .. } => Some(self.select_pi(id, prompts, *codemode, mcp, roots)?),
            Suite::Omp { .. } => None,
        };
        let skills = self.select_skills(suite, roots)?;
        let extensions = self.select_extensions(suite, roots)?;
        let mcp = match suite {
            Suite::Omp { mcp, .. } => self.select_omp_mcp(id, mcp, roots)?,
            Suite::Pi { .. } => None,
        };
        let directory = self.generated_directory.join(id);
        let assembly = directory.join(crate::chats::new_creation_id());
        let fail = |error: String| TaskError::InvalidSuite(format!(
            "Suite「{id}」资源装配失败：{error}；没有回退为空环境、复制替代或自动提权。"));
        self.files.with(&[directory], |access| {
            access.create_dir_all(assembly.parent().unwrap())
                .map_err(|error| fail(format!("{}：{error}", assembly.parent().unwrap().display())))?;
            // A collision is an error, never permission to clean an existing
            // tree. Ownership begins only after this create succeeds.
            access.create_dir(&assembly).map_err(|error| fail(error.to_string()))?;
            let result = (|| {
                let mut args = skills::assemble(access, &assembly, suite, &skills)
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
                if let Some((pi_args, bytes)) = pi {
                    args.extend(pi_args);
                    let config = assembly.join("pi-suite.json");
                    access.create(&config, &bytes).map_err(|error| error.to_string())?;
                    args.extend(["--pi-suite-config".into(), config.to_string_lossy().into_owned()]);
                }
                Ok(args)
            })();
            result.map_err(|error| match access.remove_dir_all(&assembly) {
                // Only this launch's generated configuration tree is owned;
                // native skill source directories are never written here.
                Ok(()) => fail(error),
                Err(cleanup) => fail(format!("{error}；清理本次产物另失败：{cleanup}")),
            })
        }).map_err(|error| fail(error.to_string()))?
        // The FileStore lease is gone before the caller starts Herdr.
    }
}
