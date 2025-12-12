//! Product type definitions for VoltageEMS
//!
//! This module defines all product types and their point definitions as compile-time constants.
//! Products are defined in code rather than configuration files for:
//! - Compile-time type checking
//! - IDE support (autocomplete, go-to-definition)
//! - Zero runtime overhead
//! - Simplified deployment (no config files to manage)

use serde::{Deserialize, Serialize};

// ============================================================================
// Product Type Enum
// ============================================================================

/// All supported product types in VoltageEMS
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum ProductType {
    // === Station Level ===
    /// Station - top level site
    Station,
    /// Gateway - EMS device
    Gateway,

    // === PV System ===
    /// PV Inverter / DCDC
    PvInverter,

    // === Power Generation ===
    /// Diesel Generator
    Diesel,

    // === Load ===
    /// Load
    Load,

    // === Energy Storage - PCS ===
    /// Power Conversion System
    Pcs,

    // === BMS Hierarchy (Stack -> Cluster -> Pack -> Module -> Cell) ===
    /// Battery Stack - highest level in BMS hierarchy
    BatteryStack,
    /// Battery Cluster
    BatteryCluster,
    /// Battery Pack
    BatteryPack,
    /// Battery Module
    BatteryModule,
    /// Battery Cell - smallest unit
    BatteryCell,

    // === Environment ===
    /// Environment monitoring
    Env,
}

impl ProductType {
    /// Get all product types
    pub fn all() -> &'static [ProductType] {
        &[
            Self::Station,
            Self::Gateway,
            Self::PvInverter,
            Self::Diesel,
            Self::Load,
            Self::Pcs,
            Self::BatteryStack,
            Self::BatteryCluster,
            Self::BatteryPack,
            Self::BatteryModule,
            Self::BatteryCell,
            Self::Env,
        ]
    }

    /// Get product name as string (for database compatibility)
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Station => "station",
            Self::Gateway => "gateway",
            Self::PvInverter => "pv_inverter",
            Self::Diesel => "diesel",
            Self::Load => "load",
            Self::Pcs => "pcs",
            Self::BatteryStack => "battery_stack",
            Self::BatteryCluster => "battery_cluster",
            Self::BatteryPack => "battery_pack",
            Self::BatteryModule => "battery_module",
            Self::BatteryCell => "battery_cell",
            Self::Env => "env",
        }
    }

    /// Parse from string (use try_parse to avoid trait conflict)
    pub fn try_parse(s: &str) -> Option<Self> {
        match s {
            "station" => Some(Self::Station),
            "gateway" => Some(Self::Gateway),
            "pv_inverter" => Some(Self::PvInverter),
            "diesel" => Some(Self::Diesel),
            "load" => Some(Self::Load),
            "pcs" => Some(Self::Pcs),
            "battery_stack" => Some(Self::BatteryStack),
            "battery_cluster" => Some(Self::BatteryCluster),
            "battery_pack" => Some(Self::BatteryPack),
            "battery_module" => Some(Self::BatteryModule),
            "battery_cell" => Some(Self::BatteryCell),
            "env" => Some(Self::Env),
            _ => None,
        }
    }

    /// BMS hierarchy: get parent type (Cell -> Module -> Pack -> Cluster -> Stack)
    pub fn bms_parent(&self) -> Option<ProductType> {
        match self {
            Self::BatteryCell => Some(Self::BatteryModule),
            Self::BatteryModule => Some(Self::BatteryPack),
            Self::BatteryPack => Some(Self::BatteryCluster),
            Self::BatteryCluster => Some(Self::BatteryStack),
            Self::BatteryStack => None, // Top level
            _ => None,                  // Non-BMS types
        }
    }

    /// BMS hierarchy level (0=Cell, 4=Stack)
    pub fn bms_level(&self) -> Option<u8> {
        match self {
            Self::BatteryCell => Some(0),
            Self::BatteryModule => Some(1),
            Self::BatteryPack => Some(2),
            Self::BatteryCluster => Some(3),
            Self::BatteryStack => Some(4),
            _ => None,
        }
    }

    /// Check if this type can contain the given child type in BMS hierarchy
    pub fn can_contain(&self, child: &ProductType) -> bool {
        child.bms_parent() == Some(*self)
    }

    /// Check if this is a BMS type
    pub fn is_bms(&self) -> bool {
        self.bms_level().is_some()
    }

    /// Get valid parent types for this product (soft constraint)
    ///
    /// This defines the device hierarchy:
    /// - Station is the top level (no parent)
    /// - Site-level devices (Gateway, PV, Diesel, Load, PCS, Env) belong to Station
    /// - BMS hierarchy: Stack → Cluster → Pack → Module → Cell
    pub fn valid_parents(&self) -> &'static [ProductType] {
        match self {
            // Station is top level, no parent
            Self::Station => &[],

            // Site-level devices → Station
            Self::Gateway
            | Self::PvInverter
            | Self::Diesel
            | Self::Load
            | Self::Pcs
            | Self::Env => &[Self::Station],

            // BMS hierarchy
            Self::BatteryStack => &[Self::Station],
            Self::BatteryCluster => &[Self::BatteryStack],
            Self::BatteryPack => &[Self::BatteryCluster],
            Self::BatteryModule => &[Self::BatteryPack],
            Self::BatteryCell => &[Self::BatteryModule],
        }
    }

    /// Check if this product can have the given parent type
    pub fn can_have_parent(&self, parent: &ProductType) -> bool {
        self.valid_parents().contains(parent)
    }

    /// Check if this is a top-level product (no valid parents)
    pub fn is_top_level(&self) -> bool {
        self.valid_parents().is_empty()
    }

    /// Get product definition
    pub fn definition(&self) -> &'static ProductDef {
        match self {
            Self::Station => &STATION_DEF,
            Self::Gateway => &GATEWAY_DEF,
            Self::PvInverter => &PV_INVERTER_DEF,
            Self::Diesel => &DIESEL_DEF,
            Self::Load => &LOAD_DEF,
            Self::Pcs => &PCS_DEF,
            Self::BatteryStack => &BATTERY_STACK_DEF,
            Self::BatteryCluster => &BATTERY_CLUSTER_DEF,
            Self::BatteryPack => &BATTERY_PACK_DEF,
            Self::BatteryModule => &BATTERY_MODULE_DEF,
            Self::BatteryCell => &BATTERY_CELL_DEF,
            Self::Env => &ENV_DEF,
        }
    }
}

