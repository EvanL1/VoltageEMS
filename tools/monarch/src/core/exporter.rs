//! Configuration export module
//!
//! This module provides functionality to export configuration from the SQLite
//! database back to YAML/CSV files.

use anyhow::{Context, Result};
use serde_yaml;
use sqlx::{Row, SqlitePool};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use tracing::{debug, info};
use voltage_config::{
    comsrv::{ChannelConfig, ComsrvConfig},
    modsrv::ModsrvConfig,
    rulesrv::{RuleConfig, RulesrvConfig},
};

/// Result type for export operations
#[derive(Debug, Default)]
pub struct ExportResult {
    pub files_exported: Vec<String>,
    pub records_exported: usize,
}

/// Configuration exporter
pub struct ConfigExporter {
    pool: SqlitePool,
}

impl ConfigExporter {
    /// Create a new exporter
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Export configuration for a specific service
    pub async fn export_service(
        &self,
        service: &str,
        output_dir: impl AsRef<Path>,
    ) -> Result<ExportResult> {
        info!("Exporting configuration for service: {}", service);

        let output_dir = output_dir.as_ref();

        // Ensure output directory exists
        std::fs::create_dir_all(output_dir).context("Failed to create output directory")?;

        debug!("Exporting to directory: {:?}", output_dir);

        let result = match service {
            "comsrv" => self.export_comsrv(output_dir).await?,
            "modsrv" => self.export_modsrv(output_dir).await?,
            "rulesrv" => self.export_rulesrv(output_dir).await?,
            _ => {
                return Err(anyhow::anyhow!("Unknown service: {}", service));
            },
        };

        info!(
            "Successfully exported {} files with {} records for {}",
            result.files_exported.len(),
            result.records_exported,
            service
        );
        Ok(result)
    }

    async fn export_comsrv(&self, output_dir: &Path) -> Result<ExportResult> {
        let mut result = ExportResult::default();

        // Export service configuration
        let mut service_config = self.export_comsrv_config().await?;
        let yaml_path = output_dir.join("comsrv.yaml");

        // Export channels
        let channels = self.export_channels().await?;
        service_config.channels = channels.clone();
        result.records_exported += channels.len();

        let yaml_content = serde_yaml::to_string(&service_config)?;
        std::fs::write(&yaml_path, yaml_content)?;
        result.files_exported.push("comsrv.yaml".to_string());

        // Export telemetry points to CSV
        let telemetry_points = self.export_points("telemetry").await?;
        if !telemetry_points.is_empty() {
            self.write_points_csv(output_dir.join("telemetry.csv"), &telemetry_points)?;
            result.files_exported.push("telemetry.csv".to_string());
            result.records_exported += telemetry_points.len();
        }

        // Export signal points to CSV
        let signal_points = self.export_points("signal").await?;
        if !signal_points.is_empty() {
            self.write_points_csv(output_dir.join("signal.csv"), &signal_points)?;
            result.files_exported.push("signal.csv".to_string());
            result.records_exported += signal_points.len();
        }

        // Export control points to CSV
        let control_points = self.export_points("control").await?;
        if !control_points.is_empty() {
            self.write_points_csv(output_dir.join("control.csv"), &control_points)?;
            result.files_exported.push("control.csv".to_string());
            result.records_exported += control_points.len();
        }

        // Export adjustment points to CSV
        let adjustment_points = self.export_points("adjustment").await?;
        if !adjustment_points.is_empty() {
            self.write_points_csv(output_dir.join("adjustment.csv"), &adjustment_points)?;
            result.files_exported.push("adjustment.csv".to_string());
            result.records_exported += adjustment_points.len();
        }

        // Export protocol mappings for each channel
        for channel in &channels {
            let channel_mappings = self.export_channel_mappings(channel.id() as u32).await?;
            if !channel_mappings.is_empty() {
                for (mapping_type, mappings) in channel_mappings {
                    let mapping_dir = output_dir.join(format!("{}/mapping", channel.id()));
                    std::fs::create_dir_all(&mapping_dir)?;

                    let csv_path = mapping_dir.join(format!("{}.csv", mapping_type));
                    self.write_mappings_csv(&csv_path, &mappings)?;

                    let relative_path = format!("{}/mapping/{}.csv", channel.id(), mapping_type);
                    result.files_exported.push(relative_path);
                    result.records_exported += mappings.len();
                }
            }
        }

        Ok(result)
    }

