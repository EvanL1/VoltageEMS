use regex::Regex;
use std::any::Any;
use std::fmt;

use crate::{ConfigError, Result};

pub trait ValidationRule: Send + Sync {
    fn validate(&self, value: &dyn Any) -> Result<()>;
    fn name(&self) -> &str;
}

pub struct RegexRule {
    name: String,
    regex: Regex,
    field_path: String,
}

impl RegexRule {
    pub fn new(
        name: impl Into<String>,
        pattern: &str,
        field_path: impl Into<String>,
    ) -> Result<Self> {
        let regex = Regex::new(pattern)
            .map_err(|e| ConfigError::Validation(format!("Invalid regex pattern: {}", e)))?;

        Ok(Self {
            name: name.into(),
            regex,
            field_path: field_path.into(),
        })
    }
}

impl ValidationRule for RegexRule {
    fn validate(&self, value: &dyn Any) -> Result<()> {
        if let Some(s) = value.downcast_ref::<String>() {
            if !self.regex.is_match(s) {
                return Err(ConfigError::Validation(format!(
                    "Field '{}' does not match pattern: {}",
                    self.field_path,
                    self.regex.as_str()
                )));
            }
        }
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

pub struct RangeRule<T> {
    name: String,
    min: Option<T>,
    max: Option<T>,
    field_path: String,
}

impl<T: PartialOrd + fmt::Display + Send + Sync + 'static> RangeRule<T> {
    pub fn new(
        name: impl Into<String>,
        min: Option<T>,
        max: Option<T>,
        field_path: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            min,
            max,
            field_path: field_path.into(),
        }
    }
}

impl<T: PartialOrd + fmt::Display + Send + Sync + 'static> ValidationRule for RangeRule<T> {
    fn validate(&self, value: &dyn Any) -> Result<()> {
        if let Some(v) = value.downcast_ref::<T>() {
            if let Some(ref min) = self.min {
                if v < min {
                    return Err(ConfigError::Validation(format!(
                        "Field '{}' value {} is less than minimum {}",
                        self.field_path, v, min
                    )));
                }
            }
            if let Some(ref max) = self.max {
                if v > max {
                    return Err(ConfigError::Validation(format!(
                        "Field '{}' value {} is greater than maximum {}",
                        self.field_path, v, max
                    )));
                }
            }
        }
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

pub struct RequiredRule {
    name: String,
    field_path: String,
}

impl RequiredRule {
    pub fn new(name: impl Into<String>, field_path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            field_path: field_path.into(),
        }
    }
}

impl ValidationRule for RequiredRule {
    fn validate(&self, value: &dyn Any) -> Result<()> {
        if let Some(opt) = value.downcast_ref::<Option<Box<dyn Any>>>() {
            if opt.is_none() {
                return Err(ConfigError::Validation(format!(
                    "Required field '{}' is missing",
                    self.field_path
                )));
            }
        }
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

pub fn validate_config<T: Any>(config: &T, rules: &[Box<dyn ValidationRule>]) -> Result<()> {
    for rule in rules {
        rule.validate(config)?;
    }
    Ok(())
}