// ============================================================================
// Point Definition Structures
// ============================================================================

/// Point definition (compile-time constant)
#[derive(Debug, Clone, Copy)]
pub struct PointDef {
    /// Point ID (unique within product for each point type)
    pub id: u32,
    /// Point name (English)
    pub name: &'static str,
    /// Point description (Chinese)
    pub description: &'static str,
    /// Unit of measurement
    pub unit: Option<&'static str>,
}

/// Product definition (compile-time constant)
#[derive(Debug, Clone, Copy)]
pub struct ProductDef {
    /// Product type
    pub product_type: ProductType,
    /// Property points (P)
    pub properties: &'static [PointDef],
    /// Measurement points (M)
    pub measurements: &'static [PointDef],
    /// Action points (A)
    pub actions: &'static [PointDef],
}

// ============================================================================
// Station Definition
// ============================================================================

pub const STATION_DEF: ProductDef = ProductDef {
    product_type: ProductType::Station,
    properties: &[
        PointDef {
            id: 1,
            name: "Rated Capacity",
            description: "额定装机容量",
            unit: None,
        },
        PointDef {
            id: 2,
            name: "Longitude",
            description: "经度",
            unit: None,
        },
        PointDef {
            id: 3,
            name: "Latitude",
            description: "纬度",
            unit: None,
        },
        PointDef {
            id: 4,
            name: "Altitude",
            description: "海拔",
            unit: Some("m"),
        },
    ],
    measurements: &[
        PointDef {
            id: 1,
            name: "Status",
            description: "状态",
            unit: None,
        }, // 1:运行 2:告警 0:故障
        PointDef {
            id: 2,
            name: "Saving Billing",
            description: "节省的费用",
            unit: Some("$"),
        },
    ],
    actions: &[],
};

// ============================================================================
// Gateway Definition (placeholder - to be defined later)
// ============================================================================

pub const GATEWAY_DEF: ProductDef = ProductDef {
    product_type: ProductType::Gateway,
    properties: &[],
    measurements: &[],
    actions: &[],
};

// ============================================================================
// PV Inverter Definition
// ============================================================================

