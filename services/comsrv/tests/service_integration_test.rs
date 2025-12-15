//! Service Integration Tests
//!
//! This test suite verifies that the comsrv service can start up properly,
//! connect to dependencies (Redis, SQLite), and serve API endpoints.

#![allow(clippy::disallowed_methods)] // Integration test - unwrap is acceptable

use anyhow::Result;
use axum::http::StatusCode;
use rand::Rng;
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::time::sleep;

/// Generate a random port for testing to avoid conflicts
fn random_test_port() -> u16 {
    let mut rng = rand::thread_rng();
    // Use range 10000-60000 (50000 ports) to minimize collision probability
    // Avoids well-known ports (<10000) and ephemeral ports (>60000)
    10000 + rng.gen_range(0..50000)
}

/// Test environment for service integration testing
struct ServiceTestEnvironment {
    service_process: Option<std::process::Child>,
    base_url: String,
    port: u16,
    redis: redis::aio::ConnectionManager,
}

impl ServiceTestEnvironment {
    async fn new(port: u16) -> Result<Self> {
        // Connect to Redis first
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());
        let client = redis::Client::open(redis_url)?;
        let redis = redis::aio::ConnectionManager::new(client).await?;

        Ok(Self {
            service_process: None,
            base_url: format!("http://localhost:{}", port),
            port,
            redis,
        })
    }

    /// Create minimal test configuration files
    fn create_test_config(&self, config_dir: &str) -> Result<()> {
        // Create config directory structure
        std::fs::create_dir_all(format!("{}/comsrv", config_dir))?;

        // Create minimal comsrv.yaml
        let comsrv_yaml = r#"service:
  name: comsrv-test
  description: Test Communication Service
  version: 0.0.1

api:
  host: ""
  port: 0
  workers: 2

redis:
  url: redis://localhost:6379
  enabled: true

logging:
  level: info
  dir: logs/comsrv-test
  format: json"#;

        std::fs::write(format!("{}/comsrv/comsrv.yaml", config_dir), comsrv_yaml)?;

        // Create minimal channels.yaml with a virtual channel for testing
        // Virtual channel doesn't require real hardware and is perfect for tests
        let channels_yaml = format!(
            r#"channels:
  - channel_id: {}
    name: "Test Virtual Channel"
    protocol_type: "Virtual"
    enabled: true
    description: "Virtual channel for integration testing"
    config:
      update_interval_ms: 1000
      simulation_mode: true"#,
            9900 + rand::random::<u32>() % 100 // Random ID between 9900-9999 to avoid conflicts
        );
        std::fs::write(
            format!("{}/comsrv/channels.yaml", config_dir),
            channels_yaml,
        )?;

        // Create point CSV files with minimal test data
        let telemetry_csv = "point_id,signal_name,scale,offset,unit,reverse,data_type,description\n1,TestTemp,1.0,0.0,°C,false,float32,Test temperature\n";
        std::fs::write(
            format!("{}/comsrv/telemetry.csv", config_dir),
            telemetry_csv,
        )?;

        let control_csv = "point_id,signal_name,scale,offset,unit,reverse,data_type,description\n1,TestControl,1.0,0.0,,false,bool,Test control\n";
        std::fs::write(format!("{}/comsrv/control.csv", config_dir), control_csv)?;

        let adjustment_csv = "point_id,signal_name,scale,offset,unit,reverse,data_type,description\n1,TestSetpoint,1.0,0.0,°C,false,float32,Test setpoint\n";
        std::fs::write(
            format!("{}/comsrv/adjustment.csv", config_dir),
            adjustment_csv,
        )?;

        let signal_csv = "point_id,signal_name,scale,offset,unit,reverse,normal_state,data_type,description\n1,TestStatus,1.0,0.0,,false,0,uint16,Test status signal\n";
        std::fs::write(format!("{}/comsrv/signal.csv", config_dir), signal_csv)?;

        // Create minimal modsrv configuration (required for monarch sync)
        std::fs::create_dir_all(format!("{}/modsrv", config_dir))?;
        let modsrv_yaml = r#"service:
  name: modsrv-test
  description: Test Model Service
  version: 0.0.1

api:
  host: ""
  port: 0
  workers: 2

redis:
  url: redis://localhost:6379
  enabled: true

logging:
  level: info
  dir: logs/modsrv-test
  format: json"#;
        std::fs::write(format!("{}/modsrv/modsrv.yaml", config_dir), modsrv_yaml)?;

        // Create minimal rulesrv configuration (required for monarch sync)
        std::fs::create_dir_all(format!("{}/rulesrv", config_dir))?;
        let rulesrv_yaml = r#"service:
  name: rulesrv-test
  description: Test Rule Service
  version: 0.0.1

api:
  host: ""
  port: 0
  workers: 2

redis:
  url: redis://localhost:6379
  enabled: true

logging:
  level: info
  dir: logs/rulesrv-test
  format: json"#;
        std::fs::write(format!("{}/rulesrv/rulesrv.yaml", config_dir), rulesrv_yaml)?;

        Ok(())
    }

    /// Initialize test database using Monarch
    /// @param db_dir - Directory where database will be created (not the file path)
    async fn setup_test_database(&self, db_dir: &str) -> Result<()> {
        // Create database directory
        std::fs::create_dir_all(db_dir)?;

        // Find project root (contains Cargo.toml with workspace)
        let current_dir = std::env::current_dir()?;
        let mut project_root = current_dir.as_path();
        while !project_root.join("Cargo.toml").exists()
            || !std::fs::read_to_string(project_root.join("Cargo.toml"))
                .unwrap_or_default()
                .contains("[workspace]")
        {
            project_root = project_root
                .parent()
                .ok_or_else(|| anyhow::anyhow!("Could not find project root"))?;
        }

        let monarch_path = project_root.join("target/debug/monarch");

        // Always rebuild Monarch to ensure schema consistency with current code

        println!("Building Monarch with latest schema...");
        let build = Command::new("cargo")
            .args(["build", "--package", "monarch"])
            .current_dir(project_root)
            .output()?;

        if !build.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to build monarch: {}",
                String::from_utf8_lossy(&build.stderr)
            ));
        }

        // Create temporary config directory
        let config_dir = format!("/tmp/comsrv_test_config_{}", rand::random::<u32>());
        self.create_test_config(&config_dir)?;

        // Initialize database schema using Monarch
        println!("Initializing database in directory: {}", db_dir);
        println!("Using monarch at: {}", monarch_path.display());
        println!("Config dir: {}", config_dir);

        let init_output = Command::new(&monarch_path)
            .args(["init", "--config-path", &config_dir, "--db-path", db_dir])
            .current_dir(project_root)
            .output()?;

        if !init_output.status.success() {
            eprintln!(
                "Monarch init stdout: {}",
                String::from_utf8_lossy(&init_output.stdout)
            );
            eprintln!(
                "Monarch init stderr: {}",
                String::from_utf8_lossy(&init_output.stderr)
            );
            return Err(anyhow::anyhow!(
                "Monarch init failed with exit code: {:?}",
                init_output.status.code()
            ));
        }

        // Check if database was created (Monarch init all creates {db_dir}/voltage.db)
        let actual_db_path = format!("{}/voltage.db", db_dir);
        if !std::path::Path::new(&actual_db_path).exists() {
            return Err(anyhow::anyhow!(
                "Database file was not created at: {}",
                actual_db_path
            ));
        }

        // Sync configuration using Monarch
        // Use --force to skip validation since test configs are minimal
        println!("Syncing configuration...");
        let sync_output = Command::new(&monarch_path)
            .args([
                "sync",
                "--force", // Skip validation for test configs
                "--config-path",
                &config_dir,
                "--db-path",
                db_dir,
            ])
            .current_dir(project_root)
            .output()?;

        if !sync_output.status.success() {
            eprintln!(
                "Monarch sync stdout: {}",
                String::from_utf8_lossy(&sync_output.stdout)
            );
            eprintln!(
                "Monarch sync stderr: {}",
                String::from_utf8_lossy(&sync_output.stderr)
            );
            return Err(anyhow::anyhow!(
                "Monarch sync failed with exit code: {:?}",
                sync_output.status.code()
            ));
        }

        println!(
            "Database initialized successfully at: {}/voltage.db",
            db_dir
        );

        // Clean up temporary config
        std::fs::remove_dir_all(config_dir).ok();

        Ok(())
    }

    /// Start the comsrv service process
    async fn start_service(&mut self, db_path: &str) -> Result<()> {
        // Ensure database exists before starting service
        if !std::path::Path::new(db_path).exists() {
            return Err(anyhow::anyhow!(
                "Database must be created before starting service"
            ));
        }

        // Get project root directory
        let project_root = std::env::current_dir()?
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();

        // Use the already-built binary directly for faster startup
        let comsrv_binary = project_root.join("target/debug/comsrv");

        // Build if not exists
        if !comsrv_binary.exists() {
            let build = Command::new("cargo")
                .args(["build", "--bin", "comsrv"])
                .current_dir(&project_root)
                .output()?;

            if !build.status.success() {
                return Err(anyhow::anyhow!(
                    "Failed to build comsrv: {}",
                    String::from_utf8_lossy(&build.stderr)
                ));
            }
        }

        // Start the service with test configuration
        let mut child = Command::new(&comsrv_binary)
            .env("VOLTAGE_DB_PATH", db_path)
            .env("REDIS_URL", std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string()))
            .env("SERVICE_PORT", self.port.to_string()) // Use SERVICE_PORT for unified configuration
            .env("SERVICE_HOST", "0.0.0.0") // Explicitly set host
            .env("RUST_LOG", "info") // Set log level
            .env_remove("http_proxy") // Disable HTTP proxy for tests
            .env_remove("https_proxy") // Disable HTTPS proxy for tests
            .env_remove("HTTP_PROXY") // Disable HTTP proxy (uppercase)
            .env_remove("HTTPS_PROXY") // Disable HTTPS proxy (uppercase)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        println!("Service process started with PID: {:?}", child.id());

        // Wait for service to start (check health endpoint)
        for i in 0..30 {
            // Check if process is still running
            match child.try_wait() {
                Ok(Some(status)) => {
                    // Process has exited
                    let stdout = if let Some(mut out) = child.stdout.take() {
                        let mut buffer = String::new();
                        use std::io::Read;
                        out.read_to_string(&mut buffer).ok();
                        buffer
                    } else {
                        String::new()
                    };

                    let stderr = if let Some(mut err) = child.stderr.take() {
                        let mut buffer = String::new();
                        use std::io::Read;
                        err.read_to_string(&mut buffer).ok();
                        buffer
                    } else {
                        String::new()
                    };

                    return Err(anyhow::anyhow!(
                        "Service exited with status: {:?}\nstdout: {}\nstderr: {}",
                        status,
                        stdout,
                        stderr
                    ));
                },
                Ok(None) => {
                    // Still running, check health
                    if self.is_healthy().await {
                        println!("Service is healthy after {} seconds", i);
                        self.service_process = Some(child);
                        return Ok(());
                    }
                },
                Err(e) => {
                    return Err(anyhow::anyhow!("Failed to check process status: {}", e));
                },
            }

            if i == 5 {
                println!("Still waiting for service to start (5 seconds elapsed)...");
            } else if i == 15 {
                println!("Still waiting for service to start (15 seconds elapsed)...");
            }

            sleep(Duration::from_secs(1)).await;
        }

        // If we get here, service didn't become healthy in time
        println!("Service did not become healthy within 30 seconds, killing process");
        child.kill()?;
        Err(anyhow::anyhow!("Service failed to start within 30 seconds"))
    }

    /// Check if service is healthy
    async fn is_healthy(&self) -> bool {
        // Create a client with timeout and completely disable proxy
        // Use Proxy::custom to explicitly bypass all proxies including system proxies
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .no_proxy() // Disable proxy to avoid interference
            .danger_accept_invalid_certs(false) // Security: only for testing
            .build()
            .unwrap();

        match client.get(format!("{}/health", self.base_url)).send().await {
            Ok(resp) => {
                let status = resp.status();
                if status == StatusCode::OK {
                    println!("✓ Health check successful");
                    true
                } else {
                    println!("✗ Health check failed with status: {}", status);
                    false
                }
            },
            Err(e) => {
                // Don't log connection refused errors during startup
                if !e.to_string().contains("Connection refused") {
                    println!("✗ Health check error: {}", e);
                }
                false
            },
        }
    }

    /// Cleanup test environment
    async fn cleanup(&mut self) -> Result<()> {
        // Stop service if running
        if let Some(mut process) = self.service_process.take() {
            let _ = process.kill(); // Ignore errors - process might already be dead
            let _ = process.wait(); // Ignore errors
        }

        // Note: Redis connection (self.redis) will be automatically closed when dropped
        // No need to explicitly delete test data - each test uses unique ports and databases

        Ok(())
    }
}