    async fn export_modsrv(&self, output_dir: &Path) -> Result<ExportResult> {
        let mut result = ExportResult::default();

        // Export service configuration
        let service_config = self.export_modsrv_config().await?;
        let yaml_path = output_dir.join("modsrv.yaml");
        let yaml_content = serde_yaml::to_string(&service_config)?;
        std::fs::write(&yaml_path, yaml_content)?;
        result.files_exported.push("modsrv.yaml".to_string());

        // Export products hierarchy
        let products_hierarchy = self.export_products_hierarchy().await?;
        if !products_hierarchy.is_empty() {
            let products_yaml = output_dir.join("products.yaml");
            let records_count = products_hierarchy.len();
            let mut root: BTreeMap<String, BTreeMap<String, Option<String>>> = BTreeMap::new();
            root.insert("products".to_string(), products_hierarchy);
            std::fs::write(&products_yaml, serde_yaml::to_string(&root)?)?;
            result.files_exported.push("products.yaml".to_string());
            result.records_exported += records_count;
        }

        // Export instances
        let instances = self.export_instances().await?;
        if !instances.is_empty() {
            let instances_yaml = output_dir.join("instances.yaml");
            let instances_map: BTreeMap<String, serde_yaml::Value> =
                BTreeMap::from_iter([("instances".to_string(), serde_yaml::to_value(&instances)?)]);
            std::fs::write(&instances_yaml, serde_yaml::to_string(&instances_map)?)?;
            result.files_exported.push("instances.yaml".to_string());
            result.records_exported += instances.len();
        }

        // Export instance mappings to CSV files
        for instance_name in instances.keys() {
            let mappings = self.export_instance_mappings(instance_name).await?;
            if !mappings.is_empty() {
                let instance_dir = output_dir.join(format!("instances/{}", instance_name));
                std::fs::create_dir_all(&instance_dir)?;

                let csv_path = instance_dir.join("channel_routing.csv");
                self.write_instance_mappings_csv(&csv_path, &mappings)?;

                let relative_path = format!("instances/{}/channel_routing.csv", instance_name);
                result.files_exported.push(relative_path);
                result.records_exported += mappings.len();
            }
        }

        Ok(result)
    }

    async fn export_rulesrv(&self, output_dir: &Path) -> Result<ExportResult> {
        let mut result = ExportResult::default();

        // Export service configuration
        let service_config = self.export_rulesrv_config().await?;
        let yaml_path = output_dir.join("rulesrv.yaml");
        let yaml_content = serde_yaml::to_string(&service_config)?;
        std::fs::write(&yaml_path, yaml_content)?;
        result.files_exported.push("rulesrv.yaml".to_string());

        // Export rules
        let rules = self.export_rules().await?;
        if !rules.is_empty() {
            let rules_yaml = output_dir.join("rules.yaml");
            let rules_map: BTreeMap<String, Vec<RuleConfig>> =
                BTreeMap::from_iter([("rules".to_string(), rules.clone())]);
            std::fs::write(&rules_yaml, serde_yaml::to_string(&rules_map)?)?;
            result.files_exported.push("rules.yaml".to_string());
            result.records_exported += rules.len();
        }

        Ok(result)
    }

    // Helper methods for comsrv export
    async fn export_comsrv_config(&self) -> Result<ComsrvConfig> {
        let mut config = ComsrvConfig::default();

        // Query service configuration
        let rows = sqlx::query("SELECT key, value FROM service_config")
            .fetch_all(&self.pool)
            .await?;

        for row in rows {
            let key: String = row.try_get("key")?;
            let value: String = row.try_get("value")?;

            match key.as_str() {
                "service_name" => config.service.name = value,
                "api_host" => config.api.host = value,
                "service.port" | "api_port" | "port" => {
                    config.api.port = value.parse().unwrap_or(6000)
                },
                "redis.url" | "redis_url" => config.redis.url = value,
                "log_level" => config.logging.level = value,
                "log_file_prefix" => config.logging.file_prefix = Some(value),
                _ => {},
            }
        }

        Ok(config)
    }

