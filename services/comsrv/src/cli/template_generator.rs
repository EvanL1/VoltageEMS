//! Protocol Template Generator
//!
//! This module provides template generation functionality for creating new protocol plugins.

use std::path::Path;
use std::fs;
use handlebars::Handlebars;
use serde_json::json;

use crate::utils::{Result, Error};

/// Template generator for protocol plugins
pub struct TemplateGenerator {
    template_name: String,
    handlebars: Handlebars<'static>,
}

impl TemplateGenerator {
    /// Create a new template generator
    pub fn new(template_name: &str) -> Self {
        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(false);
        
        Self {
            template_name: template_name.to_string(),
            handlebars,
        }
    }
    
    /// Generate protocol plugin structure
    pub fn generate(&self, name: &str, output: &Path) -> Result<()> {
        let project_dir = output.join(name);
        
        // Create directory structure
        self.create_directory_structure(&project_dir)?;
        
        // Generate files from templates
        self.generate_files(name, &project_dir)?;
        
        Ok(())
    }
    
    /// Create directory structure
    fn create_directory_structure(&self, root: &Path) -> Result<()> {
        // Create directories
        let dirs = [
            "src",
            "src/protocol",
            "src/transport",
            "src/config",
            "tests",
            "examples",
            "docs",
        ];
        
        for dir in &dirs {
            fs::create_dir_all(root.join(dir))?;
        }
        
        Ok(())
    }
    
    /// Generate files from templates
    fn generate_files(&self, name: &str, root: &Path) -> Result<()> {
        // Template context
        let context = json!({
            "name": name,
            "struct_name": to_pascal_case(name),
            "description": format!("{} protocol implementation", name),
            "year": chrono::Utc::now().year(),
        });
        
        // Generate Cargo.toml
        self.generate_cargo_toml(root, &context)?;
        
        // Generate lib.rs
        self.generate_lib_rs(root, &context)?;
        
        // Generate plugin.rs
        self.generate_plugin_rs(root, &context)?;
        
        // Generate protocol implementation
        self.generate_protocol_rs(root, &context)?;
        
        // Generate tests
        self.generate_tests(root, &context)?;
        
        // Generate example
        self.generate_example(root, &context)?;
        
        Ok(())
    }
    
    /// Generate Cargo.toml
    fn generate_cargo_toml(&self, root: &Path, context: &serde_json::Value) -> Result<()> {
        let template = r#"[package]
name = "{{name}}"
version = "0.1.0"
edition = "2021"
authors = ["VoltageEMS Team"]
description = "{{description}}"
license = "MIT"

[dependencies]
# Core dependencies
async-trait = "0.1"
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Protocol-specific dependencies
# Add your protocol-specific dependencies here

# Import comsrv as a library
comsrv = { path = "../../../" }

[dev-dependencies]
tokio-test = "0.4"
pretty_assertions = "1.0"

[[bin]]
name = "{{name}}_simulator"
path = "src/bin/simulator.rs"
"#;
        
        let rendered = self.handlebars.render_template(template, context)?;
        fs::write(root.join("Cargo.toml"), rendered)?;
        
        Ok(())
    }
    
    /// Generate lib.rs
    fn generate_lib_rs(&self, root: &Path, context: &serde_json::Value) -> Result<()> {
        let template = r#"//! {{description}}
//!
//! This crate provides a protocol plugin implementation for {{name}}.

pub mod plugin;
pub mod protocol;
pub mod transport;
pub mod config;

// Re-export main types
pub use plugin::{{struct_name}}Plugin;
pub use protocol::{{struct_name}}Protocol;

/// Protocol version
pub const PROTOCOL_VERSION: &str = env!("CARGO_PKG_VERSION");
"#;
        
        let rendered = self.handlebars.render_template(template, context)?;
        fs::write(root.join("src/lib.rs"), rendered)?;
        
        Ok(())
    }
    
