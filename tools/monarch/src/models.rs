//! Model management module
//!
//! Provides functionality to manage products and instances via HTTP API

use anyhow::Result;
use clap::Subcommand;
use tracing::info;

pub mod client;
pub mod csv_loader;

#[derive(Subcommand)]
pub enum ModelCommands {
    /// Manage products (device type templates)
    #[command(about = "Manage product definitions and templates")]
    Products {
        #[command(subcommand)]
        command: ProductCommands,
    },

    /// Manage instances (device configurations)
    #[command(about = "Manage device instances based on product templates")]
    Instances {
        #[command(subcommand)]
        command: InstanceCommands,
    },
}

#[derive(Subcommand)]
pub enum ProductCommands {
    /// List all built-in products
    #[command(about = "Show all built-in products from voltage-model")]
    List,

    /// Show available products in products/ directory (for development)
    #[command(about = "List product definitions in the products/ directory")]
    Available,

    /// Get product details
    #[command(about = "Show detailed information about a built-in product")]
    Get {
        /// Product name
        name: String,
    },
}

#[derive(Subcommand)]
pub enum InstanceCommands {
    /// List all instances
    #[command(about = "Show all device instances")]
    List {
        /// Filter by product type
        #[arg(short, long)]
        product: Option<String>,
    },

    /// Create a new instance
    #[command(about = "Create a new device instance from a product template")]
    Create {
        /// Product name
        product: String,
        /// Instance name
        name: String,
        /// Properties in key=value format
        #[arg(short, long, value_parser = parse_property)]
        props: Vec<(String, String)>,
    },

    /// Get instance details
    #[command(about = "Show detailed information about an instance")]
    Get {
        /// Instance name
        name: String,
    },

    /// Update an instance
    #[command(about = "Update instance properties")]
    Update {
        /// Instance name
        name: String,
        /// Properties to update in key=value format
        #[arg(short, long, value_parser = parse_property)]
        props: Vec<(String, String)>,
    },

    /// Delete an instance
    #[command(about = "Delete a device instance")]
    Delete {
        /// Instance name
        name: String,
        /// Force deletion without confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// Get instance runtime data
    #[command(about = "Get realtime measurement and action point data from RTDB")]
    Data {
        /// Instance name
        name: String,
        /// Point type filter (M for measurements, A for actions, both if not specified)
        #[arg(short = 't', long)]
        point_type: Option<String>,
    },
}

fn parse_property(s: &str) -> Result<(String, String), String> {
    let parts: Vec<&str> = s.splitn(2, '=').collect();
    if parts.len() != 2 {
        return Err(format!(
            "Invalid property format: '{}'. Expected key=value",
            s
        ));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

pub async fn handle_command(cmd: ModelCommands, base_url: &str) -> Result<()> {
    match cmd {
        ModelCommands::Products { command } => handle_product_command(command, base_url).await,
        ModelCommands::Instances { command } => handle_instance_command(command, base_url).await,
    }
}

async fn handle_product_command(cmd: ProductCommands, base_url: &str) -> Result<()> {
    match cmd {
        // Local operation (reads from filesystem)
        ProductCommands::Available => {
            csv_loader::list_available_products()?;
        },

        // Remote operations via HTTP
        ProductCommands::List => {
            let client = client::ModelClient::new(base_url)?;
            let products = client.list_products().await?;
            println!("Products: {}", serde_json::to_string_pretty(&products)?);
        },
        ProductCommands::Get { name } => {
            let client = client::ModelClient::new(base_url)?;
            let product = client.get_product(&name).await?;
            println!(
                "Product '{}': {}",
                name,
                serde_json::to_string_pretty(&product)?
            );
        },
    }
    Ok(())
}

async fn handle_instance_command(cmd: InstanceCommands, base_url: &str) -> Result<()> {
    let client = client::ModelClient::new(base_url)?;

    match cmd {
        InstanceCommands::List { product } => {
            let instances = client.list_instances(product.as_deref()).await?;
            println!("Instances: {}", serde_json::to_string_pretty(&instances)?);
        },
        InstanceCommands::Create {
            product,
            name,
            props,
        } => {
            let props_map: std::collections::HashMap<String, String> = props.into_iter().collect();
            client.create_instance(&product, &name, props_map).await?;
            info!("Instance '{}' created", name);
        },
        InstanceCommands::Get { name } => {
            let instance = client.get_instance(&name).await?;
            println!(
                "Instance '{}': {}",
                name,
                serde_json::to_string_pretty(&instance)?
            );
        },
        InstanceCommands::Update { name, props } => {
            let props_map: std::collections::HashMap<String, String> = props.into_iter().collect();
            client.update_instance(&name, props_map).await?;
            info!("Instance '{}' updated", name);
        },
        InstanceCommands::Delete { name, force } => {
            if !force {
                println!("Delete instance '{}'? [y/N]", name);
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("Cancelled");
                    return Ok(());
                }
            }

            client.delete_instance(&name).await?;
            info!("Instance '{}' deleted", name);
        },
        InstanceCommands::Data { name, point_type } => {
            // Instance data requires direct RTDB access, use rtdb command instead
            let filter_msg = match point_type.as_deref() {
                Some("M") => " (measurements only)",
                Some("A") => " (actions only)",
                _ => "",
            };
            println!(
                "Instance data{} for '{}' requires running services.",
                filter_msg, name
            );
            println!("Use: monarch rtdb inspect inst:<id>:M --full");
            println!("Or:  monarch rtdb inspect inst:<id>:A --full");
            println!(
                "\nTo find instance ID, use: monarch models instances get {}",
                name
            );
        },
    }
    Ok(())
}
