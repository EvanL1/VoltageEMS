//! Model management module (formerly modsrv-cli)
//!
//! Provides functionality to manage products and instances

use anyhow::Result;
use clap::Subcommand;
use tracing::{info, warn};

pub mod client;
pub mod csv_loader;

#[cfg(feature = "lib-mode")]
use crate::{context::ServiceContext, lib_api};

#[cfg(feature = "lib-mode")]
use voltage_rtdb::Rtdb;

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
    /// List all imported products
    #[command(about = "Show all products that have been imported to ModSrv")]
    List,

    /// Show available products to import
    #[command(about = "List products available in the products/ directory")]
    Available,

    /// Import a product from CSV files
    #[command(
        about = "Import a product definition from CSV files",
        long_about = "Import a product definition from the products/{name}/ directory.\nThis will load measurements.csv, actions.csv, and properties.csv files."
    )]
    Import {
        /// Product name (directory name in products/)
        name: String,
    },

    /// Get product details
    #[command(about = "Show detailed information about a product")]
    Get {
        /// Product name
        name: String,
    },

    /// Delete a product
    #[command(about = "Delete a product and all its instances")]
    Delete {
        /// Product name
        name: String,
        /// Force deletion without confirmation
        #[arg(short, long)]
        force: bool,
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

pub async fn handle_command(
    cmd: ModelCommands,
    service_ctx: Option<&ServiceContext>,
    base_url: Option<&str>,
) -> Result<()> {
    match cmd {
        ModelCommands::Products { command } => {
            handle_product_command(command, service_ctx, base_url).await
        },
        ModelCommands::Instances { command } => {
            handle_instance_command(command, service_ctx, base_url).await
        },
    }
}

async fn handle_product_command(
    cmd: ProductCommands,
    service_ctx: Option<&ServiceContext>,
    base_url: Option<&str>,
) -> Result<()> {
    // Determine which mode to use
    #[cfg(feature = "lib-mode")]
    let use_lib_api = service_ctx.is_some();
    #[cfg(not(feature = "lib-mode"))]
    let use_lib_api = false;

    match cmd {
        // Local operations (always use CSV loader)
        ProductCommands::Available => {
            csv_loader::list_available_products()?;
        },
        ProductCommands::Import { name } => {
            let product = csv_loader::load_product_from_csv(&name)?;

            if use_lib_api {
                #[cfg(feature = "lib-mode")]
                {
                    warn!("Product import: offline unsupported, use monarch sync");
                }
            } else {
                // Online mode: use HTTP API
                let url = base_url.ok_or_else(|| {
                    anyhow::anyhow!(
                        "Base URL required for online mode. Please set MODSRV_URL or use --offline"
                    )
                })?;
                let client = client::ModelClient::new(url)?;
                client.import_product(product).await?;
                info!("Product '{}' imported", name);
            }
        },

        // Remote operations (support both modes)
        ProductCommands::List => {
            if use_lib_api {
                #[cfg(feature = "lib-mode")]
                {
                    let ctx = service_ctx.expect("ServiceContext should be available in lib-mode");
                    let modsrv = ctx.modsrv()?;
                    let service = lib_api::models::ModelsService::new(modsrv);
                    let products = service.list_products().await?;
                    println!("Products: {}", serde_json::to_string_pretty(&products)?);
                }
            } else {
                let url = base_url.ok_or_else(|| {
                    anyhow::anyhow!(
                        "Base URL required for online mode. Please set MODSRV_URL or use --offline"
                    )
                })?;
                let client = client::ModelClient::new(url)?;
                let products = client.list_products().await?;
                println!("Products: {}", serde_json::to_string_pretty(&products)?);
            }
        },
        ProductCommands::Get { name } => {
            if use_lib_api {
                #[cfg(feature = "lib-mode")]
                {
                    let ctx = service_ctx.expect("ServiceContext should be available in lib-mode");
                    let modsrv = ctx.modsrv()?;
                    let service = lib_api::models::ModelsService::new(modsrv);
                    let product = service.get_product(&name).await?;
                    println!(
                        "Product '{}': {}",
                        name,
                        serde_json::to_string_pretty(&product)?
                    );
                }
            } else {
                let url = base_url.ok_or_else(|| {
                    anyhow::anyhow!(
                        "Base URL required for online mode. Please set MODSRV_URL or use --offline"
                    )
                })?;
                let client = client::ModelClient::new(url)?;
                let product = client.get_product(&name).await?;
                println!(
                    "Product '{}': {}",
                    name,
                    serde_json::to_string_pretty(&product)?
                );
            }
        },
        ProductCommands::Delete { name, force } => {
            if !force {
                println!(
                    "Delete product '{}'? This will also delete all instances. [y/N]",
                    name
                );
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("Cancelled");
                    return Ok(());
                }
            }

            if use_lib_api {
                #[cfg(feature = "lib-mode")]
                {
                    warn!("Product delete: offline unsupported");
                }
            } else {
                let url = base_url.ok_or_else(|| {
                    anyhow::anyhow!(
                        "Base URL required for online mode. Please set MODSRV_URL or use --offline"
                    )
                })?;
                let client = client::ModelClient::new(url)?;
                client.delete_product(&name).await?;
                info!("Product '{}' deleted", name);
            }
        },
    }
    Ok(())
}

