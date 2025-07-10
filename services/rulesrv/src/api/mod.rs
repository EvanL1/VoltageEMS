pub mod handlers;

use crate::engine::RuleExecutor;
use std::sync::Arc;

pub struct ApiServer {
    executor: Arc<RuleExecutor>,
    port: u16,
}

impl ApiServer {
    pub fn new(executor: Arc<RuleExecutor>, port: u16) -> Self {
        Self { executor, port }
    }

    pub async fn start(self) -> crate::error::Result<()> {
        // TODO: Implement API server
        Ok(())
    }
}
