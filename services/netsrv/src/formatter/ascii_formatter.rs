use crate::error::{NetSrvError, Result};
use crate::formatter::DataFormatter;
use serde_json::Value;
use std::fmt::Write;

pub struct AsciiFormatter;

impl AsciiFormatter {
    pub fn new() -> Self {
        AsciiFormatter
    }
}

impl DataFormatter for AsciiFormatter {
    fn format(&self, data: &Value) -> Result<String> {
        let mut output = String::new();
        format_value(data, &mut output, 0)?;
        Ok(output)
    }
}

fn format_value(value: &Value, output: &mut String, depth: usize) -> Result<()> {
    match value {
        Value::Object(map) => {
            for (key, val) in map {
                let indent = " ".repeat(depth * 2);
                write!(output, "{}{}: ", indent, key)
                    .map_err(|e| NetSrvError::Format(format!("ASCII formatting error: {}", e)))?;

                match val {
                    Value::Object(_) | Value::Array(_) => {
                        writeln!(output).map_err(|e| {
                            NetSrvError::Format(format!("ASCII formatting error: {}", e))
                        })?;
                        format_value(val, output, depth + 1)?;
                    }
                    _ => {
                        let val_str = format_simple_value(val)?;
                        writeln!(output, "{}", val_str).map_err(|e| {
                            NetSrvError::Format(format!("ASCII formatting error: {}", e))
                        })?;
                    }
                }
            }
        }
        Value::Array(arr) => {
            for (i, val) in arr.iter().enumerate() {
                let indent = " ".repeat(depth * 2);
                write!(output, "{}[{}]: ", indent, i)
                    .map_err(|e| NetSrvError::Format(format!("ASCII formatting error: {}", e)))?;

                match val {
                    Value::Object(_) | Value::Array(_) => {
                        writeln!(output).map_err(|e| {
                            NetSrvError::Format(format!("ASCII formatting error: {}", e))
                        })?;
                        format_value(val, output, depth + 1)?;
                    }
                    _ => {
                        let val_str = format_simple_value(val)?;
                        writeln!(output, "{}", val_str).map_err(|e| {
                            NetSrvError::Format(format!("ASCII formatting error: {}", e))
                        })?;
                    }
                }
            }
        }
        _ => {
            let val_str = format_simple_value(value)?;
            writeln!(output, "{}", val_str)
                .map_err(|e| NetSrvError::Format(format!("ASCII formatting error: {}", e)))?;
        }
    }

    Ok(())
}

fn format_simple_value(value: &Value) -> Result<String> {
    match value {
        Value::Null => Ok("null".to_string()),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Number(n) => Ok(n.to_string()),
        Value::String(s) => Ok(s.clone()),
        _ => Err(NetSrvError::Format("Unexpected complex value".to_string())),
    }
}