pub const PV_INVERTER_DEF: ProductDef = ProductDef {
    product_type: ProductType::PvInverter,
    properties: &[
        PointDef {
            id: 1,
            name: "Max Power",
            description: "最大功率",
            unit: Some("kW"),
        },
        PointDef {
            id: 2,
            name: "Max Voltage",
            description: "最大电压",
            unit: Some("V"),
        },
        PointDef {
            id: 3,
            name: "Max Current",
            description: "最大电流",
            unit: Some("A"),
        },
        PointDef {
            id: 4,
            name: "Station",
            description: "对应站点",
            unit: None,
        },
        PointDef {
            id: 5,
            name: "String Count",
            description: "串数",
            unit: None,
        },
    ],
    measurements: &[
        PointDef {
            id: 1,
            name: "PV Power",
            description: "一串光伏板的发电功率",
            unit: Some("kW"),
        }, // Array
        PointDef {
            id: 2,
            name: "PV Voltage",
            description: "一串光伏板的电压",
            unit: Some("V"),
        }, // Array
        PointDef {
            id: 3,
            name: "PV Current",
            description: "一串光伏板的电流",
            unit: Some("A"),
        }, // Array
        PointDef {
            id: 4,
            name: "Sub PVI",
            description: "子逆变器功率",
            unit: Some("kW"),
        },
        PointDef {
            id: 5,
            name: "Energy Today",
            description: "今日发电量",
            unit: Some("kWh"),
        },
        PointDef {
            id: 6,
            name: "Start Stop Status",
            description: "启停状态",
            unit: None,
        },
    ],
    actions: &[
        PointDef {
            id: 1,
            name: "Start",
            description: "启动",
            unit: None,
        },
        PointDef {
            id: 2,
            name: "Stop",
            description: "停止",
            unit: None,
        },
        PointDef {
            id: 3,
            name: "Power Set",
            description: "功率设定",
            unit: Some("kW"),
        },
        PointDef {
            id: 4,
            name: "Clear Error",
            description: "清除错误",
            unit: None,
        },
    ],
};

// ============================================================================
// Diesel Generator Definition
// ============================================================================

pub const DIESEL_DEF: ProductDef = ProductDef {
    product_type: ProductType::Diesel,
    properties: &[
        PointDef {
            id: 1,
            name: "Max Power",
            description: "最大功率",
            unit: Some("kW"),
        },
        PointDef {
            id: 2,
            name: "Max Voltage",
            description: "最大电压",
            unit: Some("V"),
        },
        PointDef {
            id: 3,
            name: "Max Current",
            description: "最大电流",
            unit: Some("A"),
        },
        PointDef {
            id: 4,
            name: "Max Fuel",
            description: "最大柴油量",
            unit: Some("L"),
        },
        PointDef {
            id: 5,
            name: "Rated Frequency",
            description: "额定频率",
            unit: Some("Hz"),
        },
    ],
    measurements: &[
        PointDef {
            id: 1,
            name: "Diesel Power",
            description: "柴发发电功率",
            unit: Some("kW"),
        },
        PointDef {
            id: 2,
            name: "Diesel Energy",
            description: "柴发发电量",
            unit: Some("kWh"),
        },
        PointDef {
            id: 3,
            name: "Diesel Voltage",
            description: "柴发电压",
            unit: Some("V"),
        },
        PointDef {
            id: 4,
            name: "Diesel Current A",
            description: "柴发A相电流",
            unit: Some("A"),
        },
        PointDef {
            id: 5,
            name: "Diesel Current B",
            description: "柴发B相电流",
            unit: Some("A"),
        },
        PointDef {
            id: 6,
            name: "Diesel Current C",
            description: "柴发C相电流",
            unit: Some("A"),
        },
        PointDef {
            id: 7,
            name: "Diesel Voltage A",
            description: "柴发A相电压",
            unit: Some("V"),
        },
        PointDef {
            id: 8,
            name: "Diesel Voltage B",
            description: "柴发B相电压",
            unit: Some("V"),
        },
        PointDef {
            id: 9,
            name: "Diesel Voltage C",
            description: "柴发C相电压",
            unit: Some("V"),
        },
        PointDef {
            id: 10,
            name: "Diesel Power A",
            description: "柴发A相功率",
            unit: Some("kW"),
        },
        PointDef {
            id: 11,
            name: "Diesel Power B",
            description: "柴发B相功率",
            unit: Some("kW"),
        },
        PointDef {
            id: 12,
            name: "Diesel Power C",
            description: "柴发C相功率",
            unit: Some("kW"),
        },
        PointDef {
            id: 13,
            name: "Diesel Oil",
            description: "柴发燃油量",
            unit: Some("%"),
        },
        PointDef {
            id: 14,
            name: "Diesel Temperature",
            description: "柴发温度",
            unit: Some("°C"),
        },
        PointDef {
            id: 15,
            name: "Start Stop Status",
            description: "启停状态",
            unit: None,
        }, // 1:运行 2:错误 0:空闲
        PointDef {
            id: 16,
            name: "Frequency",
            description: "频率",
            unit: Some("Hz"),
        },
    ],
    actions: &[
        PointDef {
            id: 1,
            name: "Start",
            description: "启动",
            unit: None,
        },
        PointDef {
            id: 2,
            name: "Stop",
            description: "停止",
            unit: None,
        },
        PointDef {
            id: 3,
            name: "Power Set",
            description: "功率设定",
            unit: Some("kW"),
        },
        PointDef {
            id: 4,
            name: "Off On Grid",
            description: "离并网切换",
            unit: None,
        },
        PointDef {
            id: 5,
            name: "Clear Error",
            description: "清除错误",
            unit: None,
        },
    ],
};

