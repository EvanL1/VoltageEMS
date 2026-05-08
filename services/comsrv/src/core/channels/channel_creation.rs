//! Channel creation and factory methods
//!
//! Contains all protocol-specific channel creation logic and
//! Redis storage initialization for the ChannelManager.

use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::core::channels::channel_entry::ChannelEntry;
use crate::core::channels::channel_manager::ChannelManager;
use crate::core::channels::converters::convert_to_point_configs;
use crate::core::channels::factory::create_virtual_channel;
use crate::core::config::{ChannelConfig, RuntimeChannelConfig};
use crate::error::{ComSrvError, Result};
use crate::protocols::core::file_logging::{ChannelFileLogHandler, FileLogLevel};
use crate::protocols::core::log_handlers::{CompositeLogHandler, TracingLogHandler};
use crate::protocols::core::logging::{ChannelLogConfig, ChannelLogHandler, LogEventType};
use crate::protocols::gateway::ChannelRuntime;
use crate::store::RedisDataStore;
use voltage_rtdb::Rtdb;

#[cfg(feature = "modbus")]
use crate::core::channels::converters::convert_to_modbus_point_configs;
#[cfg(feature = "modbus")]
use crate::core::channels::factory::{create_modbus_channel, create_modbus_rtu_channel};

#[cfg(all(target_os = "linux", feature = "gpio"))]
use crate::core::channels::factory::create_gpio_channel;

#[cfg(all(feature = "can", target_os = "linux"))]
use crate::core::channels::converters::{
    convert_can_to_point_configs, convert_to_can_point_configs,
};
#[cfg(all(feature = "can", target_os = "linux"))]
use crate::core::channels::factory::create_can_channel;

#[cfg(feature = "voltage_485")]
use crate::core::channels::factory::create_voltage_485_channel;

/// Get the base directory for channel log files.
/// Uses VOLTAGE_LOG_DIR environment variable if set, otherwise falls back to "/app/logs".
fn get_channel_log_base_dir() -> String {
    let base = std::env::var("VOLTAGE_LOG_DIR").unwrap_or_else(|_| "/app/logs".to_string());
    format!("{}/comsrv/channels", base)
}

