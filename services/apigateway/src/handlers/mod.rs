pub mod auth;
pub mod direct;
pub mod health;
pub mod proxy;

// Re-export commonly used handlers
pub use auth::{current_user, login, logout, refresh_token};
pub use direct::{batch_read, direct_read};
pub use health::{detailed_health, health_check};
pub use proxy::{
    alarmsrv_proxy, comsrv_proxy, hissrv_proxy, modsrv_proxy, netsrv_proxy, rulesrv_proxy,
};
