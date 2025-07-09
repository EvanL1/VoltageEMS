//! CLI Command Implementations
//!
//! This module contains the implementation of all CLI commands for protocol development.

use std::path::Path;
use std::fs;
use colored::*;
use prettytable::{Table, row, cell};

use crate::core::plugins::{PluginRegistry, ConfigGenerator};
use crate::cli::{ConfigFormat, template_generator, test_framework};
use crate::utils::{Result, Error};

/// Create a new protocol plugin
pub async fn new_protocol(name: &str, output: &Path, template: Option<&str>) -> Result<()> {
    println!("{} Creating new protocol plugin: {}", "✨".green(), name.cyan());
    
    // Validate protocol name
    if !is_valid_protocol_name(name) {
        return Err(Error::Config(
            "Protocol name must be lowercase with underscores (e.g., modbus_tcp)".to_string()
        ));
    }
    
    // Select template
    let template_name = template.unwrap_or("default");
    println!("Using template: {}", template_name.yellow());
    
    // Generate protocol structure
    let generator = template_generator::TemplateGenerator::new(template_name);
    generator.generate(name, output)?;
    
    println!("{} Protocol plugin created successfully!", "✅".green());
    println!("\nNext steps:");
    println!("  1. cd {}", output.join(name).display());
    println!("  2. cargo build");
    println!("  3. cargo test");
    println!("  4. comsrv-cli test {name}");
    
    Ok(())
}

/// List available protocol plugins
pub async fn list_protocols(verbose: bool) -> Result<()> {
    let registry = PluginRegistry::global();
    let registry = registry.read().unwrap();
    
    let plugins = registry.get_all_plugins();
    
    if plugins.is_empty() {
        println!("{} No protocol plugins found", "ℹ".blue());
        return Ok(());
    }
    
    if verbose {
        // Detailed table view
        let mut table = Table::new();
        table.add_row(row![
            "ID".bold(),
            "Name".bold(),
            "Version".bold(),
            "Features".bold(),
            "Status".bold()
        ]);
        
        for plugin in plugins {
            let metadata = plugin.metadata();
            table.add_row(row![
                metadata.id.cyan(),
                metadata.name,
                metadata.version.yellow(),
                metadata.features.join(", "),
                "Active".green()
            ]);
        }
        
        table.printstd();
        
        // Show statistics
        let stats = registry.get_statistics();
        println!("\n{} Statistics:", "📊".blue());
        println!("  Total plugins: {}", stats.total_plugins);
        println!("  Enabled plugins: {}", stats.enabled_plugins);
        println!("  Plugin types:");
        for (ptype, count) in &stats.plugin_types {
            println!("    - {}: {count}", ptype);
        }
    } else {
        // Simple list
        println!("{} Available protocol plugins:", "📦".blue());
        for plugin in plugins {
            let metadata = plugin.metadata();
            println!("  {} {} - {}", 
                metadata.id.cyan(),
                format!("v{}", metadata.version).dimmed(),
                metadata.description
            );
        }
    }
    
    Ok(())
}

/// Generate configuration for a protocol
pub async fn generate_config(
    protocol: &str,
    format: ConfigFormat,
    output: Option<&Path>,
) -> Result<()> {
    println!("{} Generating configuration for protocol: {}", "⚙".blue(), protocol.cyan());
    
    // Get plugin from registry
    let plugin = PluginRegistry::get_global(protocol)
        .ok_or_else(|| Error::Config(format!("Protocol '{}' not found", protocol)))?;
    
    // Generate configuration
    let config = plugin.generate_example_config();
    
    // Convert to requested format
    let output_str = match format {
        ConfigFormat::Yaml => serde_yaml::to_string(&config)?,
        ConfigFormat::Json => serde_json::to_string_pretty(&config)?,
        ConfigFormat::Toml => toml::to_string_pretty(&config)?,
    };
    
    // Write output
    if let Some(path) = output {
        fs::write(path, output_str)?;
        println!("{} Configuration written to: {}", "✅".green(), path.display());
    } else {
        println!("\n{output_str}");
    }
    
    Ok(())
}

