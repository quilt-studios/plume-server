//! Process-isolated plugin discovery and lifecycle events.
//!
//! Plugins are normal executables, so they do not share Rust's unstable ABI
//! with the server. Each enabled `*.plugin.json` manifest points at an
//! executable. For every event the executable is started once and receives the
//! JSON event on standard input. A non-zero exit status is logged without
//! stopping the server.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::{fs, io};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    name: String,
    executable: PathBuf,
    #[serde(default = "enabled_by_default")]
    enabled: bool,
}

const fn enabled_by_default() -> bool {
    true
}

#[derive(Debug, Clone)]
pub struct Plugin {
    pub name: String,
    executable: PathBuf,
}

#[derive(Debug, Default)]
pub struct PluginManager {
    plugins: Vec<Plugin>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum PluginEvent<'a> {
    ServerStarted { address: String },
    ClientConnected { peer: &'a str },
    ClientDisconnected { peer: &'a str },
    ServerStopping,
}

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("could not read plugin directory {path}: {source}")]
    ReadDirectory { path: PathBuf, source: io::Error },
    #[error("could not read plugin manifest {path}: {source}")]
    ReadManifest { path: PathBuf, source: io::Error },
    #[error("invalid plugin manifest {path}: {source}")]
    InvalidManifest {
        path: PathBuf,
        source: serde_json::Error,
    },
}

impl PluginManager {
    pub fn load(directory: &Path) -> Result<Self, PluginError> {
        fs::create_dir_all(directory).map_err(|source| PluginError::ReadDirectory {
            path: directory.to_owned(),
            source,
        })?;

        let entries = fs::read_dir(directory).map_err(|source| PluginError::ReadDirectory {
            path: directory.to_owned(),
            source,
        })?;
        let mut manifests = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.to_string_lossy().ends_with(".plugin.json"))
            .collect::<Vec<_>>();
        manifests.sort();

        let mut plugins = Vec::new();
        for path in manifests {
            let contents =
                fs::read_to_string(&path).map_err(|source| PluginError::ReadManifest {
                    path: path.clone(),
                    source,
                })?;
            let manifest: Manifest =
                serde_json::from_str(&contents).map_err(|source| PluginError::InvalidManifest {
                    path: path.clone(),
                    source,
                })?;
            if manifest.enabled {
                let executable = if manifest.executable.is_absolute() {
                    manifest.executable
                } else {
                    directory.join(manifest.executable)
                };
                plugins.push(Plugin {
                    name: manifest.name,
                    executable,
                });
            }
        }
        Ok(Self { plugins })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Delivers an event to every plugin. Individual plugin failures are
    /// isolated.
    pub async fn emit(&self, event: &PluginEvent<'_>) {
        let payload = match serde_json::to_vec(event) {
            Ok(payload) => payload,
            Err(error) => {
                tracing::error!(%error, "could not serialize plugin event");
                return;
            }
        };

        for plugin in &self.plugins {
            let plugin = plugin.clone();
            let payload = payload.clone();
            match tokio::task::spawn_blocking(move || run_plugin(&plugin, &payload)).await {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    tracing::warn!(plugin = %error.0, error = %error.1, "plugin failed")
                }
                Err(error) => tracing::warn!(%error, "plugin task failed"),
            }
        }
    }
}

fn run_plugin(plugin: &Plugin, payload: &[u8]) -> Result<(), (String, String)> {
    use std::io::Write;

    let mut child = Command::new(&plugin.executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| (plugin.name.clone(), error.to_string()))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(payload)
            .map_err(|error| (plugin.name.clone(), error.to_string()))?;
    }
    let status = child
        .wait()
        .map_err(|error| (plugin.name.clone(), error.to_string()))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| (plugin.name.clone(), format!("process exited with {status}")))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::PluginManager;

    #[test]
    fn discovers_only_enabled_manifests() {
        let directory = std::env::temp_dir().join(format!("pomme-plugins-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("enabled.plugin.json"),
            r#"{"name":"enabled","executable":"plugin"}"#,
        )
        .unwrap();
        fs::write(
            directory.join("disabled.plugin.json"),
            r#"{"name":"disabled","executable":"plugin","enabled":false}"#,
        )
        .unwrap();

        let manager = PluginManager::load(&directory).unwrap();
        assert_eq!(manager.len(), 1);
        fs::remove_dir_all(directory).unwrap();
    }
}