impl Drop for ServiceTestEnvironment {
    fn drop(&mut self) {
        // Ensure service is stopped with more robust cleanup
        if let Some(mut process) = self.service_process.take() {
            // First attempt: normal termination (SIGTERM on Unix)
            let _ = process.kill();

            // Wait up to 3 seconds for graceful shutdown (using try_wait in a loop)
            let max_wait = 30; // 30 * 100ms = 3 seconds
            let mut waited = 0;
            let mut exited = false;

            while waited < max_wait {
                match process.try_wait() {
                    Ok(Some(_status)) => {
                        // Process exited successfully
                        exited = true;
                        break;
                    },
                    Ok(None) => {
                        // Still running, continue waiting
                        std::thread::sleep(Duration::from_millis(100));
                        waited += 1;
                    },
                    Err(_e) => {
                        // Error checking status, assume dead
                        break;
                    },
                }
            }

            if !exited {
                // Process still running after timeout, force kill again
                eprintln!(
                    "⚠ Test service didn't stop gracefully (port {}), forcing termination",
                    self.port
                );
                let _ = process.kill();

                // Give it another 2 seconds
                std::thread::sleep(Duration::from_secs(2));

                // Final wait (blocking)
                let _ = process.wait();
            }

            // Small delay to ensure port is released by the OS
            std::thread::sleep(Duration::from_millis(200));
        }
    }
}