/// Validate a configuration file
pub async fn validate_config(config_path: &Path, protocol: Option<&str>) -> Result<()> {
    println!("{} Validating configuration: {}", "🔍".blue(), config_path.display());
    
    // Read configuration file
    let config_str = fs::read_to_string(config_path)?;
    
    // Detect format from extension
    let ext = config_path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or("yaml");
    
    // Parse configuration
    let config: serde_json::Map<String, serde_json::Value> = match ext {
        "json" => serde_json::from_str(&config_str)?,
        "toml" => toml::from_str(&config_str)?,
        _ => serde_yaml::from_str(&config_str)?,
    };
    
    // Determine protocol if not specified
    let protocol_id = if let Some(p) = protocol {
        p.to_string()
    } else if let Some(p) = config.get("protocol").and_then(|v| v.as_str()) {
        p.to_string()
    } else {
        return Err(Error::Config("Protocol not specified".to_string()));
    };
    
    // Get plugin and validate
    let plugin = PluginRegistry::get_global(&protocol_id)
        .ok_or_else(|| Error::Config(format!("Protocol '{}' not found", protocol_id)))?;
    
    let config_map: std::collections::HashMap<String, serde_json::Value> = 
        config.into_iter().collect();
    
    match plugin.validate_config(&config_map) {
        Ok(()) => {
            println!("{} Configuration is valid!", "✅".green());
        }
        Err(e) => {
            println!("{} Configuration validation failed:", "❌".red());
            println!("  {e}");
            return Err(e);
        }
    }
    
    Ok(())
}

/// Test a protocol implementation
pub async fn test_protocol(
    protocol: &str,
    config: Option<&Path>,
    test: Option<&str>,
) -> Result<()> {
    println!("{} Testing protocol: {}", "🧪".blue(), protocol.cyan());
    
    // Create test framework
    let mut framework = test_framework::TestFramework::new(protocol)?;
    
    // Load configuration if provided
    if let Some(config_path) = config {
        framework.load_config(config_path)?;
    }
    
    // Run tests
    if let Some(test_name) = test {
        // Run specific test
        println!("Running test: {}", test_name.yellow());
        framework.run_test(test_name).await?;
    } else {
        // Run all tests
        println!("Running all tests...");
        framework.run_all_tests().await?;
    }
    
    // Print results
    framework.print_results();
    
    Ok(())
}

/// Generate protocol documentation
pub async fn generate_docs(protocol: &str, output: &Path) -> Result<()> {
    println!("{} Generating documentation for protocol: {}", "📚".blue(), protocol.cyan());
    
    // Get plugin
    let plugin = PluginRegistry::get_global(protocol)
        .ok_or_else(|| Error::Config(format!("Protocol '{}' not found", protocol)))?;
    
    // Create output directory
    fs::create_dir_all(output)?;
    
    // Generate README
    let readme_path = output.join("README.md");
    let metadata = plugin.metadata();
    let mut readme = String::new();
    
    readme.push_str(&format!("# {} Protocol Plugin\n\n", metadata.name));
    readme.push_str(&format!("**Version:** {}\n", metadata.version));
    readme.push_str(&format!("**Author:** {}\n", metadata.author));
    readme.push_str(&format!("**License:** {}\n\n", metadata.license));
    readme.push_str(&format!("## Description\n\n{}\n\n", metadata.description));
    
    // Add configuration documentation
    readme.push_str("## Configuration\n\n");
    readme.push_str("### Parameters\n\n");
    
    for template in plugin.config_template() {
        readme.push_str(&format!("#### {}\n\n", template.name));
        readme.push_str(&format!("- **Description:** {}\n", template.description));
        readme.push_str(&format!("- **Type:** {}\n", template.param_type));
        readme.push_str(&format!("- **Required:** {}\n", template.required));
        if let Some(default) = &template.default_value {
            readme.push_str(&format!("- **Default:** `{}`\n", default));
        }
        readme.push_str("\n");
    }
    
    // Add custom documentation
    if !plugin.documentation().is_empty() {
        readme.push_str("## Additional Documentation\n\n");
        readme.push_str(plugin.documentation());
    }
    
    fs::write(&readme_path, readme)?;
    
    // Generate example configuration
    let example_path = output.join("example_config.yaml");
    let example_config = plugin.generate_example_config();
    let yaml = serde_yaml::to_string(&example_config)?;
    fs::write(&example_path, yaml)?;
    
    println!("{} Documentation generated in: {}", "✅".green(), output.display());
    
    Ok(())
}

/// Start protocol simulator
pub async fn simulate_protocol(
    protocol: &str,
    config: Option<&Path>,
    bind: &str,
) -> Result<()> {
    println!("{} Starting protocol simulator: {}", "🚀".blue(), protocol.cyan());
    println!("Bind address: {}", bind.yellow());
    
    // TODO: Implement protocol simulator
    println!("{} Protocol simulator not yet implemented", "⚠".yellow());
    
    Ok(())
}

/// Analyze protocol performance
pub async fn analyze_protocol(
    input: &Path,
    protocol: Option<&str>,
    output: Option<&Path>,
) -> Result<()> {
    println!("{} Analyzing protocol data: {}", "📊".blue(), input.display());
    
    // TODO: Implement protocol analyzer
    println!("{} Protocol analyzer not yet implemented", "⚠".yellow());
    
    Ok(())
}

/// Validate protocol name
fn is_valid_protocol_name(name: &str) -> bool {
    name.chars().all(|c| c.is_ascii_lowercase() || c == '_')
}