    async fn export_channels(&self) -> Result<Vec<ChannelConfig>> {
        let mut channels = Vec::new();

        let rows = sqlx::query("SELECT channel_id, name, protocol, enabled, config FROM channels")
            .fetch_all(&self.pool)
            .await?;

        for row in rows {
            let channel_id: i64 = row.try_get("channel_id")?;
            let name: String = row.try_get("name")?;
            let protocol: Option<String> = row.try_get("protocol")?;
            let enabled: bool = row.try_get("enabled")?;
            let config_str: Option<String> = row.try_get("config")?;

            let mut channel = ChannelConfig {
                core: voltage_config::comsrv::ChannelCore {
                    id: channel_id as u16,
                    name,
                    description: None,
                    protocol: protocol.unwrap_or_else(|| "virtual".to_string()),
                    enabled,
                },
                parameters: HashMap::new(),
                logging: Default::default(),
            };

            // Parse protocol-specific parameters if available
            if let Some(config_json) = config_str {
                if let Ok(config_value) =
                    serde_json::from_str::<HashMap<String, serde_json::Value>>(&config_json)
                {
                    channel.parameters = config_value;
                }
            }

            channels.push(channel);
        }

        Ok(channels)
    }

    async fn export_points(&self, point_type: &str) -> Result<Vec<HashMap<String, String>>> {
        let telemetry_type = match point_type {
            "telemetry" => "T",
            "signal" => "S",
            "control" => "C",
            "adjustment" => "A",
            _ => return Ok(Vec::new()),
        };

        let query = "SELECT DISTINCT point_id, signal_name, scale, offset, unit, reverse, data_type, description
                     FROM points WHERE telemetry_type = ? ORDER BY point_id";

        let rows = sqlx::query(query)
            .bind(telemetry_type)
            .fetch_all(&self.pool)
            .await?;

        let mut points = Vec::new();
        for row in rows {
            let mut point = HashMap::new();
            point.insert(
                "point_id".to_string(),
                row.try_get::<i64, _>("point_id")?.to_string(),
            );
            point.insert("signal_name".to_string(), row.try_get("signal_name")?);

            if let Ok(scale) = row.try_get::<f64, _>("scale") {
                point.insert("scale".to_string(), scale.to_string());
            }
            if let Ok(offset) = row.try_get::<f64, _>("offset") {
                point.insert("offset".to_string(), offset.to_string());
            }
            if let Ok(Some(u)) = row.try_get::<Option<String>, _>("unit") {
                point.insert("unit".to_string(), u);
            }
            if let Ok(reverse) = row.try_get::<bool, _>("reverse") {
                point.insert("reverse".to_string(), reverse.to_string());
            }
            if let Ok(Some(dt)) = row.try_get::<Option<String>, _>("data_type") {
                point.insert("data_type".to_string(), dt);
            }
            if let Ok(Some(desc)) = row.try_get::<Option<String>, _>("description") {
                point.insert("description".to_string(), desc);
            }

            points.push(point);
        }

        Ok(points)
    }