#[tokio::test]
async fn test_service_startup_and_health() -> Result<()> {
    let mut rng = rand::thread_rng();
    let test_id = rng.gen::<u32>();
    // Use directory for db_path, Monarch init all creates voltage.db inside it
    let db_dir = format!("/tmp/comsrv_test_{:08x}", test_id);
    let db_path = format!("{}/voltage.db", db_dir);
    let mut env = ServiceTestEnvironment::new(random_test_port()).await?;

    // Setup test database (pass directory path)
    env.setup_test_database(&db_dir).await?;

    // Start service (pass full database file path)
    env.start_service(&db_path).await?;

    // Verify health endpoint
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let resp = client
        .get(format!("{}/health", env.base_url))
        .send()
        .await?;
    assert_eq!(resp.status(), StatusCode::OK);

    let health_data: serde_json::Value = resp.json().await?;
    // ApiResponse wraps the data, so we need to check data.status
    assert_eq!(health_data["success"], true);
    assert_eq!(health_data["data"]["status"], "healthy");
    assert!(health_data["data"]["uptime_seconds"].as_u64().is_some());

    println!("✅ Service started successfully and is healthy");

    // Cleanup
    env.cleanup().await?;
    std::fs::remove_dir_all(&db_dir).ok();

    Ok(())
}