    /// Generate plugin.rs
    fn generate_plugin_rs(&self, root: &Path, context: &serde_json::Value) -> Result<()> {
        let template = r#"//! {{struct_name}} Protocol Plugin Implementation

use async_trait::async_trait;
use std::collections::HashMap;
use serde_json::Value;

use comsrv::plugins::{
    ProtocolPlugin, ProtocolMetadata, ConfigTemplate, ValidationRule,
    protocol_plugin,
};
use comsrv::core::framework::traits::ComBase;
use comsrv::core::config::types::channel::ChannelConfig;
use comsrv::utils::Result;

use crate::protocol::{{struct_name}}Protocol;

/// {{struct_name}} protocol plugin
#[derive(Default)]
pub struct {{struct_name}}Plugin;

// Use the protocol_plugin macro to define metadata
protocol_plugin! {
    id: "{{name}}",
    name: "{{struct_name}} Protocol",
    version: "0.1.0",
    description: "{{description}}",
    author: "VoltageEMS Team",
    license: "MIT",
    features: ["telemetry", "control"],
    config: [
        {
            name: "host",
            description: "Server host address",
            param_type: "string",
            required: true,
        },
        {
            name: "port",
            description: "Server port",
            param_type: "int",
            required: true,
            default: 8080,
            validation: {
                min: 1,
                max: 65535,
            }
        },
        {
            name: "timeout",
            description: "Connection timeout in seconds",
            param_type: "int",
            required: false,
            default: 30,
            validation: {
                min: 1,
                max: 300,
            }
        }
    ]
}

#[async_trait]
impl ProtocolPlugin for {{struct_name}}Plugin {
    fn metadata(&self) -> ProtocolMetadata {
        PluginMetadataImpl::metadata()
    }
    
    fn config_template(&self) -> Vec<ConfigTemplate> {
        PluginMetadataImpl::config_template()
    }
    
    fn validate_config(&self, config: &HashMap<String, Value>) -> Result<()> {
        // Validate required fields
        if !config.contains_key("host") {
            return Err(comsrv::utils::Error::Config("Missing required field: host".into()));
        }
        
        if !config.contains_key("port") {
            return Err(comsrv::utils::Error::Config("Missing required field: port".into()));
        }
        
        // Validate port range
        if let Some(port) = config.get("port").and_then(|v| v.as_u64()) {
            if port == 0 || port > 65535 {
                return Err(comsrv::utils::Error::Config("Port must be between 1 and 65535".into()));
            }
        }
        
        Ok(())
    }
    
    async fn create_instance(
        &self,
        channel_config: ChannelConfig,
    ) -> Result<Box<dyn ComBase>> {
        let protocol = {{struct_name}}Protocol::new(channel_config)?;
        Ok(Box::new(protocol))
    }
    
    fn documentation(&self) -> &str {
        include_str!("../docs/README.md")
    }
}

// Register the plugin
comsrv::register_plugin!({{struct_name}}Plugin);
"#;
        
        let rendered = self.handlebars.render_template(template, context)?;
        fs::write(root.join("src/plugin.rs"), rendered)?;
        
        Ok(())
    }
    
    /// Generate protocol.rs
    fn generate_protocol_rs(&self, root: &Path, context: &serde_json::Value) -> Result<()> {
        let template = r#"//! {{struct_name}} Protocol Implementation

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, debug, error};

use comsrv::core::framework::{
    traits::ComBase,
    types::{ChannelStatus, PointData},
};
use comsrv::core::config::types::channel::ChannelConfig;
use comsrv::utils::{Result, Error};

/// {{struct_name}} protocol implementation
#[derive(Debug)]
pub struct {{struct_name}}Protocol {
    /// Channel configuration
    config: ChannelConfig,
    /// Connection status
    status: Arc<RwLock<ChannelStatus>>,
    /// Protocol-specific state
    // TODO: Add your protocol state here
}

impl {{struct_name}}Protocol {
    /// Create a new protocol instance
    pub fn new(config: ChannelConfig) -> Result<Self> {
        info!("Creating {{struct_name}} protocol instance for channel: {}", config.id);
        
        Ok(Self {
            config,
            status: Arc::new(RwLock::new(ChannelStatus::Disconnected)),
        })
    }
}

#[async_trait]
impl ComBase for {{struct_name}}Protocol {
    fn name(&self) -> &str {
        &self.config.name
    }
    
    fn channel_id(&self) -> String {
        self.config.id.clone()
    }
    
    fn protocol_type(&self) -> &str {
        "{{name}}"
    }
    
    fn get_parameters(&self) -> HashMap<String, String> {
        self.config.parameters.clone()
    }
    
    async fn is_running(&self) -> bool {
        let status = self.status.read().await;
        matches!(*status, ChannelStatus::Connected)
    }
    
    async fn start(&mut self) -> Result<()> {
        info!("Starting {{struct_name}} protocol");
        
        // TODO: Implement connection logic
        
        *self.status.write().await = ChannelStatus::Connected;
        Ok(())
    }
    
    async fn stop(&mut self) -> Result<()> {
        info!("Stopping {{struct_name}} protocol");
        
        // TODO: Implement disconnection logic
        
        *self.status.write().await = ChannelStatus::Disconnected;
        Ok(())
    }
    
    async fn status(&self) -> ChannelStatus {
        self.status.read().await.clone()
    }
    
