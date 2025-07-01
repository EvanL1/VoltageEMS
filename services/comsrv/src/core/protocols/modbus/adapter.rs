use async_trait::async_trait;
use serde_json::Value;
use tokio_modbus::client::tcp;
use tokio_modbus::prelude::*;

use crate::core::config::modbus::ModbusCfg;
use crate::core::config::point::Point;
use crate::core::protocols::common::combase::adapter::Adapter;

/// An adapter for the Modbus protocol.
pub struct ModbusAdapter {
    ctx: Option<Box<dyn Reader + Send>>,
    config: ModbusCfg,
}

impl ModbusAdapter {
    pub fn new(config: ModbusCfg) -> Self {
        Self { ctx: None, config }
    }
}

#[async_trait]
impl Adapter for ModbusAdapter {
    fn id(&self) -> &'static str {
        "modbus"
    }

    async fn init(&mut self) -> anyhow::Result<()> {
        let socket_addr = format!("{}:{}", self.config.host, self.config.port).parse().unwrap();
        let mut ctx = tcp::connect(socket_addr).await?;
        ctx.set_slave(self.config.unit_id.into());
        self.ctx = Some(ctx);
        Ok(())
    }

    async fn read(&self, point: &Point) -> anyhow::Result<Value> {
        if let Some(ctx) = &self.ctx {
            let addr = point.address.parse()?;
            let regs = ctx.read_holding_registers(addr, 2).await?;
            let raw = ((regs[0] as u32) << 16) | regs[1] as u32;
            let f = f32::from_bits(raw).mul_add(point.scale, 0.0);
            Ok(Value::from(f))
        } else {
            anyhow::bail!("Modbus adapter not initialized")
        }
    }

    async fn write(&self, _point: &Point, _v: Value) -> anyhow::Result<()> {
        anyhow::bail!("Modbus write not implemented")
    }
}