// ============================================================================
// Load Definition
// ============================================================================

pub const LOAD_DEF: ProductDef = ProductDef {
    product_type: ProductType::Load,
    properties: &[
        PointDef {
            id: 1,
            name: "Max Voltage",
            description: "最大电压",
            unit: Some("V"),
        },
        PointDef {
            id: 2,
            name: "Max Current",
            description: "最大电流",
            unit: Some("A"),
        },
        PointDef {
            id: 3,
            name: "Rated Frequency",
            description: "额定频率",
            unit: Some("Hz"),
        },
        PointDef {
            id: 4,
            name: "Max Power",
            description: "最大功率",
            unit: Some("kW"),
        },
    ],
    measurements: &[
        PointDef {
            id: 1,
            name: "Load Power",
            description: "负载有功功率",
            unit: Some("kW"),
        },
        PointDef {
            id: 2,
            name: "Energy Used",
            description: "负载用电量",
            unit: Some("kWh"),
        },
        PointDef {
            id: 3,
            name: "Voltage",
            description: "电压",
            unit: Some("V"),
        },
        PointDef {
            id: 4,
            name: "Current",
            description: "电流",
            unit: Some("A"),
        },
        PointDef {
            id: 5,
            name: "Frequency",
            description: "频率",
            unit: Some("Hz"),
        },
    ],
    actions: &[],
};

// ============================================================================
// PCS (Power Conversion System) Definition
// ============================================================================

pub const PCS_DEF: ProductDef = ProductDef {
    product_type: ProductType::Pcs,
    properties: &[
        PointDef {
            id: 1,
            name: "Max Power",
            description: "最大功率",
            unit: Some("kW"),
        },
        PointDef {
            id: 2,
            name: "Max Voltage",
            description: "最大电压",
            unit: Some("V"),
        },
        PointDef {
            id: 3,
            name: "Max Current AC",
            description: "最大电流（交流）",
            unit: Some("A"),
        },
        PointDef {
            id: 4,
            name: "Max Current DC",
            description: "最大电流（直流）",
            unit: Some("A"),
        },
        PointDef {
            id: 5,
            name: "Rated Frequency",
            description: "额定频率",
            unit: Some("Hz"),
        },
    ],
    measurements: &[
        PointDef {
            id: 1,
            name: "Total Power",
            description: "总功率",
            unit: Some("kW"),
        },
        PointDef {
            id: 2,
            name: "DC Power",
            description: "直流功率",
            unit: Some("kW"),
        },
        PointDef {
            id: 3,
            name: "Power A",
            description: "PCS A相功率",
            unit: Some("kW"),
        },
        PointDef {
            id: 4,
            name: "Power B",
            description: "PCS B相功率",
            unit: Some("kW"),
        },
        PointDef {
            id: 5,
            name: "Power C",
            description: "PCS C相功率",
            unit: Some("kW"),
        },
        PointDef {
            id: 6,
            name: "DC Voltage",
            description: "直流电压",
            unit: Some("V"),
        },
        PointDef {
            id: 7,
            name: "Voltage A",
            description: "PCS A相电压",
            unit: Some("V"),
        },
        PointDef {
            id: 8,
            name: "Voltage B",
            description: "PCS B相电压",
            unit: Some("V"),
        },
        PointDef {
            id: 9,
            name: "Voltage C",
            description: "PCS C相电压",
            unit: Some("V"),
        },
        PointDef {
            id: 10,
            name: "Current A",
            description: "PCS A相电流",
            unit: Some("A"),
        },
        PointDef {
            id: 11,
            name: "Current B",
            description: "PCS B相电流",
            unit: Some("A"),
        },
        PointDef {
            id: 12,
            name: "Current C",
            description: "PCS C相电流",
            unit: Some("A"),
        },
        PointDef {
            id: 13,
            name: "Temperature",
            description: "温度",
            unit: Some("°C"),
        },
        PointDef {
            id: 14,
            name: "Start Stop Status",
            description: "启停状态",
            unit: None,
        }, // 1:运行 2:空闲 0:停止
        PointDef {
            id: 15,
            name: "Grid Status",
            description: "并离网状态",
            unit: None,
        },
        PointDef {
            id: 16,
            name: "Direction",
            description: "输入输出方向",
            unit: None,
        }, // 1:DC→AC 0:AC→DC
        PointDef {
            id: 17,
            name: "AC Frequency",
            description: "交流频率",
            unit: Some("Hz"),
        },
    ],
    actions: &[
        PointDef {
            id: 1,
            name: "Start",
            description: "启动",
            unit: None,
        },
        PointDef {
            id: 2,
            name: "Stop",
            description: "停止",
            unit: None,
        },
        PointDef {
            id: 3,
            name: "Power Set",
            description: "功率设定",
            unit: Some("kW"),
        },
        PointDef {
            id: 4,
            name: "Off On Grid",
            description: "离并网切换",
            unit: None,
        },
        PointDef {
            id: 5,
            name: "Clear Error",
            description: "清除错误",
            unit: None,
        },
    ],
};