    async fn update_status(&mut self, status: ChannelStatus) -> Result<()> {
        *self.status.write().await = status;
        Ok(())
    }
    
    async fn get_all_points(&self) -> Vec<PointData> {
        // TODO: Implement point retrieval
        Vec::new()
    }
    
    async fn read_point(&self, point_id: &str) -> Result<PointData> {
        // TODO: Implement point reading
        Err(Error::NotImplemented("read_point".into()))
    }
    
    async fn write_point(&mut self, point_id: &str, value: &str) -> Result<()> {
        // TODO: Implement point writing
        Err(Error::NotImplemented("write_point".into()))
    }
    
    async fn get_diagnostics(&self) -> HashMap<String, String> {
        let mut diagnostics = HashMap::new();
        diagnostics.insert("protocol".to_string(), "{{name}}".to_string());
        diagnostics.insert("version".to_string(), crate::PROTOCOL_VERSION.to_string());
        // TODO: Add protocol-specific diagnostics
        diagnostics
    }
}
"#;
        
        let rendered = self.handlebars.render_template(template, context)?;
        fs::write(root.join("src/protocol.rs"), rendered)?;
        
        // Create empty transport and config modules
        fs::write(root.join("src/transport.rs"), "//! Transport layer implementation\n")?;
        fs::write(root.join("src/config.rs"), "//! Configuration utilities\n")?;
        
        Ok(())
    }
    
    /// Generate tests
    fn generate_tests(&self, root: &Path, context: &serde_json::Value) -> Result<()> {
        let template = r#"//! Integration tests for {{name}} protocol

use {{name}}::{{"{"}}{{struct_name}}Plugin, {{struct_name}}Protocol{{"}"}};
use comsrv::plugins::ProtocolPlugin;

#[tokio::test]
async fn test_plugin_metadata() {
    let plugin = {{struct_name}}Plugin::default();
    let metadata = plugin.metadata();
    
    assert_eq!(metadata.id, "{{name}}");
    assert_eq!(metadata.name, "{{struct_name}} Protocol");
}

#[tokio::test]
async fn test_config_validation() {
    let plugin = {{struct_name}}Plugin::default();
    let mut config = std::collections::HashMap::new();
    
    // Test missing required fields
    assert!(plugin.validate_config(&config).is_err());
    
    // Test valid config
    config.insert("host".to_string(), serde_json::json!("localhost"));
    config.insert("port".to_string(), serde_json::json!(8080));
    assert!(plugin.validate_config(&config).is_ok());
}
"#;
        
        let rendered = self.handlebars.render_template(template, context)?;
        fs::write(root.join("tests/integration_test.rs"), rendered)?;
        
        Ok(())
    }
    
    /// Generate example
    fn generate_example(&self, root: &Path, context: &serde_json::Value) -> Result<()> {
        let template = r#"//! Example usage of {{name}} protocol

use {{name}}::{{struct_name}}Plugin;
use comsrv::plugins::ProtocolPlugin;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Create plugin instance
    let plugin = {{struct_name}}Plugin::default();
    
    // Show metadata
    let metadata = plugin.metadata();
    println!("Protocol: {} v{}", metadata.name, metadata.version);
    println!("Description: {}", metadata.description);
    
    // Generate example configuration
    let config = plugin.generate_example_config();
    println!("\nExample configuration:");
    println!("{}", serde_yaml::to_string(&config)?);
    
    Ok(())
}
"#;
        
        let rendered = self.handlebars.render_template(template, context)?;
        fs::write(root.join("examples/basic.rs"), rendered)?;
        
        // Create simulator binary stub
        let simulator = r#"//! Protocol simulator for testing

fn main() {
    println!("{{struct_name}} protocol simulator");
    // TODO: Implement simulator
}
"#;
        
        let rendered = self.handlebars.render_template(simulator, context)?;
        fs::create_dir_all(root.join("src/bin"))?;
        fs::write(root.join("src/bin/simulator.rs"), rendered)?;
        
        // Create README
        let readme = r#"# {{struct_name}} Protocol Plugin

{{description}}

## Quick Start

```bash
# Build the plugin
cargo build

# Run tests
cargo test

# Run example
cargo run --example basic

# Start simulator
cargo run --bin {{name}}_simulator
```

## Configuration

See the generated example configuration for available options.

## Development

TODO: Add development instructions
"#;
        
        let rendered = self.handlebars.render_template(readme, context)?;
        fs::write(root.join("docs/README.md"), rendered)?;
        
        Ok(())
    }
}

/// Convert snake_case to PascalCase
fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}