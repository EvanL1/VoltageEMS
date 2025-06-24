/// Example: Tracing vs Log comparison in VoltageEMS context
/// 
/// This example demonstrates the differences between `tracing` and `log` crates
/// and explains why VoltageEMS uses both in different contexts.

use std::time::Instant;

/// Demonstrate tracing capabilities
fn demo_tracing_features() {
    println!("=== Tracing Features Demo ===\n");
    
    // 1. Structured logging with fields
    tracing::info!(
        service = "comsrv",
        channel_id = 1001,
        protocol = "modbus",
        slave_id = 1,
        "Channel connection established"
    );
    
    // 2. Spans for operation tracking
    let _span = tracing::info_span!("modbus_read_operation", 
        channel = 1001, 
        address = 100, 
        quantity = 10
    ).entered();
    
    tracing::debug!("Starting register read");
    
    // Simulate some work
    std::thread::sleep(std::time::Duration::from_millis(50));
    
    tracing::info!(value = 12345, quality = "good", "Read completed successfully");
    
    // 3. Error context with spans
    let error_span = tracing::error_span!("error_context", operation = "modbus_write");
    let _guard = error_span.enter();
    
    tracing::error!(
        error_code = "timeout", 
        duration_ms = 1000, 
        "Operation failed"
    );
}

/// Demonstrate log crate usage
fn demo_log_features() {
    println!("\n=== Log Features Demo ===\n");
    
    // Simple text-based logging
    log::info!("Service started: comsrv");
    log::debug!("Processing channel configuration");
    log::warn!("Connection retry attempt");
    log::error!("Failed to connect to device");
}

/// Why VoltageEMS uses different logging approaches
fn explain_usage_patterns() {
    println!("\n=== VoltageEMS Logging Strategy ===\n");
    
    println!("1. **Tracing** used for:");
    println!("   - Service lifecycle events (startup, shutdown)");
    println!("   - Complex operations with context (Redis operations)");
    println!("   - Performance monitoring (IEC60870 operations)");
    println!("   - Hierarchical operation tracking");
    
    println!("\n2. **Log** used for:");
    println!("   - Protocol-level debugging (ModBus, IEC60870)");
    println!("   - Channel-specific events");
    println!("   - Simple status messages");
    println!("   - Target-based filtering (modbus::channel::001)");
    
    println!("\n3. **Current problems:**");
    println!("   - Mixed usage creates confusion");
    println!("   - Tracing initialization conflicts");
    println!("   - Different output formats");
    println!("   - Maintenance overhead");
}

/// Demonstrate the problem: mixed initialization
fn show_initialization_problem() {
    println!("\n=== Initialization Problem ===\n");
    
    println!("Current situation:");
    println!("- alarmsrv: tracing_subscriber::fmt::init()");
    println!("- comsrv: init_logger() + tracing macros");
    println!("- modsrv: env_logger::init()");
    println!("- netsrv: env_logger::init()");
    
    println!("\nProblems:");
    println!("- Multiple global logger initializations");
    println!("- Different output formats");
    println!("- Cannot easily unify filtering");
    println!("- Tracing spans not properly displayed");
}

/// Recommended solution: unified approach
fn recommend_solution() {
    println!("\n=== Recommended Solution ===\n");
    
    println!("**Option 1: Pure env_logger (Simplest)**");
    println!("- Remove tracing completely");
    println!("- Use log::* macros everywhere");
    println!("- Protocol-specific targets: modbus::channel::001");
    println!("- RUST_LOG for filtering");
    
    println!("\n**Option 2: Pure tracing (Most powerful)**");
    println!("- Replace all log::* with tracing::*");
    println!("- Use spans for operation context");
    println!("- Structured logging with fields");
    println!("- tracing_subscriber with env filter");
    
    println!("\n**Option 3: Hybrid (Current + ProtocolLogger)**");
    println!("- Keep tracing for service-level events");
    println!("- Use ProtocolLogger (log-based) for protocol details");
    println!("- Clear separation of concerns");
}

fn main() {
    // Initialize both for demonstration
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .format_timestamp_millis()
        .init();
    
    demo_tracing_features();
    demo_log_features();
    explain_usage_patterns();
    show_initialization_problem();
    recommend_solution();
    
    println!("\n=== Analysis Results ===");
    println!("Based on VoltageEMS usage patterns:");
    println!("1. Tracing is mostly used for simple info/debug/error logging");
    println!("2. The advanced features (spans, fields) are rarely utilized");
    println!("3. Most usage could be replaced with log::* macros");
    println!("4. The ProtocolLogger approach is more appropriate");
    
    println!("\n**Recommendation: Migrate to unified env_logger approach**");
} 