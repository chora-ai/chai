//! Generic tool executor driven by a skill's tools.json descriptor.
//! Builds argv from the execution spec's arg mapping and runs via the allowlist.
//! Supports optional content normalization (literal \n/\t -> newline/tab) and
//! resolve-by-command or resolve-by-script (when skills.allowScripts is true).

use std::collections::HashMap;
use std::path::Path;

use crate::agent::ToolExecutor;
use crate::exec::Allowlist;
use crate::skills::{ArgKind, ExecutionSpec, ToolDescriptor};

/// Executes tools using a descriptor's allowlist and execution mapping.
/// Holds per-tool (allowlist, spec, skill_dir) and whether scripts are allowed.
#[derive(Debug, Clone)]
pub struct GenericToolExecutor {
    /// tool_name -> (allowlist, execution spec, skill dir for script resolution)
    map: HashMap<String, (Allowlist, ExecutionSpec, Option<std::path::PathBuf>)>,
    allow_scripts: bool,
}

impl GenericToolExecutor {
    /// Build an executor from skill descriptors and optional skill dirs. When skills.allowScripts is true,
    /// resolveCommand.script in tools.json runs the named script from the skill's scripts/ directory.
    pub fn from_descriptors(
        descriptors: &[(String, ToolDescriptor)],
        skill_dirs: &[(String, std::path::PathBuf)],
        allow_scripts: bool,
    ) -> Self {
        let dir_map: HashMap<&String, &std::path::PathBuf> =
            skill_dirs.iter().map(|(n, p)| (n, p)).collect();
        let mut map = HashMap::new();
        for (skill_name, desc) in descriptors {
            let allowlist = desc.to_allowlist();
            let skill_dir = dir_map.get(skill_name).cloned().cloned();
            for spec in &desc.execution {
                map.insert(
                    spec.tool.clone(),
                    (allowlist.clone(), spec.clone(), skill_dir.clone()),
                );
            }
        }
        Self {
            map,
            allow_scripts,
        }
    }

    /// Return true if this executor handles the given tool name.
    pub fn has_tool(&self, name: &str) -> bool {
        self.map.contains_key(name)
    }

    /// Tool names this executor can run.
    pub fn tool_names(&self) -> impl Iterator<Item = &String> {
        self.map.keys()
    }
}

impl ToolExecutor for GenericToolExecutor {
    fn execute(&self, name: &str, args: &serde_json::Value) -> Result<String, String> {
        let (allowlist, spec, skill_dir) = self
            .map
            .get(name)
            .ok_or_else(|| format!("unknown tool: {}", name))?;
        let argv = build_argv(
            spec,
            args,
            allowlist,
            skill_dir.as_deref(),
            self.allow_scripts,
        )?;
        allowlist.run(&spec.binary, &spec.subcommand, &argv)
    }
}

/// Normalize string so literal `\n` and `\t` from JSON become real newlines/tabs.
fn normalize_content(s: &str) -> String {
    s.replace("\\n", "\n").replace("\\t", "\t")
}

/// If the arg has resolve_command with script set and allow_scripts and skill_dir, run the script; else if binary/subcommand set, run via allowlist. Use trimmed stdout or keep original on failure.
fn resolve_value(
    value: &str,
    arg: &crate::skills::ArgMapping,
    allowlist: &Allowlist,
    skill_dir: Option<&Path>,
    allow_scripts: bool,
) -> String {
    let Some(ref cmd) = arg.resolve_command else {
        return value.to_string();
    };
    let argv: Vec<String> = cmd
        .args
        .iter()
        .map(|a| a.replace("$param", value))
        .collect();

    if allow_scripts {
        if let (Some(dir), Some(ref script_name)) = (skill_dir, &cmd.script) {
            if let Ok(out) = run_script(dir, script_name, &argv) {
                let s = out.trim();
                return if s.is_empty() {
                    value.to_string()
                } else {
                    s.to_string()
                };
            }
            return value.to_string();
        }
    }

    if let (Some(ref binary), Some(ref subcommand)) = (&cmd.binary, &cmd.subcommand) {
        match allowlist.run(binary, subcommand, &argv) {
            Ok(out) => {
                let s = out.trim();
                return if s.is_empty() {
                    value.to_string()
                } else {
                    s.to_string()
                };
            }
            Err(_) => {}
        }
    }
    value.to_string()
}