// ============================================================================
// Environment Monitoring Definition
// ============================================================================

pub const ENV_DEF: ProductDef = ProductDef {
    product_type: ProductType::Env,
    properties: &[],
    measurements: &[
        PointDef {
            id: 1,
            name: "Water Leakage",
            description: "水浸",
            unit: None,
        },
        PointDef {
            id: 2,
            name: "Lightning Protection",
            description: "防雷",
            unit: None,
        },
        PointDef {
            id: 3,
            name: "Temperature Humidity",
            description: "温湿度",
            unit: Some("°C/%"),
        },
        PointDef {
            id: 4,
            name: "Fire Protection",
            description: "消防",
            unit: None,
        },
        PointDef {
            id: 5,
            name: "Emergency Stop",
            description: "急停",
            unit: None,
        },
    ],
    actions: &[
        PointDef {
            id: 1,
            name: "Emergency Stop",
            description: "急停",
            unit: None,
        },
        PointDef {
            id: 2,
            name: "Fire Protection",
            description: "消防",
            unit: None,
        },
    ],
};

// ============================================================================
// BMS Hierarchy Definitions
// ============================================================================

// Battery Stack - highest level (contains clusters)
pub const BATTERY_STACK_DEF: ProductDef = ProductDef {
    product_type: ProductType::BatteryStack,
    properties: &[
        PointDef {
            id: 1,
            name: "Max Power",
            description: "最大功率",
            unit: Some("kW"),
        },
        PointDef {
            id: 2,
            name: "Max Voltage",
            description: "最大电压",
            unit: Some("V"),
        },
        PointDef {
            id: 3,
            name: "Max Current",
            description: "最大电流",
            unit: Some("A"),
        },
        PointDef {
            id: 4,
            name: "Max Capacity",
            description: "最大容量",
            unit: Some("kWh"),
        },
        PointDef {
            id: 5,
            name: "Cluster Count",
            description: "簇数量",
            unit: None,
        },
    ],
    measurements: &[
        PointDef {
            id: 1,
            name: "Total Voltage",
            description: "总电压",
            unit: Some("V"),
        },
        PointDef {
            id: 2,
            name: "Total Current",
            description: "总电流",
            unit: Some("A"),
        },
        PointDef {
            id: 3,
            name: "SOC",
            description: "电池荷电状态",
            unit: Some("%"),
        },
        PointDef {
            id: 4,
            name: "SOH",
            description: "电池健康状态",
            unit: Some("%"),
        },
        PointDef {
            id: 5,
            name: "Max Temperature",
            description: "最高温度",
            unit: Some("°C"),
        },
        PointDef {
            id: 6,
            name: "Min Temperature",
            description: "最低温度",
            unit: Some("°C"),
        },
        PointDef {
            id: 7,
            name: "Charge Power",
            description: "充电功率",
            unit: Some("kW"),
        },
        PointDef {
            id: 8,
            name: "Discharge Power",
            description: "放电功率",
            unit: Some("kW"),
        },
        PointDef {
            id: 9,
            name: "Charge Discharge Status",
            description: "充放电状态",
            unit: None,
        },
    ],
    actions: &[
        PointDef {
            id: 1,
            name: "Start",
            description: "启动",
            unit: None,
        },
        PointDef {
            id: 2,
            name: "Stop",
            description: "停止",
            unit: None,
        },
        PointDef {
            id: 3,
            name: "Clear Error",
            description: "清除错误",
            unit: None,
        },
    ],
};