impl<R: Rtdb + 'static> ChannelManager<R> {
    /// Configure logging for a channel based on ChannelLoggingConfig.
    ///
    /// Sets up both tracing and file logging handlers when enabled.
    /// Returns the composite log handler for hot-reload support.
    fn configure_channel_logging(
        protocol: &mut Box<dyn ChannelRuntime>,
        channel_id: u32,
        channel_name: &str,
        logging_config: &crate::core::config::ChannelLoggingConfig,
    ) -> Arc<dyn ChannelLogHandler> {
        // Create composite handler with tracing
        let mut composite = CompositeLogHandler::new().with_handler(Arc::new(TracingLogHandler));

        // Add file logging if enabled
        if logging_config.enabled {
            let level = FileLogLevel::parse(logging_config.level.as_deref());
            let log_dir = get_channel_log_base_dir();

            let file_handler = ChannelFileLogHandler::new(&log_dir)
                .with_level(level)
                .with_channel(channel_id, channel_name);

            composite.add_handler(Arc::new(file_handler));

            info!(
                "Ch{} file logging enabled (level={:?}, dir={})",
                channel_id, level, log_dir
            );
        }

        // Create Arc and clone for return value (for hot-reload support)
        let handler: Arc<dyn ChannelLogHandler> = Arc::new(composite);
        protocol.set_log_handler(handler.clone());

        // Configure log config based on logging level
        let log_config = if logging_config.enabled {
            let level = logging_config.level.as_deref().unwrap_or("info");
            if level.eq_ignore_ascii_case("debug") {
                ChannelLogConfig::all()
            } else {
                ChannelLogConfig::new()
                    .with_raw_packets(true)
                    .enable_event(LogEventType::RawPacket)
            }
        } else {
            ChannelLogConfig::default()
        };

        protocol.set_log_config(log_config);
        handler
    }

    /// Create channel
    ///
    /// Returns an Arc to the created ChannelEntry for convenience.
    pub async fn create_channel(
        &self,
        channel_config: Arc<ChannelConfig>,
    ) -> Result<Arc<ChannelEntry<R>>> {
        let channel_id = channel_config.id();

        // Bounds check for pre-allocated Vec
        let slot = self
            .channels
            .get(channel_id as usize)
            .ok_or_else(|| ComSrvError::invalid_channel_id(channel_id))?;

        // Validate channel doesn't exist (O(1) atomic load)
        if slot.load().is_some() {
            return Err(ComSrvError::channel_exists(channel_id));
        }

        // Convert to RuntimeChannelConfig and load configuration from SQLite
        let mut runtime_config = RuntimeChannelConfig::from_base_arc(Arc::clone(&channel_config));
        self.load_channel_configuration(&mut runtime_config).await?;
        let runtime_config = Arc::new(runtime_config);

        info!(
            "Ch{}: T={} S={} C={} A={} pts",
            channel_id,
            runtime_config.telemetry_points.len(),
            runtime_config.signal_points.len(),
            runtime_config.control_points.len(),
            runtime_config.adjustment_points.len()
        );

        // Get protocol using normalized name
        let protocol_name = crate::utils::normalize_protocol_name(runtime_config.protocol());
        let base_config = Arc::clone(&runtime_config.base);

        // Redis fields appear naturally on the first successful poll via
        // ShmRedisSync; comsrv must not pre-seed inst:{id}:M with placeholder
        // zeros (it would violate comsrv↔modsrv data independence).

        // Branch based on protocol type - create ChannelEntry directly
        let entry = self
            .create_channel_by_protocol(&protocol_name, channel_id, &runtime_config, base_config)
            .await?;

        let entry = Arc::new(entry);

        // Register channel with all subsystems
        self.register_channel_subsystems(channel_id, slot, &entry, &runtime_config);

        info!("Ch{} created ({})", channel_id, protocol_name);
        Ok(entry)
    }

    /// Create a ChannelEntry for the given protocol type.
    async fn create_channel_by_protocol(
        &self,
        protocol_name: &str,
        channel_id: u32,
        runtime_config: &Arc<RuntimeChannelConfig>,
        base_config: Arc<ChannelConfig>,
    ) -> Result<ChannelEntry<R>> {
        match protocol_name {
            "virtual" => {
                self.create_virtual_channel_impl(channel_id, runtime_config, base_config)
                    .await
            },
            #[cfg(feature = "modbus")]
            "modbus_tcp" => {
                self.create_modbus_channel_impl(channel_id, runtime_config, base_config)
                    .await
            },
            #[cfg(feature = "modbus")]
            "modbus_rtu" => {
                self.create_modbus_rtu_channel_impl(channel_id, runtime_config, base_config)
                    .await
            },
            #[cfg(all(target_os = "linux", feature = "gpio"))]
            "gpio" | "di_do" | "dido" => {
                self.create_gpio_channel_impl(channel_id, runtime_config, base_config)
                    .await
            },
            #[cfg(all(feature = "can", target_os = "linux"))]
            "can" => {
                self.create_can_channel_impl(channel_id, runtime_config, base_config)
                    .await
            },
            #[cfg(feature = "voltage_485")]
            "voltage_485" => {
                self.create_voltage_485_channel_impl(channel_id, runtime_config, base_config)
                    .await
            },
            #[cfg(feature = "iec61850")]
            "iec61850" => {
                self.create_iec61850_channel_impl(channel_id, runtime_config, base_config)
                    .await
            },
            _ => {
                #[allow(unused_mut)]
                let mut supported = String::from("virtual");
                #[cfg(feature = "modbus")]
                supported.push_str(", modbus_tcp, modbus_rtu");
                #[cfg(all(target_os = "linux", feature = "gpio"))]
                supported.push_str(", gpio/di_do");
                #[cfg(all(feature = "can", target_os = "linux"))]
                supported.push_str(", can");
                #[cfg(feature = "voltage_485")]
                supported.push_str(", voltage_485");
                #[cfg(feature = "iec61850")]
                supported.push_str(", iec61850");

                Err(anyhow::anyhow!(
                    "Unsupported protocol '{}' for channel {}. Supported: {}",
                    protocol_name,
                    channel_id,
                    supported
                )
                .into())
            },
        }
    }

    /// Register a newly created channel with all subsystems.
    fn register_channel_subsystems(
        &self,
        channel_id: u32,
        slot: &arc_swap::ArcSwapOption<ChannelEntry<R>>,
        entry: &Arc<ChannelEntry<R>>,
        runtime_config: &Arc<RuntimeChannelConfig>,
    ) {
        // 1. Atomic store (publish channel to be visible)
        slot.store(Some(Arc::clone(entry)));

        // 2. Register command_tx with cache
        if let (Some(cache), Some(tx)) = (&self.command_tx_cache, &entry.command_tx) {
            cache.register(channel_id, tx.clone());
        }

        // 3. Register with SHM listener for event-driven M2C dispatch
        if let (Some(listener), Some(tx)) = (&self.shm_listener, &entry.command_tx) {
            listener.register_channel(channel_id, tx.clone());
            debug!(
                "Ch{} registered with ShmListener for event-driven dispatch",
                channel_id
            );
        }

        // 4. Register in active channel index for O(1) iteration
        self.active_channel_ids.insert(channel_id);

        // 5. Dynamic Slot Allocation: Add channel to ChannelIndex
        if let (Some(index), Some(bitmap)) = (&self.dynamic_channel_index, &self.slot_bitmap) {
            let type_counts = [
                runtime_config.telemetry_points.len() as u32,
                runtime_config.signal_points.len() as u32,
                runtime_config.control_points.len() as u32,
                runtime_config.adjustment_points.len() as u32,
            ];
            let total: u32 = type_counts.iter().sum();
            if total > 0 {
                let mut bitmap_guard = bitmap.write();
                match index.add_channel(channel_id, type_counts, &mut bitmap_guard) {
                    Ok(layout) => {
                        debug!(
                            "Ch{} slot allocated: base={}, total={}",
                            channel_id, layout.base_slot, layout.total_points
                        );
                    },
                    Err(e) => {
                        warn!("Ch{} slot allocation failed: {}", channel_id, e);
                    },
                }
            }
        }

        // 6. Write channel name to Redis hash (fire-and-forget via spawn)
        let rtdb = Arc::clone(&self.rtdb);
        let channel_name = entry.channel_config.name().to_string();
        tokio::spawn(async move {
            let keyspace = voltage_model::KeySpaceConfig::production_cached();
            let channels_key = keyspace.channels_hash_key();
            match rtdb
                .hash_set(
                    &channels_key,
                    &channel_id.to_string(),
                    bytes::Bytes::from(channel_name),
                )
                .await
            {
                Err(e) => {
                    warn!(
                        "Ch{} failed to write name to Redis hash ({}): {}",
                        channel_id, channels_key, e
                    );
                },
                _ => {
                    debug!(
                        "Ch{} name written to Redis hash: {}",
                        channel_id, channels_key
                    );
                },
            }
        });
    }

    /// Create virtual channel entry.
    async fn create_virtual_channel_impl(
        &self,
        channel_id: u32,
        runtime_config: &Arc<RuntimeChannelConfig>,
        base_config: Arc<ChannelConfig>,
    ) -> Result<ChannelEntry<R>> {
        debug!("Ch{} creating virtual channel", channel_id);

        let store = self.create_data_store();
        let point_configs = convert_to_point_configs(runtime_config);
        store.set_point_configs(channel_id, point_configs.clone());
        store.start_flush_task().await;

        let mut protocol = create_virtual_channel(channel_id, runtime_config.name(), point_configs);

        let log_handler = Self::configure_channel_logging(
            &mut protocol,
            channel_id,
            runtime_config.name(),
            &base_config.logging,
        );

        let poll_interval_ms = runtime_config
            .base
            .parameters
            .get("poll_interval_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000);

        Ok(ChannelEntry::new(
            protocol,
            store,
            base_config,
            "virtual".to_string(),
            poll_interval_ms,
            log_handler,
        ))
    }

    /// Create Modbus TCP channel entry.
    #[cfg(feature = "modbus")]
    async fn create_modbus_channel_impl(
        &self,
        channel_id: u32,
        runtime_config: &Arc<RuntimeChannelConfig>,
        base_config: Arc<ChannelConfig>,
    ) -> Result<ChannelEntry<R>> {
        debug!("Ch{} creating Modbus TCP channel", channel_id);

        let store = self.create_data_store();
        let point_configs = convert_to_modbus_point_configs(runtime_config);
        store.set_point_configs(channel_id, point_configs.clone());
        store.start_flush_task().await;

        let params = &runtime_config.base.parameters;
        let host = params
            .get("host")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| {
                info!(
                    "Ch{} host not configured, using default: 127.0.0.1",
                    channel_id
                );
                "127.0.0.1"
            });
        let port = params
            .get("port")
            .and_then(|v| v.as_u64())
            .map(|n| n as u16)
            .unwrap_or_else(|| {
                info!("Ch{} port not configured, using default: 502", channel_id);
                502
            });

        let io_timeout_ms = params.get("read_timeout_ms").and_then(|v| v.as_u64());
        if let Some(timeout) = io_timeout_ms {
            debug!("Ch{} using read_timeout_ms: {}ms", channel_id, timeout);
        }

        let mut protocol =
            create_modbus_channel(channel_id, host, port, point_configs, io_timeout_ms);

        let log_handler = Self::configure_channel_logging(
            &mut protocol,
            channel_id,
            runtime_config.name(),
            &base_config.logging,
        );

        let poll_interval_ms = params
            .get("poll_interval_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000);

        Ok(ChannelEntry::new(
            protocol,
            store,
            base_config,
            "modbus_tcp".to_string(),
            poll_interval_ms,
            log_handler,
        ))
    }

    /// Create Modbus RTU (serial) channel entry.
    #[cfg(feature = "modbus")]
    async fn create_modbus_rtu_channel_impl(
        &self,
        channel_id: u32,
        runtime_config: &Arc<RuntimeChannelConfig>,
        base_config: Arc<ChannelConfig>,
    ) -> Result<ChannelEntry<R>> {
        debug!("Ch{} creating Modbus RTU channel", channel_id);

        let store = self.create_data_store();
        let point_configs = convert_to_modbus_point_configs(runtime_config);
        store.set_point_configs(channel_id, point_configs.clone());
        store.start_flush_task().await;

        let params = &runtime_config.base.parameters;
        let device = params
            .get("device")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| {
                info!(
                    "Ch{} device not configured, using default: /dev/ttyUSB0",
                    channel_id
                );
                "/dev/ttyUSB0"
            });
        let baud_rate = params
            .get("baud_rate")
            .and_then(|v| v.as_u64())
            .map(|n| n as u32)
            .unwrap_or_else(|| {
                info!(
                    "Ch{} baud_rate not configured, using default: 9600",
                    channel_id
                );
                9600
            });

        let io_timeout_ms = params.get("read_timeout_ms").and_then(|v| v.as_u64());
        if let Some(timeout) = io_timeout_ms {
            debug!("Ch{} using read_timeout_ms: {}ms", channel_id, timeout);
        }

        let mut protocol =
            create_modbus_rtu_channel(channel_id, device, baud_rate, point_configs, io_timeout_ms);

        let log_handler = Self::configure_channel_logging(
            &mut protocol,
            channel_id,
            runtime_config.name(),
            &base_config.logging,
        );

        let poll_interval_ms = params
            .get("poll_interval_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000);

        Ok(ChannelEntry::new(
            protocol,
            store,
            base_config,
            "modbus_rtu".to_string(),
            poll_interval_ms,
            log_handler,
        ))
    }

    /// Create GPIO channel entry for DI/DO.
    #[cfg(all(target_os = "linux", feature = "gpio"))]
    async fn create_gpio_channel_impl(
        &self,
        channel_id: u32,
        runtime_config: &Arc<RuntimeChannelConfig>,
        base_config: Arc<ChannelConfig>,
    ) -> Result<ChannelEntry<R>> {
        debug!("Ch{} creating GPIO channel", channel_id);

        let store = self.create_data_store();
        let point_configs = convert_to_point_configs(runtime_config);
        store.set_point_configs(channel_id, point_configs);
        store.start_flush_task().await;

        let mut protocol = create_gpio_channel(channel_id, runtime_config);

        let log_handler = Self::configure_channel_logging(
            &mut protocol,
            channel_id,
            runtime_config.name(),
            &base_config.logging,
        );

        // GPIO needs faster polling (default 200ms for responsive DI detection)
        let poll_interval_ms = runtime_config
            .base
            .parameters
            .get("poll_interval_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(200);

        Ok(ChannelEntry::new(
            protocol,
            store,
            base_config,
            "gpio".to_string(),
            poll_interval_ms,
            log_handler,
        ))
    }

    /// Create CAN channel entry.
    #[cfg(all(feature = "can", target_os = "linux"))]
    async fn create_can_channel_impl(
        &self,
        channel_id: u32,
        runtime_config: &Arc<RuntimeChannelConfig>,
        base_config: Arc<ChannelConfig>,
    ) -> Result<ChannelEntry<R>> {
        debug!("Ch{} creating CAN channel", channel_id);

        let store = self.create_data_store();
        let can_point_configs = convert_to_can_point_configs(runtime_config);
        let point_configs = convert_can_to_point_configs(runtime_config);

        if can_point_configs.is_empty() {
            warn!("Ch{} has no CAN point mappings configured", channel_id);
        }

        store.set_point_configs(channel_id, point_configs);
        store.start_flush_task().await;

        let params = &runtime_config.base.parameters;
        let can_interface = params
            .get("device")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| {
                info!(
                    "Ch{} CAN device not configured, using default: can0",
                    channel_id
                );
                "can0"
            });

        let mut protocol = create_can_channel(channel_id, can_interface, can_point_configs)?;

        let log_handler = Self::configure_channel_logging(
            &mut protocol,
            channel_id,
            runtime_config.name(),
            &base_config.logging,
        );

        // CAN is event-driven, needs faster polling (default 200ms)
        let poll_interval_ms = params
            .get("poll_interval_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(200);

        Ok(ChannelEntry::new(
            protocol,
            store,
            base_config,
            "can".to_string(),
            poll_interval_ms,
            log_handler,
        ))
    }

    /// Create Voltage-485 channel entry.
    #[cfg(feature = "voltage_485")]
    async fn create_voltage_485_channel_impl(
        &self,
        channel_id: u32,
        runtime_config: &Arc<RuntimeChannelConfig>,
        base_config: Arc<ChannelConfig>,
    ) -> Result<ChannelEntry<R>> {
        debug!("Ch{} creating Voltage-485 channel", channel_id);

        let store = self.create_data_store();
        let point_configs = convert_to_point_configs(runtime_config);
        store.set_point_configs(channel_id, point_configs);
        store.start_flush_task().await;

        let params = &runtime_config.base.parameters;

        let mut protocol =
            create_voltage_485_channel(channel_id, runtime_config.name(), params, runtime_config);

        let log_handler = Self::configure_channel_logging(
            &mut protocol,
            channel_id,
            runtime_config.name(),
            &base_config.logging,
        );

        let poll_interval_ms = params
            .get("poll_interval_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000);

        Ok(ChannelEntry::new(
            protocol,
            store,
            base_config,
            "voltage_485".to_string(),
            poll_interval_ms,
            log_handler,
        ))
    }

    /// Create IEC 61850 MMS channel entry.
    #[cfg(feature = "iec61850")]
    async fn create_iec61850_channel_impl(
        &self,
        channel_id: u32,
        runtime_config: &Arc<RuntimeChannelConfig>,
        base_config: Arc<ChannelConfig>,
    ) -> Result<ChannelEntry<R>> {
        use crate::protocols::adapters::iec61850::{Iec61850Channel, Iec61850ParamsConfig};
        use crate::protocols::core::point::{
            Iec61850Address, PointConfig, ProtocolAddress, TransformConfig,
        };

        debug!("Ch{} creating IEC 61850 MMS channel", channel_id);

        let store = self.create_data_store();
        let params = &runtime_config.base.parameters;

        // Build Iec61850ParamsConfig from the channel's `parameters` block.
        let address = params
            .get("address")
            .and_then(|v| v.as_str())
            .unwrap_or("127.0.0.1:102")
            .to_string();
        let connect_timeout_ms = params
            .get("connect_timeout_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(10_000);
        let request_timeout_ms = params
            .get("request_timeout_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(5_000);
        let poll_interval_ms = params
            .get("poll_interval_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(1_000);

        let iec61850_params = Iec61850ParamsConfig {
            address,
            connect_timeout_ms,
            request_timeout_ms,
        };

        // Convert points: parse protocol_mappings JSON → Iec61850Address.
        // Expected protocol_mappings format: {"address": "domain/item$..."}
        let mut point_configs: Vec<PointConfig> = Vec::new();

        let parse_iec61850_point = |protocol_mappings: &Option<String>| -> Option<Iec61850Address> {
            let json_str = protocol_mappings.as_deref()?;
            let obj: serde_json::Value = serde_json::from_str(json_str).ok()?;
            let addr_str = obj.get("address")?.as_str()?;
            Iec61850Address::parse(addr_str).ok()
        };

        for tp in &runtime_config.telemetry_points {
            if let Some(addr) = parse_iec61850_point(&tp.base.protocol_mappings) {
                point_configs.push(PointConfig {
                    id: tp.base.point_id,
                    point_type: voltage_model::PointType::Telemetry,
                    name: Some(tp.base.signal_name.clone()),
                    address: ProtocolAddress::Iec61850(addr),
                    transform: TransformConfig {
                        scale: tp.scale,
                        offset: tp.offset,
                        reverse: tp.reverse,
                        ..Default::default()
                    },
                    poll_group: None,
                    enabled: true,
                });
            } else {
                warn!(
                    "Ch{} telemetry point {} has no valid IEC 61850 address in protocol_mappings",
                    channel_id, tp.base.point_id
                );
            }
        }

        for sp in &runtime_config.signal_points {
            if let Some(addr) = parse_iec61850_point(&sp.base.protocol_mappings) {
                point_configs.push(PointConfig {
                    id: sp.base.point_id,
                    point_type: voltage_model::PointType::Signal,
                    name: Some(sp.base.signal_name.clone()),
                    address: ProtocolAddress::Iec61850(addr),
                    transform: TransformConfig {
                        reverse: sp.reverse,
                        ..Default::default()
                    },
                    poll_group: None,
                    enabled: true,
                });
            }
        }

        store.set_point_configs(channel_id, point_configs.clone());
        store.start_flush_task().await;

        let mut protocol: Box<dyn crate::protocols::gateway::ChannelRuntime> =
            Box::new(Iec61850Channel::new(
                channel_id,
                runtime_config.name(),
                &iec61850_params,
                point_configs,
            ));

        let log_handler = Self::configure_channel_logging(
            &mut protocol,
            channel_id,
            runtime_config.name(),
            &base_config.logging,
        );

        Ok(ChannelEntry::new(
            protocol,
            store,
            base_config,
            "iec61850".to_string(),
            poll_interval_ms,
            log_handler,
        ))
    }

    /// Create RedisDataStore with optional shared memory support
    fn create_data_store(&self) -> Arc<RedisDataStore<R>> {
        let store = RedisDataStore::new(Arc::clone(&self.rtdb), Arc::clone(&self.routing_cache));

        let store = if let Some(handle) = &self.shm_handle {
            store.with_shm_handle(Arc::clone(handle))
        } else {
            store
        };

        Arc::new(store)
    }

    /// Load channel configuration from SQLite
    async fn load_channel_configuration(
        &self,
        runtime_config: &mut RuntimeChannelConfig,
    ) -> Result<()> {
        use crate::core::config::sqlite_loader::ComsrvSqliteLoader;

        if let Some(ref pool) = self.sqlite_pool {
            let loader = ComsrvSqliteLoader::with_pool(pool.clone());
            loader.load_runtime_channel_points(runtime_config).await?;
        } else {
            let db_path =
                std::env::var("VOLTAGE_DB_PATH").unwrap_or_else(|_| "data/voltage.db".to_string());
            let loader = ComsrvSqliteLoader::new(&db_path).await?;
            loader.load_runtime_channel_points(runtime_config).await?;
        }
        Ok(())
    }
}
