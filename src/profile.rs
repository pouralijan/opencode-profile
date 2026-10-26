use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub(crate) struct Roots {
    pub(crate) config: PathBuf,
    pub(crate) data: PathBuf,
    pub(crate) state: PathBuf,
    pub(crate) cache: PathBuf,
}

impl Roots {
    pub(crate) fn new() -> Result<Self> {
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .context("HOME is not set")?;
        let base = |key: &str, fallback: &str| {
            env::var_os(key)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(fallback))
        };
        let config = base("XDG_CONFIG_HOME", ".config");
        let data = base("XDG_DATA_HOME", ".local/share");
        let state = base("XDG_STATE_HOME", ".local/state");
        let cache = base("XDG_CACHE_HOME", ".cache");
        for path in [&config, &data, &state, &cache] {
            if !path.is_absolute() {
                bail!("XDG home must be absolute: {}", path.display());
            }
        }
        Ok(Self {
            config: config.join("opencode-profile/profiles"),
            data: data.join("opencode-profile/profiles"),
            state: state.join("opencode-profile/profiles"),
            cache: cache.join("opencode-profile/profiles"),
        })
    }

    pub(crate) fn profile(&self, name: &str) -> Result<Profile> {
        validate_name(name)?;
        Ok(Profile {
            name: name.to_string(),
            config: self.config.join(name),
            data: self.data.join(name),
            state: self.state.join(name),
            cache: self.cache.join(name),
        })
    }

    pub(crate) fn names(&self) -> Result<Vec<String>> {
        let mut names = Vec::new();
        if self.config.exists() {
            for entry in fs::read_dir(&self.config)? {
                let entry = entry?;
                let name = entry.file_name();
                if let Some(name) = name.to_str() {
                    if validate_name(name).is_ok() && entry.file_type()?.is_dir() {
                        names.push(name.to_string());
                    }
                }
            }
        }
        names.sort();
        Ok(names)
    }
}

pub(crate) struct Profile {
    pub(crate) name: String,
    pub(crate) config: PathBuf,
    pub(crate) data: PathBuf,
    pub(crate) state: PathBuf,
    pub(crate) cache: PathBuf,
}

impl Profile {
    fn paths(&self) -> [&Path; 4] {
        [&self.config, &self.data, &self.state, &self.cache]
    }

    pub(crate) fn exists(&self) -> bool {
        self.paths().iter().any(|p| fs::symlink_metadata(p).is_ok())
    }

    pub(crate) fn require_config(&self) -> Result<()> {
        if !self.config.is_dir() || fs::symlink_metadata(&self.config)?.file_type().is_symlink() {
            bail!("Profile '{}' has no safe config directory", self.name);
        }
        Ok(())
    }

    pub(crate) fn create(&self) -> Result<()> {
        if self.exists() {
            bail!("Profile '{}' already exists", self.name);
        }
        fs::create_dir_all(&self.config)?;
        fs::create_dir_all(&self.data)?;
        for subdir in ["plugins", "commands", "agents", "modes"] {
            fs::create_dir_all(self.config.join(subdir))?;
        }
        fs::write(
            self.config.join("opencode.json"),
            "{\n  \"$schema\": \"https://opencode.ai/config.json\"\n}\n",
        )?;
        Ok(())
    }

    pub(crate) fn safe_settings(&self) -> Result<Value> {
        let path = self.config.join("opencode.json");
        if !path.exists() {
            return Ok(json!({"$schema": "https://opencode.ai/config.json"}));
        }
        if fs::symlink_metadata(&path)?.file_type().is_symlink() {
            bail!("Refusing symlinked config: {}", path.display());
        }
        safe_settings_from(&path)
    }

    pub(crate) fn status(&self) -> &'static str {
        if !self.config.is_dir() || !self.data.is_dir() {
            "⚠ incomplete"
        } else {
            "✓ ready (auth unverified)"
        }
    }

    pub(crate) fn remove(&self) -> Result<()> {
        if !self.exists() {
            bail!("Profile '{}' not found", self.name);
        }
        for path in self.paths() {
            match fs::symlink_metadata(path) {
                Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
                    bail!(
                        "Refusing non-directory or symlink profile path: {}",
                        path.display()
                    );
                }
                Ok(_) => (),
                Err(e) if e.kind() == io::ErrorKind::NotFound => (),
                Err(e) => return Err(e.into()),
            }
        }
        for path in self.paths() {
            if path.exists() {
                fs::remove_dir_all(path).with_context(|| format!("Removing {}", path.display()))?;
            }
        }
        Ok(())
    }
}

fn safe_settings_from(path: &Path) -> Result<Value> {
    let value: Value = serde_json::from_slice(&fs::read(path)?)
        .with_context(|| format!("Invalid JSON: {}", path.display()))?;
    let object = value
        .as_object()
        .context("OpenCode config must be a JSON object")?;
    let mut safe = Map::new();
    safe.insert("$schema".into(), json!("https://opencode.ai/config.json"));
    // Copy only simple, non-secret display/model preferences. Provider and plugin settings may carry tokens.
    for key in ["theme", "model", "small_model"] {
        if let Some(Value::String(value)) = object.get(key) {
            safe.insert(key.into(), Value::String(value.clone()));
        }
    }
    Ok(Value::Object(safe))
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 64
        || !name.as_bytes()[0].is_ascii_alphanumeric()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        bail!("Invalid profile name: use 1-64 ASCII letters, digits, hyphens or underscores; start with a letter or digit");
    }
    Ok(())
}