// Battery Cluster (contains packs)
pub const BATTERY_CLUSTER_DEF: ProductDef = ProductDef {
    product_type: ProductType::BatteryCluster,
    properties: &[
        PointDef {
            id: 1,
            name: "Max Power",
            description: "最大功率",
            unit: Some("kW"),
        },
        PointDef {
            id: 2,
            name: "Max Voltage",
            description: "最大电压",
            unit: Some("V"),
        },
        PointDef {
            id: 3,
            name: "Max Current",
            description: "最大电流",
            unit: Some("A"),
        },
        PointDef {
            id: 4,
            name: "Max Capacity",
            description: "最大容量",
            unit: Some("kWh"),
        },
        PointDef {
            id: 5,
            name: "Pack Count",
            description: "包数量",
            unit: None,
        },
    ],
    measurements: &[
        PointDef {
            id: 1,
            name: "Total Voltage",
            description: "总电压",
            unit: Some("V"),
        },
        PointDef {
            id: 2,
            name: "Total Current",
            description: "总电流",
            unit: Some("A"),
        },
        PointDef {
            id: 3,
            name: "SOC",
            description: "电池荷电状态",
            unit: Some("%"),
        },
        PointDef {
            id: 4,
            name: "SOH",
            description: "电池健康状态",
            unit: Some("%"),
        },
        PointDef {
            id: 5,
            name: "Max Temperature",
            description: "最高温度",
            unit: Some("°C"),
        },
        PointDef {
            id: 6,
            name: "Min Temperature",
            description: "最低温度",
            unit: Some("°C"),
        },
        PointDef {
            id: 7,
            name: "Charge Discharge Status",
            description: "充放电状态",
            unit: None,
        },
    ],
    actions: &[
        PointDef {
            id: 1,
            name: "Start",
            description: "启动",
            unit: None,
        },
        PointDef {
            id: 2,
            name: "Stop",
            description: "停止",
            unit: None,
        },
        PointDef {
            id: 3,
            name: "Clear Error",
            description: "清除错误",
            unit: None,
        },
    ],
};

// Battery Pack (contains modules) - main ESS unit from feishu doc
pub const BATTERY_PACK_DEF: ProductDef = ProductDef {
    product_type: ProductType::BatteryPack,
    properties: &[
        PointDef {
            id: 1,
            name: "Max Power",
            description: "最大功率",
            unit: Some("kW"),
        },
        PointDef {
            id: 2,
            name: "Max Voltage",
            description: "最大电压",
            unit: Some("V"),
        },
        PointDef {
            id: 3,
            name: "Max Current",
            description: "最大电流",
            unit: Some("A"),
        },
        PointDef {
            id: 4,
            name: "Max Capacity",
            description: "最大容量",
            unit: Some("kWh"),
        },
        PointDef {
            id: 5,
            name: "Module Count",
            description: "模组数量",
            unit: None,
        }, // 下级子模组数量
        PointDef {
            id: 6,
            name: "Cell Count",
            description: "cell总数量",
            unit: None,
        },
        PointDef {
            id: 7,
            name: "Temperature Count",
            description: "温度点数量",
            unit: None,
        },
    ],
    measurements: &[
        PointDef {
            id: 1,
            name: "Total Voltage",
            description: "总电压",
            unit: Some("V"),
        },
        PointDef {
            id: 2,
            name: "Total Current",
            description: "总电流",
            unit: Some("A"),
        },
        PointDef {
            id: 3,
            name: "Max Battery Pack Temperature",
            description: "最高电池包温度",
            unit: Some("°C"),
        },
        PointDef {
            id: 4,
            name: "Min Battery Pack Temperature",
            description: "最低电池包温度",
            unit: Some("°C"),
        },
        PointDef {
            id: 5,
            name: "Charge Power",
            description: "充电功率",
            unit: Some("kW"),
        },
        PointDef {
            id: 6,
            name: "Discharge Power",
            description: "放电功率",
            unit: Some("kW"),
        },
        PointDef {
            id: 7,
            name: "SOC",
            description: "电池荷电状态",
            unit: Some("%"),
        },
        PointDef {
            id: 8,
            name: "SOH",
            description: "电池健康状态",
            unit: Some("%"),
        },
        PointDef {
            id: 9,
            name: "Charge Energy",
            description: "充电量",
            unit: Some("kWh"),
        },
        PointDef {
            id: 10,
            name: "Discharge Energy",
            description: "放电量",
            unit: Some("kWh"),
        },
        PointDef {
            id: 11,
            name: "Charge Discharge Status",
            description: "充/放电状态",
            unit: None,
        },
        PointDef {
            id: 12,
            name: "Max Cell Voltage",
            description: "单元最大电压",
            unit: Some("V"),
        },
        PointDef {
            id: 13,
            name: "Min Cell Voltage",
            description: "单元最小电压",
            unit: Some("V"),
        },
        PointDef {
            id: 14,
            name: "Avg Cell Voltage",
            description: "单元平均电压",
            unit: Some("V"),
        },
        PointDef {
            id: 15,
            name: "Cell Voltage Difference",
            description: "单元压差",
            unit: Some("V"),
        },
        PointDef {
            id: 16,
            name: "Avg Cell Temperature",
            description: "单元平均温度",
            unit: Some("°C"),
        },
        PointDef {
            id: 17,
            name: "Cell Voltage Array",
            description: "cell电压",
            unit: Some("V"),
        }, // Array
        PointDef {
            id: 18,
            name: "Cell Temperature Array",
            description: "cell温度",
            unit: Some("°C"),
        }, // Array
    ],
    actions: &[
        PointDef {
            id: 1,
            name: "Start",
            description: "启动",
            unit: None,
        },
        PointDef {
            id: 2,
            name: "Stop",
            description: "停止",
            unit: None,
        },
        PointDef {
            id: 3,
            name: "Clear Error",
            description: "清除错误",
            unit: None,
        },
    ],
};

