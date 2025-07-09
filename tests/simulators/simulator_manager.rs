use std::collections::HashMap;
use std::process::{Child, Command};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct SimulatorManager {
    simulators: Arc<RwLock<HashMap<String, SimulatorInstance>>>,
    configs: HashMap<String, SimulatorConfig>,
}

#[derive(Debug)]
struct SimulatorInstance {
    name: String,
    protocol: String,
    process: Option<Child>,
    status: SimulatorStatus,
    config: SimulatorConfig,
}

#[derive(Debug, Clone, PartialEq)]
enum SimulatorStatus {
    Stopped,
    Starting,
    Running,
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulatorConfig {
    pub name: String,
    pub protocol: String,
    pub executable: String,
    pub args: Vec<String>,
    pub env_vars: HashMap<String, String>,
    pub startup_delay_ms: u64,
    pub health_check: HealthCheckConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    pub method: HealthCheckMethod,
    pub interval_ms: u64,
    pub timeout_ms: u64,
    pub max_retries: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum HealthCheckMethod {
    TcpConnect { host: String, port: u16 },
    HttpGet { url: String, expected_status: u16 },
    ProcessRunning,
    Custom { command: String, args: Vec<String> },
}

impl SimulatorManager {
    pub fn new() -> Self {
        Self {
            simulators: Arc::new(RwLock::new(HashMap::new())),
            configs: HashMap::new(),
        }
    }
    
    pub fn register_simulator(&mut self, config: SimulatorConfig) {
        self.configs.insert(config.name.clone(), config);
    }
    
    pub async fn start_simulator(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let config = self.configs.get(name)
            .ok_or_else(|| format!("Simulator '{}' not found", name))?
            .clone();
        
        let mut simulators = self.simulators.write().await;
        
        if let Some(instance) = simulators.get(name) {
            if instance.status == SimulatorStatus::Running {
                return Ok(());
            }
        }
        
        let mut instance = SimulatorInstance {
            name: name.to_string(),
            protocol: config.protocol.clone(),
            process: None,
            status: SimulatorStatus::Starting,
            config: config.clone(),
        };
        
        let mut cmd = Command::new(&config.executable);
        cmd.args(&config.args);
        
        for (key, value) in &config.env_vars {
            cmd.env(key, value);
        }
        
        match cmd.spawn() {
            Ok(child) => {
                instance.process = Some(child);
                
                sleep(Duration::from_millis(config.startup_delay_ms)).await;
                
                if self.check_health(&config.health_check).await {
                    instance.status = SimulatorStatus::Running;
                    println!("Simulator '{}' started successfully", name);
                } else {
                    instance.status = SimulatorStatus::Failed("Health check failed".to_string());
                    if let Some(mut process) = instance.process.take() {
                        let _ = process.kill();
                    }
                    return Err("Simulator health check failed".into());
                }
            }
            Err(e) => {
                instance.status = SimulatorStatus::Failed(e.to_string());
                return Err(Box::new(e));
            }
        }
        
        simulators.insert(name.to_string(), instance);
        Ok(())
    }
    
    pub async fn stop_simulator(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut simulators = self.simulators.write().await;
        
        if let Some(mut instance) = simulators.remove(name) {
            if let Some(mut process) = instance.process.take() {
                process.kill()?;
                process.wait()?;
            }
            instance.status = SimulatorStatus::Stopped;
            println!("Simulator '{}' stopped", name);
        }
        
        Ok(())
    }
    
    pub async fn start_all(&self) -> Result<(), Box<dyn std::error::Error>> {
        for name in self.configs.keys() {
            self.start_simulator(name).await?;
        }
        Ok(())
    }
    
    pub async fn stop_all(&self) -> Result<(), Box<dyn std::error::Error>> {
        let names: Vec<String> = {
            let simulators = self.simulators.read().await;
            simulators.keys().cloned().collect()
        };
        
        for name in names {
            self.stop_simulator(&name).await?;
        }
        Ok(())
    }
    
    pub async fn get_status(&self, name: &str) -> Option<SimulatorStatus> {
        let simulators = self.simulators.read().await;
        simulators.get(name).map(|instance| instance.status.clone())
    }
    
    pub async fn get_all_status(&self) -> HashMap<String, SimulatorStatus> {
        let simulators = self.simulators.read().await;
        simulators.iter()
            .map(|(name, instance)| (name.clone(), instance.status.clone()))
            .collect()
    }
    
    async fn check_health(&self, config: &HealthCheckConfig) -> bool {
        let mut retries = 0;
        
        loop {
            match &config.method {
                HealthCheckMethod::TcpConnect { host, port } => {
                    if let Ok(_) = tokio::time::timeout(
                        Duration::from_millis(config.timeout_ms),
                        tokio::net::TcpStream::connect(format!("{}:{}", host, port))
                    ).await {
                        return true;
                    }
                }
                
                HealthCheckMethod::HttpGet { url, expected_status } => {
                    if let Ok(response) = tokio::time::timeout(
                        Duration::from_millis(config.timeout_ms),
                        reqwest::get(url)
                    ).await {
                        if let Ok(resp) = response {
                            if resp.status().as_u16() == *expected_status {
                                return true;
                            }
                        }
                    }
                }
                
                HealthCheckMethod::ProcessRunning => {
                    return true;
                }
                
                HealthCheckMethod::Custom { command, args } => {
                    if let Ok(output) = Command::new(command)
                        .args(args)
                        .output() {
                        if output.status.success() {
                            return true;
                        }
                    }
                }
            }
            
            retries += 1;
            if retries >= config.max_retries {
                return false;
            }
            
            sleep(Duration::from_millis(config.interval_ms)).await;
        }
    }
}

pub fn create_default_configs() -> Vec<SimulatorConfig> {
    vec![
        SimulatorConfig {
            name: "modbus_tcp_simulator".to_string(),
            protocol: "modbus_tcp".to_string(),
            executable: "python3".to_string(),
            args: vec![
                "tests/simulators/modbus_tcp_simulator.py".to_string(),
                "--host".to_string(), "127.0.0.1".to_string(),
                "--port".to_string(), "5502".to_string(),
            ],
            env_vars: HashMap::new(),
            startup_delay_ms: 2000,
            health_check: HealthCheckConfig {
                method: HealthCheckMethod::TcpConnect {
                    host: "127.0.0.1".to_string(),
                    port: 5502,
                },
                interval_ms: 500,
                timeout_ms: 1000,
                max_retries: 5,
            },
        },
        
        SimulatorConfig {
            name: "modbus_rtu_simulator".to_string(),
            protocol: "modbus_rtu".to_string(),
            executable: "python3".to_string(),
            args: vec![
                "tests/simulators/modbus_rtu_simulator.py".to_string(),
                "--port".to_string(), "/tmp/modbus_rtu_test".to_string(),
                "--baudrate".to_string(), "9600".to_string(),
            ],
            env_vars: HashMap::new(),
            startup_delay_ms: 2000,
            health_check: HealthCheckConfig {
                method: HealthCheckMethod::ProcessRunning,
                interval_ms: 500,
                timeout_ms: 1000,
                max_retries: 5,
            },
        },
        
        SimulatorConfig {
            name: "can_simulator".to_string(),
            protocol: "can".to_string(),
            executable: "tests/simulators/can_simulator".to_string(),
            args: vec![
                "--interface".to_string(), "vcan0".to_string(),
                "--node-id".to_string(), "10".to_string(),
            ],
            env_vars: HashMap::new(),
            startup_delay_ms: 1000,
            health_check: HealthCheckConfig {
                method: HealthCheckMethod::Custom {
                    command: "ip".to_string(),
                    args: vec!["link".to_string(), "show".to_string(), "vcan0".to_string()],
                },
                interval_ms: 500,
                timeout_ms: 1000,
                max_retries: 5,
            },
        },
        
        SimulatorConfig {
            name: "iec104_simulator".to_string(),
            protocol: "iec104".to_string(),
            executable: "tests/simulators/iec104_simulator".to_string(),
            args: vec![
                "--host".to_string(), "127.0.0.1".to_string(),
                "--port".to_string(), "2404".to_string(),
                "--asdu".to_string(), "1".to_string(),
            ],
            env_vars: HashMap::new(),
            startup_delay_ms: 2000,
            health_check: HealthCheckConfig {
                method: HealthCheckMethod::TcpConnect {
                    host: "127.0.0.1".to_string(),
                    port: 2404,
                },
                interval_ms: 500,
                timeout_ms: 1000,
                max_retries: 5,
            },
        },
    ]
}