/// Run a script from the skill's scripts/ dir. Returns stdout or error. Only runs if path is under skill_dir/scripts and is a file.
fn run_script(
    skill_dir: &Path,
    script_name: &str,
    args: &[String],
) -> Result<String, String> {
    if script_name.contains("..") || script_name.contains('/') || script_name.contains('\\') {
        return Err("invalid script name".to_string());
    }
    let scripts_dir = skill_dir.join("scripts");
    let mut script_path = scripts_dir.join(script_name);
    if !script_path.starts_with(&scripts_dir) {
        return Err("script path outside scripts dir".to_string());
    }
    if !script_path.is_file() {
        script_path = script_path.with_extension("sh");
        if !script_path.starts_with(&scripts_dir) || !script_path.is_file() {
            return Err("script not found".to_string());
        }
    }
    let output = std::process::Command::new("sh")
        .arg(&script_path)
        .args(args)
        .output()
        .map_err(|e| format!("exec failed: {}", e))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    if output.status.success() {
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Err(format!("exit {}: {}", output.status, stderr))
    }
}

/// Apply optional normalize_newlines and resolve_command to a string value.
fn transform_param_value(
    s: String,
    arg: &crate::skills::ArgMapping,
    allowlist: &Allowlist,
    skill_dir: Option<&Path>,
    allow_scripts: bool,
) -> String {
    let s = if arg.normalize_newlines == Some(true) {
        normalize_content(&s)
    } else {
        s
    };
    resolve_value(&s, arg, allowlist, skill_dir, allow_scripts)
}

/// Build argv from the execution spec's arg mapping and the JSON args object.
fn build_argv(
    spec: &ExecutionSpec,
    args: &serde_json::Value,
    allowlist: &Allowlist,
    skill_dir: Option<&Path>,
    allow_scripts: bool,
) -> Result<Vec<String>, String> {
    let obj = args
        .as_object()
        .ok_or_else(|| "arguments must be an object".to_string())?;
    let mut argv = Vec::new();
    for arg in &spec.args {
        match arg.kind {
            ArgKind::Positional => {
                let value = obj
                    .get(&arg.param)
                    .ok_or_else(|| format!("missing parameter: {}", arg.param))?;
                let s = json_value_to_string(value).ok_or_else(|| {
                    format!(
                        "parameter {} must be a string, number, or boolean",
                        arg.param
                    )
                })?;
                argv.push(transform_param_value(
                    s, arg, allowlist, skill_dir, allow_scripts,
                ));
            }
            ArgKind::Flag => {
                let value = match obj.get(&arg.param) {
                    Some(v) if !v.is_null() => v,
                    _ => continue,
                };
                let s = json_value_to_string(value).ok_or_else(|| {
                    format!(
                        "parameter {} must be a string, number, or boolean",
                        arg.param
                    )
                })?;
                let flag = arg.flag.as_deref().unwrap_or(&arg.param);
                argv.push(format!("--{}", flag));
                argv.push(transform_param_value(
                    s, arg, allowlist, skill_dir, allow_scripts,
                ));
            }
            ArgKind::FlagIfBoolean => {
                let value = obj.get(&arg.param);
                let flag = match parse_bool(value) {
                    Some(true) => arg.flag_if_true.as_deref(),
                    _ => arg.flag_if_false.as_deref(),
                };
                if let Some(f) = flag {
                    argv.push(f.to_string());
                }
            }
        }
    }
    Ok(argv)
}

fn json_value_to_string(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn parse_bool(v: Option<&serde_json::Value>) -> Option<bool> {
    match v {
        Some(serde_json::Value::Bool(b)) => Some(*b),
        Some(serde_json::Value::String(s)) => Some(s.eq_ignore_ascii_case("true")),
        Some(serde_json::Value::Number(n)) => n.as_i64().map(|i| i != 0),
        _ => None,
    }
}