async fn handle_instance_command(
    cmd: InstanceCommands,
    service_ctx: Option<&ServiceContext>,
    base_url: Option<&str>,
) -> Result<()> {
    // Determine which mode to use
    #[cfg(feature = "lib-mode")]
    let use_lib_api = service_ctx.is_some();
    #[cfg(not(feature = "lib-mode"))]
    let use_lib_api = false;

    match cmd {
        InstanceCommands::List { product } => {
            if use_lib_api {
                #[cfg(feature = "lib-mode")]
                {
                    let ctx = service_ctx.expect("ServiceContext should be available in lib-mode");
                    let modsrv = ctx.modsrv()?;
                    let service = lib_api::models::ModelsService::new(modsrv);

                    if product.is_some() {
                        warn!("Product filtering not yet supported in offline mode");
                        warn!("Showing all instances:");
                    }
                    let instances = service.list_instances().await?;
                    println!("Instances: {}", serde_json::to_string_pretty(&instances)?);
                }
            } else {
                let url = base_url.ok_or_else(|| {
                    anyhow::anyhow!(
                        "Base URL required for online mode. Please set MODSRV_URL or use --offline"
                    )
                })?;
                let client = client::ModelClient::new(url)?;
                let instances = client.list_instances(product.as_deref()).await?;
                println!("Instances: {}", serde_json::to_string_pretty(&instances)?);
            }
        },
        InstanceCommands::Create {
            product,
            name,
            props,
        } => {
            if use_lib_api {
                #[cfg(feature = "lib-mode")]
                {
                    let ctx = service_ctx.expect("ServiceContext should be available in lib-mode");
                    let modsrv = ctx.modsrv()?;
                    let service = lib_api::models::ModelsService::new(modsrv);

                    // Convert properties to JSON values
                    let props_map: std::collections::HashMap<String, serde_json::Value> = props
                        .into_iter()
                        .map(|(k, v)| {
                            // Try to parse as number first, fallback to string
                            let value = v
                                .parse::<f64>()
                                .map(serde_json::Value::from)
                                .unwrap_or_else(|_| serde_json::Value::from(v));
                            (k, value)
                        })
                        .collect();

                    // Generate unique instance_id (simple timestamp-based for now)
                    let instance_id = (std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .expect("System time should be after UNIX epoch")
                        .as_secs()
                        % 65535) as u32;

                    // Create request struct
                    let request = modsrv::CreateInstanceRequest {
                        instance_id,
                        instance_name: name.clone(),
                        product_name: product,
                        properties: props_map,
                    };

                    let _instance = service.create_instance(request).await?;
                    info!("Instance '{}' created (ID: {})", name, instance_id);
                }
            } else {
                let url = base_url.ok_or_else(|| {
                    anyhow::anyhow!(
                        "Base URL required for online mode. Please set MODSRV_URL or use --offline"
                    )
                })?;
                let client = client::ModelClient::new(url)?;
                let props_map: std::collections::HashMap<String, String> =
                    props.into_iter().collect();
                client.create_instance(&product, &name, props_map).await?;
                info!("Instance '{}' created", name);
            }
        },
        InstanceCommands::Get { name } => {
            if use_lib_api {
                #[cfg(feature = "lib-mode")]
                {
                    let ctx = service_ctx.expect("ServiceContext should be available in lib-mode");
                    let modsrv = ctx.modsrv()?;
                    let service = lib_api::models::ModelsService::new(modsrv);
                    let instance = service.get_instance(&name).await?;
                    println!(
                        "Instance '{}': {}",
                        name,
                        serde_json::to_string_pretty(&instance)?
                    );
                }
            } else {
                let url = base_url.ok_or_else(|| {
                    anyhow::anyhow!(
                        "Base URL required for online mode. Please set MODSRV_URL or use --offline"
                    )
                })?;
                let client = client::ModelClient::new(url)?;
                let instance = client.get_instance(&name).await?;
                println!(
                    "Instance '{}': {}",
                    name,
                    serde_json::to_string_pretty(&instance)?
                );
            }
        },
        InstanceCommands::Update { name, props } => {
            if use_lib_api {
                #[cfg(feature = "lib-mode")]
                {
                    warn!("Instance update not yet supported in offline mode");
                    warn!("Please use online mode or update configuration via monarch sync");
                }
            } else {
                let url = base_url.ok_or_else(|| {
                    anyhow::anyhow!(
                        "Base URL required for online mode. Please set MODSRV_URL or use --offline"
                    )
                })?;
                let client = client::ModelClient::new(url)?;
                let props_map: std::collections::HashMap<String, String> =
                    props.into_iter().collect();
                client.update_instance(&name, props_map).await?;
                info!("Instance '{}' updated", name);
            }
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

            if use_lib_api {
                #[cfg(feature = "lib-mode")]
                {
                    let ctx = service_ctx.expect("ServiceContext should be available in lib-mode");
                    let modsrv = ctx.modsrv()?;
                    let service = lib_api::models::ModelsService::new(modsrv);
                    service.delete_instance(&name).await?;
                    info!("Instance '{}' deleted", name);
                }
            } else {
                let url = base_url.ok_or_else(|| {
                    anyhow::anyhow!(
                        "Base URL required for online mode. Please set MODSRV_URL or use --offline"
                    )
                })?;
                let client = client::ModelClient::new(url)?;
                client.delete_instance(&name).await?;
                info!("Instance '{}' deleted", name);
            }
        },
        InstanceCommands::Data { name, point_type } => {
            if use_lib_api {
                #[cfg(feature = "lib-mode")]
                {
                    use voltage_rtdb::KeySpaceConfig;

                    let ctx = service_ctx.expect("ServiceContext should be available in lib-mode");
                    let modsrv = ctx.modsrv()?;

                    // Query instance from SQLite to get instance_id
                    let instance_row: Option<(u32, String)> = sqlx::query_as(
                        "SELECT instance_id, instance_name FROM instances WHERE instance_name = ?",
                    )
                    .bind(&name)
                    .fetch_optional(&modsrv.sqlite_pool)
                    .await?;

                    let (instance_id, instance_name) = instance_row
                        .ok_or_else(|| anyhow::anyhow!("Instance '{}' not found", name))?;

                    let keyspace = KeySpaceConfig::production();
                    let rtdb = &modsrv.rtdb;

                    // Determine which data to fetch based on point_type (zero-allocation comparison)
                    let fetch_measurements = point_type
                        .as_deref()
                        .is_none_or(|s| !s.eq_ignore_ascii_case("A"));
                    let fetch_actions = point_type
                        .as_deref()
                        .is_none_or(|s| !s.eq_ignore_ascii_case("M"));

                    println!("=== Instance Runtime Data ===\n");
                    println!("Instance: {} (ID: {})", instance_name, instance_id);
                    println!();

                    // Fetch measurement data
                    if fetch_measurements {
                        let m_key = keyspace.instance_measurement_key(instance_id);
                        let m_data = rtdb.hash_get_all(m_key.as_ref()).await?;

                        // Filter out timestamp fields (ts:*)
                        // Optimization: use std::str::from_utf8 to borrow-validate, avoiding to_vec() copy
                        let m_points: std::collections::HashMap<u32, String> = m_data
                            .into_iter()
                            .filter(|(field, _)| !field.starts_with("ts:"))
                            .filter_map(|(field, value_bytes)| {
                                let point_id = field.parse::<u32>().ok()?;
                                let value_str = std::str::from_utf8(&value_bytes)
                                    .ok()
                                    .map(|s| s.to_string())?;
                                Some((point_id, value_str))
                            })
                            .collect();

                        println!("--- Measurements (M) ---");
                        if m_points.is_empty() {
                            println!("  (No measurement data)");
                        } else {
                            println!("{:<12} {:<20}", "Point ID", "Value");
                            println!("{}", "─".repeat(34));
                            let mut sorted_points: Vec<_> = m_points.into_iter().collect();
                            sorted_points.sort_by_key(|(id, _)| *id);
                            for (point_id, value) in sorted_points {
                                println!("{:<12} {:<20}", point_id, value);
                            }
                        }
                        println!();
                    }

                    // Fetch action data
                    if fetch_actions {
                        let a_key = keyspace.instance_action_key(instance_id);
                        let a_data = rtdb.hash_get_all(a_key.as_ref()).await?;

                        // Filter out timestamp fields (ts:*)
                        // Optimization: use std::str::from_utf8 to borrow-validate, avoiding to_vec() copy
                        let a_points: std::collections::HashMap<u32, String> = a_data
                            .into_iter()
                            .filter(|(field, _)| !field.starts_with("ts:"))
                            .filter_map(|(field, value_bytes)| {
                                let point_id = field.parse::<u32>().ok()?;
                                let value_str = std::str::from_utf8(&value_bytes)
                                    .ok()
                                    .map(|s| s.to_string())?;
                                Some((point_id, value_str))
                            })
                            .collect();

                        println!("--- Actions (A) ---");
                        if a_points.is_empty() {
                            println!("  (No action data)");
                        } else {
                            println!("{:<12} {:<20}", "Point ID", "Value");
                            println!("{}", "─".repeat(34));
                            let mut sorted_points: Vec<_> = a_points.into_iter().collect();
                            sorted_points.sort_by_key(|(id, _)| *id);
                            for (point_id, value) in sorted_points {
                                println!("{:<12} {:<20}", point_id, value);
                            }
                        }
                    }
                }
            } else {
                warn!("Instance data command only supported in offline mode (lib API)");
                warn!("Please use --offline flag or run monarch in lib-mode");
            }
        },
    }
    Ok(())
}
