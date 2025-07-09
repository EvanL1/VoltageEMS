use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    Yaml,
    Toml,
    Json,
    Env,
}

impl ConfigFormat {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "yaml" | "yml" => Some(Self::Yaml),
            "toml" => Some(Self::Toml),
            "json" => Some(Self::Json),
            "env" => Some(Self::Env),
            _ => None,
        }
    }

    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            Self::Yaml => &["yaml", "yml"],
            Self::Toml => &["toml"],
            Self::Json => &["json"],
            Self::Env => &["env"],
        }
    }
}

impl fmt::Display for ConfigFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Yaml => write!(f, "YAML"),
            Self::Toml => write!(f, "TOML"),
            Self::Json => write!(f, "JSON"),
            Self::Env => write!(f, "ENV"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigPath {
    path: PathBuf,
    format: Option<ConfigFormat>,
}

impl ConfigPath {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        let path = path.as_ref();
        let format = path
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(ConfigFormat::from_extension);

        Self {
            path: path.to_path_buf(),
            format,
        }
    }

    pub fn with_format<P: AsRef<Path>>(path: P, format: ConfigFormat) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            format: Some(format),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn format(&self) -> Option<ConfigFormat> {
        self.format
    }

    pub fn exists(&self) -> bool {
        self.path.exists()
    }
}

impl FromStr for ConfigPath {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(s))
    }
}

impl fmt::Display for ConfigPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.path.display())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Development,
    Testing,
    Staging,
    Production,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Testing => "testing",
            Self::Staging => "staging",
            Self::Production => "production",
        }
    }

    pub fn from_env() -> Self {
        std::env::var("VOLTAGE_ENV")
            .or_else(|_| std::env::var("ENVIRONMENT"))
            .ok()
            .and_then(|s| Self::from_str(&s).ok())
            .unwrap_or(Self::Development)
    }
}

impl FromStr for Environment {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "development" | "dev" => Ok(Self::Development),
            "testing" | "test" => Ok(Self::Testing),
            "staging" | "stage" => Ok(Self::Staging),
            "production" | "prod" => Ok(Self::Production),
            _ => Err(format!("Unknown environment: {}", s)),
        }
    }
}

impl fmt::Display for Environment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