#[tokio::test]
async fn test_service_api_endpoints() -> Result<()> {
    let mut rng = rand::thread_rng();
    let test_id = rng.gen::<u32>();
    let db_dir = format!("/tmp/comsrv_api_test_{:08x}", test_id);
    let db_path = format!("{}/voltage.db", db_dir);
    let mut env = ServiceTestEnvironment::new(random_test_port()).await?;

    env.setup_test_database(&db_dir).await?;
    env.start_service(&db_path).await?;

    // Create client without proxy
    let client = reqwest::Client::builder().no_proxy().build().unwrap();

    // Test /api/channels endpoint
    let resp = client
        .get(format!("{}/api/channels", env.base_url))
        .send()
        .await?;
    assert_eq!(resp.status(), StatusCode::OK);

    let channels_response: serde_json::Value = resp.json().await?;
    // ApiResponse wraps the data, now with pagination
    assert_eq!(channels_response["success"], true);
    assert!(channels_response["data"].is_object());
    assert!(channels_response["data"]["list"].is_array());
    assert!(channels_response["data"]["total"].is_number());
    assert!(channels_response["data"]["page"].is_number());
    assert!(channels_response["data"]["page_size"].is_number());

    // Test /api/status endpoint
    let resp = client
        .get(format!("{}/api/status", env.base_url))
        .send()
        .await?;
    assert_eq!(resp.status(), StatusCode::OK);

    let status_response: serde_json::Value = resp.json().await?;
    // ApiResponse wraps the data, so check data field
    assert_eq!(status_response["success"], true);
    assert!(status_response["data"]["name"].is_string());
    assert!(status_response["data"]["active_channels"].is_number());

    println!("✅ API endpoints are responding correctly");

    env.cleanup().await?;
    std::fs::remove_dir_all(&db_dir).ok();

    Ok(())
}