// Battery Module (contains cells)
pub const BATTERY_MODULE_DEF: ProductDef = ProductDef {
    product_type: ProductType::BatteryModule,
    properties: &[
        PointDef {
            id: 1,
            name: "Max Voltage",
            description: "最大电压",
            unit: Some("V"),
        },
        PointDef {
            id: 2,
            name: "Max Current",
            description: "最大电流",
            unit: Some("A"),
        },
        PointDef {
            id: 3,
            name: "Cell Count",
            description: "cell数量",
            unit: None,
        },
    ],
    measurements: &[
        PointDef {
            id: 1,
            name: "Module Voltage",
            description: "模组电压",
            unit: Some("V"),
        },
        PointDef {
            id: 2,
            name: "Module Current",
            description: "模组电流",
            unit: Some("A"),
        },
        PointDef {
            id: 3,
            name: "Module Temperature",
            description: "模组温度",
            unit: Some("°C"),
        },
        PointDef {
            id: 4,
            name: "Max Cell Voltage",
            description: "最高电芯电压",
            unit: Some("V"),
        },
        PointDef {
            id: 5,
            name: "Min Cell Voltage",
            description: "最低电芯电压",
            unit: Some("V"),
        },
        PointDef {
            id: 6,
            name: "Avg Cell Voltage",
            description: "平均电芯电压",
            unit: Some("V"),
        },
        PointDef {
            id: 7,
            name: "Cell Voltage Array",
            description: "电芯电压数组",
            unit: Some("V"),
        }, // Array
    ],
    actions: &[],
};

