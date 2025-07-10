use crate::config::Config;
use crate::engine::RuleExecutor;
use crate::error::Result;
use std::sync::Arc;

pub struct Subscriber {
    config: Config,
}

impl Subscriber {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub async fn start(self, executor: Arc<RuleExecutor>) -> Result<()> {
        // TODO: Implement Redis subscription to modsrv:model:output:*
        Ok(())
    }
}