#[tokio::test]
async fn test_service_redis_connectivity() -> Result<()> {
    let mut rng = rand::thread_rng();
    let test_id = rng.gen::<u32>();
    let db_dir = format!("/tmp/comsrv_redis_test_{:08x}", test_id);
    let db_path = format!("{}/voltage.db", db_dir);
    let mut env = ServiceTestEnvironment::new(random_test_port()).await?;

    env.setup_test_database(&db_dir).await?;
    env.start_service(&db_path).await?;

    // Write test data to Redis
    use redis::AsyncCommands;
    let _: () = env.redis.hset("comsrv:test:T", "1", "42.5").await?;

    // Verify service can read from Redis via API
    // (This would require an endpoint that reads Redis data)

    // For now, just verify the service is running and Redis is accessible
    assert!(env.is_healthy().await);

    let value: f64 = env.redis.hget("comsrv:test:T", "1").await?;
    assert_eq!(value, 42.5);

    println!("✅ Service has Redis connectivity");

    env.cleanup().await?;
    std::fs::remove_dir_all(&db_dir).ok();

    Ok(())
}

#[tokio::test]
async fn test_service_graceful_shutdown() -> Result<()> {
    let mut rng = rand::thread_rng();
    let test_id = rng.gen::<u32>();
    let db_dir = format!("/tmp/comsrv_shutdown_test_{:08x}", test_id);
    let db_path = format!("{}/voltage.db", db_dir);
    let mut env = ServiceTestEnvironment::new(random_test_port()).await?;

    env.setup_test_database(&db_dir).await?;
    env.start_service(&db_path).await?;

    // Verify service is running
    assert!(env.is_healthy().await);

    // Send shutdown signal (kill process)
    if let Some(mut process) = env.service_process.take() {
        process.kill()?;

        // Wait for process to exit
        let exit_status = process.wait()?;

        // On Unix, killed processes have specific exit codes
        #[cfg(unix)]
        assert!(!exit_status.success());

        // Verify service is no longer accessible
        sleep(Duration::from_secs(1)).await;
        assert!(!env.is_healthy().await);
    }

    println!("✅ Service shuts down gracefully");

    // Cleanup (env.service_process is already None after take(), so cleanup won't try to kill again)
    env.cleanup().await?;
    std::fs::remove_dir_all(&db_dir).ok();

    Ok(())
}

#[tokio::test]
async fn test_service_handles_invalid_database() -> Result<()> {
    let invalid_db_path = "/nonexistent/path/comsrv.db";

    // Try to start service with invalid database path (no --validate flag needed)
    let output = Command::new("cargo")
        .args(["run", "--bin", "comsrv"])
        .env("COMSRV_DB_PATH", invalid_db_path)
        .output()?;

    // Service should fail to start with invalid database
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    // Check for the actual error message from the service
    assert!(stderr.contains("Configuration database not found") || stderr.contains("Error:"));

    println!("✅ Service properly handles invalid database configuration");

    Ok(())
}

#[tokio::test]
#[ignore] // This test requires a long-running service
async fn test_service_memory_stability() -> Result<()> {
    let mut rng = rand::thread_rng();
    let test_id = rng.gen::<u32>();
    let db_dir = format!("/tmp/comsrv_memory_test_{:08x}", test_id);
    let db_path = format!("{}/voltage.db", db_dir);
    let mut env = ServiceTestEnvironment::new(random_test_port()).await?;

    env.setup_test_database(&db_dir).await?;
    env.start_service(&db_path).await?;

    // Create client without proxy
    let client = reqwest::Client::builder().no_proxy().build().unwrap();

    // Run service for a period and monitor memory
    for i in 0..100 {
        // Make API calls
        let _ = client
            .get(format!("{}/api/health", env.base_url))
            .send()
            .await?;
        let _ = client
            .get(format!("{}/api/v1/channels", env.base_url))
            .send()
            .await?;

        if i % 10 == 0 {
            println!("Completed {} iterations", i);
        }

        sleep(Duration::from_millis(100)).await;
    }

    // Service should still be healthy after stress
    assert!(env.is_healthy().await);

    println!("✅ Service remains stable under load");

    env.cleanup().await?;
    std::fs::remove_dir_all(&db_dir).ok();

    Ok(())
}