// Battery Cell - smallest unit
pub const BATTERY_CELL_DEF: ProductDef = ProductDef {
    product_type: ProductType::BatteryCell,
    properties: &[
        PointDef {
            id: 1,
            name: "Nominal Voltage",
            description: "标称电压",
            unit: Some("V"),
        },
        PointDef {
            id: 2,
            name: "Nominal Capacity",
            description: "标称容量",
            unit: Some("Ah"),
        },
    ],
    measurements: &[
        PointDef {
            id: 1,
            name: "Cell Voltage",
            description: "电芯电压",
            unit: Some("V"),
        },
        PointDef {
            id: 2,
            name: "Cell Temperature",
            description: "电芯温度",
            unit: Some("°C"),
        },
        PointDef {
            id: 3,
            name: "Cell SOC",
            description: "电芯SOC",
            unit: Some("%"),
        },
        PointDef {
            id: 4,
            name: "Cell SOH",
            description: "电芯SOH",
            unit: Some("%"),
        },
        PointDef {
            id: 5,
            name: "Internal Resistance",
            description: "内阻",
            unit: Some("mΩ"),
        },
    ],
    actions: &[],
};

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_product_type_roundtrip() {
        for pt in ProductType::all() {
            let s = pt.as_str();
            let parsed = ProductType::try_parse(s);
            assert_eq!(parsed, Some(*pt), "Roundtrip failed for {:?}", pt);
        }
    }

    #[test]
    fn test_bms_hierarchy() {
        // Cell -> Module -> Pack -> Cluster -> Stack
        assert_eq!(
            ProductType::BatteryCell.bms_parent(),
            Some(ProductType::BatteryModule)
        );
        assert_eq!(
            ProductType::BatteryModule.bms_parent(),
            Some(ProductType::BatteryPack)
        );
        assert_eq!(
            ProductType::BatteryPack.bms_parent(),
            Some(ProductType::BatteryCluster)
        );
        assert_eq!(
            ProductType::BatteryCluster.bms_parent(),
            Some(ProductType::BatteryStack)
        );
        assert_eq!(ProductType::BatteryStack.bms_parent(), None);
    }

    #[test]
    fn test_bms_levels() {
        assert_eq!(ProductType::BatteryCell.bms_level(), Some(0));
        assert_eq!(ProductType::BatteryModule.bms_level(), Some(1));
        assert_eq!(ProductType::BatteryPack.bms_level(), Some(2));
        assert_eq!(ProductType::BatteryCluster.bms_level(), Some(3));
        assert_eq!(ProductType::BatteryStack.bms_level(), Some(4));

        // Non-BMS types should return None
        assert_eq!(ProductType::Station.bms_level(), None);
        assert_eq!(ProductType::Pcs.bms_level(), None);
    }

    #[test]
    fn test_can_contain() {
        // Module can contain Cell
        assert!(ProductType::BatteryModule.can_contain(&ProductType::BatteryCell));
        // Pack can contain Module
        assert!(ProductType::BatteryPack.can_contain(&ProductType::BatteryModule));
        // Cluster can contain Pack
        assert!(ProductType::BatteryCluster.can_contain(&ProductType::BatteryPack));
        // Stack can contain Cluster
        assert!(ProductType::BatteryStack.can_contain(&ProductType::BatteryCluster));

        // Stack cannot directly contain Cell (skip levels)
        assert!(!ProductType::BatteryStack.can_contain(&ProductType::BatteryCell));
        // Pack cannot contain Cluster (wrong direction)
        assert!(!ProductType::BatteryPack.can_contain(&ProductType::BatteryCluster));
    }

    #[test]
    fn test_product_definitions() {
        // Verify all products have definitions
        for pt in ProductType::all() {
            let def = pt.definition();
            assert_eq!(def.product_type, *pt);
        }
    }

    #[test]
    fn test_station_definition() {
        let def = ProductType::Station.definition();
        assert_eq!(def.properties.len(), 4);
        assert_eq!(def.measurements.len(), 2);
        assert_eq!(def.actions.len(), 0);

        // Check specific points
        assert_eq!(def.properties[0].name, "Rated Capacity");
        assert_eq!(def.measurements[0].name, "Status");
    }

    #[test]
    fn test_battery_pack_definition() {
        let def = ProductType::BatteryPack.definition();
        assert_eq!(def.properties.len(), 7);
        assert_eq!(def.measurements.len(), 18);
        assert_eq!(def.actions.len(), 3);

        // Check SOC is in measurements
        let soc = def
            .measurements
            .iter()
            .find(|p| p.name == "SOC")
            .expect("SOC measurement point should exist");
        assert_eq!(soc.unit, Some("%"));
    }

    #[test]
    fn test_valid_parents() {
        // Station is top-level (no valid parents)
        assert!(ProductType::Station.valid_parents().is_empty());
        assert!(ProductType::Station.is_top_level());

        // Site-level devices belong to Station
        for pt in &[
            ProductType::Gateway,
            ProductType::PvInverter,
            ProductType::Diesel,
            ProductType::Load,
            ProductType::Pcs,
            ProductType::Env,
        ] {
            assert_eq!(pt.valid_parents(), &[ProductType::Station]);
            assert!(pt.can_have_parent(&ProductType::Station));
            assert!(!pt.is_top_level());
        }

        // BMS hierarchy: Stack → Cluster → Pack → Module → Cell
        assert_eq!(
            ProductType::BatteryStack.valid_parents(),
            &[ProductType::Station]
        );
        assert_eq!(
            ProductType::BatteryCluster.valid_parents(),
            &[ProductType::BatteryStack]
        );
        assert_eq!(
            ProductType::BatteryPack.valid_parents(),
            &[ProductType::BatteryCluster]
        );
        assert_eq!(
            ProductType::BatteryModule.valid_parents(),
            &[ProductType::BatteryPack]
        );
        assert_eq!(
            ProductType::BatteryCell.valid_parents(),
            &[ProductType::BatteryModule]
        );

        // Cross-check: Cell cannot have Station as parent
        assert!(!ProductType::BatteryCell.can_have_parent(&ProductType::Station));
    }

    #[test]
    fn test_bms_child_count_properties() {
        // Each BMS level should have child count property
        let stack = ProductType::BatteryStack.definition();
        assert!(stack.properties.iter().any(|p| p.name == "Cluster Count"));

        let cluster = ProductType::BatteryCluster.definition();
        assert!(cluster.properties.iter().any(|p| p.name == "Pack Count"));

        let pack = ProductType::BatteryPack.definition();
        assert!(pack.properties.iter().any(|p| p.name == "Module Count"));

        let module = ProductType::BatteryModule.definition();
        assert!(module.properties.iter().any(|p| p.name == "Cell Count"));
    }
}
