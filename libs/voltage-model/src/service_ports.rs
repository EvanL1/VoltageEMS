//! Service Port Constants
//!
//! Single source of truth for all VoltageEMS service default ports.
//! These are used as fallback defaults when not overridden by configuration.

/// Default port for comsrv (communication service)
pub const COMSRV_PORT: u16 = 6001;

/// Default port for modsrv (model execution service)
pub const MODSRV_PORT: u16 = 6002;

/// Default port for hissrv (history service)
pub const HISSRV_PORT: u16 = 6004;

/// Default port for apigateway
pub const APIGATEWAY_PORT: u16 = 6005;

/// Default port for alarmsrv (alarm service)
pub const ALARMSRV_PORT: u16 = 6007;

/// Default port for netsrv (network service)
pub const NETSRV_PORT: u16 = 6006;

/// Default port for voltage-apps (frontend)
pub const APPS_PORT: u16 = 8080;

/// Default Redis port
pub const REDIS_PORT: u16 = 6379;

/// Get the default port for a service by name.
///
/// Returns `None` for unknown service names.
pub fn default_port_for(service: &str) -> Option<u16> {
    match service {
        "comsrv" => Some(COMSRV_PORT),
        "modsrv" => Some(MODSRV_PORT),
        "hissrv" => Some(HISSRV_PORT),
        "apigateway" => Some(APIGATEWAY_PORT),
        "alarmsrv" => Some(ALARMSRV_PORT),
        "netsrv" => Some(NETSRV_PORT),
        _ => None,
    }
}