    async fn export_channel_mappings(
        &self,
        channel_id: u32,
    ) -> Result<HashMap<String, Vec<HashMap<String, String>>>> {
        let mut mappings_by_type = HashMap::new();

        // First get the protocol type for this channel
        let protocol: String =
            sqlx::query_scalar("SELECT protocol FROM channels WHERE channel_id = ?")
                .bind(channel_id as i64)
                .fetch_optional(&self.pool)
                .await?
                .unwrap_or_else(|| "modbus_tcp".to_string());

        // Query the appropriate protocol-specific table
        let rows = match protocol.to_lowercase().as_str() {
            "modbus_tcp" | "modbus_rtu" | "modbus" => {
                sqlx::query(
                    "SELECT telemetry_type, point_id, slave_id, function_code, register_address,
                            data_type, byte_order, bit_position
                     FROM modbus_mappings WHERE channel_id = ? ORDER BY telemetry_type, point_id",
                )
                .bind(channel_id as i64)
                .fetch_all(&self.pool)
                .await?
            },
            "virtual" => {
                // For virtual, we need to handle different columns
                let virtual_rows = sqlx::query(
                    "SELECT telemetry_type, point_id, expression, update_interval, initial_value, noise_range
                     FROM virtual_mappings WHERE channel_id = ? ORDER BY telemetry_type, point_id",
                )
                .bind(channel_id as i64)
                .fetch_all(&self.pool)
                .await?;

                // Convert virtual protocol fields to match expected format
                for row in virtual_rows {
                    let telemetry_type: String = row.try_get("telemetry_type")?;
                    let mapping_type = match telemetry_type.as_str() {
                        "T" => "telemetry",
                        "S" => "signal",
                        "C" => "control",
                        "A" => "adjustment",
                        _ => continue,
                    };

                    let mut mapping = HashMap::new();
                    mapping.insert(
                        "point_id".to_string(),
                        row.try_get::<i64, _>("point_id")?.to_string(),
                    );

                    if let Ok(Some(expr)) = row.try_get::<Option<String>, _>("expression") {
                        mapping.insert("expression".to_string(), expr);
                    }
                    if let Ok(Some(ui)) = row.try_get::<Option<i64>, _>("update_interval") {
                        mapping.insert("update_interval".to_string(), ui.to_string());
                    }
                    if let Ok(Some(iv)) = row.try_get::<Option<f64>, _>("initial_value") {
                        mapping.insert("initial_value".to_string(), iv.to_string());
                    }
                    if let Ok(Some(nr)) = row.try_get::<Option<f64>, _>("noise_range") {
                        mapping.insert("noise_range".to_string(), nr.to_string());
                    }

                    mappings_by_type
                        .entry(mapping_type.to_string())
                        .or_insert_with(Vec::new)
                        .push(mapping);
                }
                // Return early for virtual since we already processed
                return Ok(mappings_by_type);
            },
            "iec60870" | "iec104" | "iec" => {
                sqlx::query(
                    "SELECT telemetry_type, point_id, asdu_address, object_address, type_id, cot, qualifier
                     FROM iec_mappings WHERE channel_id = ? ORDER BY telemetry_type, point_id",
                )
                .bind(channel_id as i64)
                .fetch_all(&self.pool)
                .await?
            },
            "grpc" => {
                sqlx::query(
                    "SELECT telemetry_type, point_id, service_name, method_name, field_path
                     FROM grpc_mappings WHERE channel_id = ? ORDER BY telemetry_type, point_id",
                )
                .bind(channel_id as i64)
                .fetch_all(&self.pool)
                .await?
            },
            _ => {
                // Unknown protocol, return empty mappings
                return Ok(mappings_by_type);
            }
        };

        // Process rows based on protocol (non-virtual protocols)
        for row in rows {
            let telemetry_type: String = row.try_get("telemetry_type")?;

            let mapping_type = match telemetry_type.as_str() {
                "T" => "telemetry",
                "S" => "signal",
                "C" => "control",
                "A" => "adjustment",
                _ => continue,
            };

            let mut mapping = HashMap::new();
            mapping.insert(
                "point_id".to_string(),
                row.try_get::<i64, _>("point_id")?.to_string(),
            );

            // Handle different protocols' fields
            match protocol.to_lowercase().as_str() {
                "modbus_tcp" | "modbus_rtu" | "modbus" => {
                    if let Ok(Some(sid)) = row.try_get::<Option<i64>, _>("slave_id") {
                        mapping.insert("slave_id".to_string(), sid.to_string());
                    }
                    if let Ok(Some(fc)) = row.try_get::<Option<i64>, _>("function_code") {
                        mapping.insert("function_code".to_string(), fc.to_string());
                    }
                    if let Ok(Some(ra)) = row.try_get::<Option<i64>, _>("register_address") {
                        mapping.insert("register_address".to_string(), ra.to_string());
                    }
                    if let Ok(Some(dt)) = row.try_get::<Option<String>, _>("data_type") {
                        mapping.insert("data_type".to_string(), dt);
                    }
                    if let Ok(Some(bo)) = row.try_get::<Option<String>, _>("byte_order") {
                        mapping.insert("byte_order".to_string(), bo);
                    }
                    if let Ok(Some(bp)) = row.try_get::<Option<i64>, _>("bit_position") {
                        mapping.insert("bit_position".to_string(), bp.to_string());
                    }
                },
                "iec60870" | "iec104" | "iec" => {
                    if let Ok(Some(asdu)) = row.try_get::<Option<i64>, _>("asdu_address") {
                        mapping.insert("asdu_address".to_string(), asdu.to_string());
                    }
                    if let Ok(Some(obj)) = row.try_get::<Option<i64>, _>("object_address") {
                        mapping.insert("object_address".to_string(), obj.to_string());
                    }
                    if let Ok(Some(tid)) = row.try_get::<Option<i64>, _>("type_id") {
                        mapping.insert("type_id".to_string(), tid.to_string());
                    }
                    if let Ok(Some(cot)) = row.try_get::<Option<i64>, _>("cot") {
                        mapping.insert("cot".to_string(), cot.to_string());
                    }
                    if let Ok(Some(qual)) = row.try_get::<Option<i64>, _>("qualifier") {
                        mapping.insert("qualifier".to_string(), qual.to_string());
                    }
                },
                "grpc" => {
                    if let Ok(Some(sn)) = row.try_get::<Option<String>, _>("service_name") {
                        mapping.insert("service_name".to_string(), sn);
                    }
                    if let Ok(Some(mn)) = row.try_get::<Option<String>, _>("method_name") {
                        mapping.insert("method_name".to_string(), mn);
                    }
                    if let Ok(Some(fp)) = row.try_get::<Option<String>, _>("field_path") {
                        mapping.insert("field_path".to_string(), fp);
                    }
                },
                _ => {},
            }

            mappings_by_type
                .entry(mapping_type.to_string())
                .or_insert_with(Vec::new)
                .push(mapping);
        }

        Ok(mappings_by_type)
    }

