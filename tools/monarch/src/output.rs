//! JSON output helpers for --json mode
//!
//! Consistent envelope format for all monarch commands:
//! - Success: `{"success": true, "data": ...}`
//! - Error: `{"success": false, "error": "..."}`

use serde::Serialize;

/// Print `{"success": true, "data": ...}` to stdout
pub fn print_success(data: impl Serialize) {
    let envelope = serde_json::json!({ "success": true, "data": data });
    match serde_json::to_string_pretty(&envelope) {
        Ok(s) => println!("{s}"),
        Err(e) => println!(r#"{{"success":false,"error":"serialize error: {e}"}}"#),
    }
}

/// Print `{"success": false, "error": "..."}` to stdout
pub fn print_error(msg: &str) {
    let envelope = serde_json::json!({ "success": false, "error": msg });
    if let Ok(s) = serde_json::to_string_pretty(&envelope) {
        println!("{s}");
    }
}

/// Print `{"success": true, "data": null}` for action-only commands
pub fn print_ok() {
    println!(r#"{{"success":true,"data":null}}"#);
}
