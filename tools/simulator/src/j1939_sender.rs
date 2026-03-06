//! J1939 diesel generator frame sender for vcan simulation.

#[cfg(target_os = "linux")]
use crate::state_machine::{DeviceState, StateMachineStore};
#[cfg(target_os = "linux")]
use anyhow::Result;
#[cfg(target_os = "linux")]
use socketcan::{CanFrame, CanSocket, EmbeddedFrame, ExtendedId, Socket};
#[cfg(target_os = "linux")]
use std::sync::Arc;
#[cfg(target_os = "linux")]
use std::time::Duration;
#[cfg(target_os = "linux")]
use tracing::info;
#[cfg(target_os = "linux")]
use voltage_j1939::{build_can_id, J1939Id};

#[cfg(target_os = "linux")]
fn build_eec1_frame(source_address: u8, state: &DeviceState) -> CanFrame {
    let raw_speed: u16 = match state {
        DeviceState::Running => 12000, // 1500 RPM / 0.125
        _ => 0,
    };
    let mut data = [0xFF_u8; 8];
    let [lo, hi] = raw_speed.to_le_bytes();
    data[3] = lo;
    data[4] = hi;

    let can_id = build_can_id(&J1939Id {
        priority: 3,
        pgn: 61444,
        source_address,
        destination_address: 0xFF,
    });
    let id = ExtendedId::new(can_id).unwrap();
    CanFrame::new(id, &data).unwrap()
}

#[cfg(target_os = "linux")]
fn build_et1_frame(source_address: u8, state: &DeviceState) -> CanFrame {
    let raw_temp: u8 = match state {
        DeviceState::Running => 125, // 85°C + 40
        DeviceState::Standby => 65,  // 25°C + 40
        DeviceState::Fault => 160,   // 120°C + 40
        _ => 65,
    };
    let mut data = [0xFF_u8; 8];
    data[0] = raw_temp;

    let can_id = build_can_id(&J1939Id {
        priority: 6,
        pgn: 65262,
        source_address,
        destination_address: 0xFF,
    });
    let id = ExtendedId::new(can_id).unwrap();
    CanFrame::new(id, &data).unwrap()
}

#[cfg(target_os = "linux")]
pub async fn run_j1939_sender(
    interface: &str,
    interval_ms: u64,
    source_address: u8,
    sm_store: Arc<StateMachineStore>,
    unit_id: u8,
) -> Result<()> {
    let socket = CanSocket::open(interface)?;
    let interval = Duration::from_millis(interval_ms);

    info!(interface, unit_id, source_address, "J1939 sender started");

    loop {
        let state = sm_store
            .get(&unit_id)
            .map(|sm| sm.current_state())
            .unwrap_or(DeviceState::Standby);

        let eec1 = build_eec1_frame(source_address, &state);
        let et1 = build_et1_frame(source_address, &state);

        socket.write_frame(&eec1)?;
        socket.write_frame(&et1)?;

        info!(unit_id, ?state, "sent EEC1 + ET1 frames");

        tokio::time::sleep(interval).await;
    }
}