    // Helper methods for modsrv export
    async fn export_modsrv_config(&self) -> Result<ModsrvConfig> {
        let mut config = ModsrvConfig::default();

        let rows = sqlx::query("SELECT key, value FROM service_config")
            .fetch_all(&self.pool)
            .await?;

        for row in rows {
            let key: String = row.try_get("key")?;
            let value: String = row.try_get("value")?;

            match key.as_str() {
                "service_name" => config.service.name = value,
                "api_host" => config.api.host = value,
                "service.port" | "api_port" | "port" => {
                    config.api.port = value.parse().unwrap_or(6001)
                },
                "redis.url" | "redis_url" => config.redis.url = value,
                _ => {},
            }
        }

        Ok(config)
    }

    async fn export_products_hierarchy(&self) -> Result<BTreeMap<String, Option<String>>> {
        let mut hierarchy = BTreeMap::new();

        let rows =
            sqlx::query("SELECT product_name, parent_name FROM products ORDER BY product_name")
                .fetch_all(&self.pool)
                .await?;

        for row in rows {
            let product_name: String = row.try_get("product_name")?;
            let parent_name: Option<String> = row.try_get("parent_name")?;
            hierarchy.insert(product_name, parent_name);
        }

        Ok(hierarchy)
    }

    async fn export_instances(
        &self,
    ) -> Result<BTreeMap<String, BTreeMap<String, serde_yaml::Value>>> {
        let mut instances = BTreeMap::new();

        let rows = sqlx::query(
            "SELECT instance_id, instance_name, product_name, properties FROM instances ORDER BY instance_id",
        )
        .fetch_all(&self.pool)
        .await?;

        for row in rows {
            // instance_id is in the database but we use instance_name as the key
            let instance_name: String = row.try_get("instance_name")?;
            let product_name: String = row.try_get("product_name")?;
            let properties_str: Option<String> = row.try_get("properties")?;

            let mut instance_data = BTreeMap::new();
            instance_data.insert(
                "product_name".to_string(),
                serde_yaml::Value::String(product_name),
            );

            if let Some(props_json) = properties_str {
                if let Ok(props) = serde_json::from_str::<serde_json::Value>(&props_json) {
                    if let Ok(yaml_props) = serde_yaml::to_value(props) {
                        instance_data.insert("properties".to_string(), yaml_props);
                    }
                }
            }

            instances.insert(instance_name, instance_data);
        }

        Ok(instances)
    }

    async fn export_instance_mappings(
        &self,
        instance_name: &str,
    ) -> Result<Vec<HashMap<String, String>>> {
        // Query measurement_routing table (T/S → M)
        let measurement_query = "SELECT mr.channel_id, mr.channel_type, mr.channel_point_id,
                                'M' as instance_type, mr.measurement_id as instance_point_id
                     FROM measurement_routing mr
                     JOIN instances i ON mr.instance_id = i.instance_id
                     WHERE i.instance_name = ?
                     ORDER BY mr.channel_id, mr.channel_type, mr.channel_point_id";

        let measurement_rows = sqlx::query(measurement_query)
            .bind(instance_name)
            .fetch_all(&self.pool)
            .await?;

        // Query action_routing table (A → C/A)
        let action_query = "SELECT ar.channel_id, ar.channel_type, ar.channel_point_id,
                           'A' as instance_type, ar.action_id as instance_point_id
                     FROM action_routing ar
                     JOIN instances i ON ar.instance_id = i.instance_id
                     WHERE i.instance_name = ?
                     ORDER BY ar.channel_id, ar.channel_type, ar.channel_point_id";

        let action_rows = sqlx::query(action_query)
            .bind(instance_name)
            .fetch_all(&self.pool)
            .await?;

        let mut mappings = Vec::new();

        // Process measurement mappings
        for row in measurement_rows {
            let mut mapping = HashMap::new();
            mapping.insert(
                "channel_id".to_string(),
                row.try_get::<i64, _>("channel_id")?.to_string(),
            );
            mapping.insert("channel_type".to_string(), row.try_get("channel_type")?);
            mapping.insert(
                "channel_point_id".to_string(),
                row.try_get::<i64, _>("channel_point_id")?.to_string(),
            );
            mapping.insert("instance_type".to_string(), "M".to_string());
            mapping.insert(
                "instance_point_id".to_string(),
                row.try_get::<i64, _>("instance_point_id")?.to_string(),
            );

            if let Ok(Some(desc)) = row.try_get::<Option<String>, _>("description") {
                mapping.insert("description".to_string(), desc);
            }

            mappings.push(mapping);
        }

        // Process action mappings
        for row in action_rows {
            let mut mapping = HashMap::new();
            mapping.insert(
                "channel_id".to_string(),
                row.try_get::<i64, _>("channel_id")?.to_string(),
            );
            mapping.insert("channel_type".to_string(), row.try_get("channel_type")?);
            mapping.insert(
                "channel_point_id".to_string(),
                row.try_get::<i64, _>("channel_point_id")?.to_string(),
            );
            mapping.insert("instance_type".to_string(), "A".to_string());
            mapping.insert(
                "instance_point_id".to_string(),
                row.try_get::<i64, _>("instance_point_id")?.to_string(),
            );

            if let Ok(Some(desc)) = row.try_get::<Option<String>, _>("description") {
                mapping.insert("description".to_string(), desc);
            }

            mappings.push(mapping);
        }

        Ok(mappings)
    }

    // Helper methods for rulesrv export
    async fn export_rulesrv_config(&self) -> Result<RulesrvConfig> {
        let mut config = RulesrvConfig::default();

        let rows = sqlx::query("SELECT key, value FROM service_config")
            .fetch_all(&self.pool)
            .await?;

        for row in rows {
            let key: String = row.try_get("key")?;
            let value: String = row.try_get("value")?;

            match key.as_str() {
                "service_name" => config.service.name = value,
                "api_host" => config.api.host = value,
                "service.port" | "api_port" | "port" => {
                    config.api.port = value.parse().unwrap_or(6002)
                },
                "redis.url" | "redis_url" => config.redis.url = value,
                // execution_interval and batch_size are deprecated
                "execution_interval" | "batch_size" => {},
                _ => {},
            }
        }

        Ok(config)
    }

    async fn export_rules(&self) -> Result<Vec<RuleConfig>> {
        let mut rules = Vec::new();

        let rows =
            sqlx::query("SELECT id, name, description, flow_json, enabled, priority FROM rules")
                .fetch_all(&self.pool)
                .await?;

        for row in rows {
            let id: String = row.try_get("id")?;
            let name: String = row.try_get("name")?;
            let description: Option<String> = row.try_get("description")?;
            let flow_json_str: String = row.try_get("flow_json")?;
            let enabled: bool = row.try_get("enabled")?;
            let priority: i64 = row.try_get("priority")?;

            // Parse flow_json string to serde_json::Value
            let flow_json = serde_json::from_str(&flow_json_str).unwrap_or(serde_json::Value::Null);

            let rule = RuleConfig {
                core: voltage_config::rulesrv::RuleCore {
                    id,
                    name,
                    description,
                    enabled,
                    priority: priority as u32,
                },
                flow_json,
            };

            rules.push(rule);
        }

        Ok(rules)
    }

    // CSV writing helpers
    fn write_points_csv(
        &self,
        path: impl AsRef<Path>,
        points: &[HashMap<String, String>],
    ) -> Result<()> {
        let mut wtr = csv::Writer::from_path(path)?;

        // Write header
        let headers = [
            "point_id",
            "signal_name",
            "scale",
            "offset",
            "unit",
            "reverse",
            "data_type",
            "description",
        ];
        wtr.write_record(headers)?;

        // Write data rows
        for point in points {
            let mut row = Vec::new();
            for header in &headers {
                row.push(point.get(*header).map(|s| s.as_str()).unwrap_or(""));
            }
            wtr.write_record(&row)?;
        }

        wtr.flush()?;
        Ok(())
    }

    fn write_mappings_csv(&self, path: &Path, mappings: &[HashMap<String, String>]) -> Result<()> {
        if mappings.is_empty() {
            return Ok(());
        }

        let mut wtr = csv::Writer::from_path(path)?;

        // Write header based on available fields
        let headers = [
            "point_id",
            "slave_id",
            "function_code",
            "register_address",
            "data_type",
            "byte_order",
            "bit_position",
        ];
        wtr.write_record(headers)?;

        // Write data rows
        for mapping in mappings {
            let mut row = Vec::new();
            for header in &headers {
                row.push(mapping.get(*header).map(|s| s.as_str()).unwrap_or(""));
            }
            wtr.write_record(&row)?;
        }

        wtr.flush()?;
        Ok(())
    }

    fn write_instance_mappings_csv(
        &self,
        path: &Path,
        mappings: &[HashMap<String, String>],
    ) -> Result<()> {
        if mappings.is_empty() {
            return Ok(());
        }

        let mut wtr = csv::Writer::from_path(path)?;

        // Write header
        let headers = [
            "channel_id",
            "channel_type",
            "channel_point_id",
            "instance_type",
            "instance_point_id",
            "description",
        ];
        wtr.write_record(headers)?;

        // Write data rows
        for mapping in mappings {
            let mut row = Vec::new();
            for header in &headers {
                row.push(mapping.get(*header).map(|s| s.as_str()).unwrap_or(""));
            }
            wtr.write_record(&row)?;
        }

        wtr.flush()?;
        Ok(())
    }
}
