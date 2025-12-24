//! Consistency tests for RTDB implementations
//!
//! This module ensures that MemoryRtdb and RedisRtdb behave consistently.
//! Redis tests are ignored by default and require a running Redis instance.
//!
//! Run all tests (including Redis): `cargo test --package voltage-rtdb --test consistency_tests -- --ignored`

// Allow unwrap() in tests for cleaner test code
#![allow(clippy::disallowed_methods)]

use bytes::Bytes;
use voltage_rtdb::{MemoryRtdb, Rtdb};

const REDIS_URL: &str = "redis://127.0.0.1:6379";

/// Helper to create a unique test key prefix
fn test_key(suffix: &str) -> String {
    format!("test:consistency:{}:{}", uuid::Uuid::new_v4(), suffix)
}

// ============================================================================
// Basic Key-Value Operations
// ============================================================================

#[tokio::test]
async fn test_memory_hash_set_get() {
    let rtdb = MemoryRtdb::new();
    rtdb.hash_set("test:hash", "field1", Bytes::from("value1"))
        .await
        .unwrap();

    let result = rtdb.hash_get("test:hash", "field1").await.unwrap();
    assert_eq!(result, Some(Bytes::from("value1")));
}

#[tokio::test]
#[ignore = "requires Redis"]
async fn test_redis_hash_set_get() {
    use voltage_rtdb::RedisRtdb;

    let rtdb = RedisRtdb::new(REDIS_URL).await.unwrap();
    let key = test_key("hash");

    rtdb.hash_set(&key, "field1", Bytes::from("value1"))
        .await
        .unwrap();

    let result = rtdb.hash_get(&key, "field1").await.unwrap();
    assert_eq!(result, Some(Bytes::from("value1")));

    // Cleanup
    rtdb.del(&key).await.ok();
}

// ============================================================================
// Increment Operations (Critical for consistency)
// ============================================================================

#[tokio::test]
async fn test_memory_hincrby_new_field() {
    let rtdb = MemoryRtdb::new();

    // hincrby on non-existent field should initialize to 0 and then increment
    let result = rtdb.hincrby("test:incr", "counter", 5).await.unwrap();
    assert_eq!(result, 5);

    // Second increment
    let result = rtdb.hincrby("test:incr", "counter", 3).await.unwrap();
    assert_eq!(result, 8);
}

#[tokio::test]
#[ignore = "requires Redis"]
async fn test_redis_hincrby_new_field() {
    use voltage_rtdb::RedisRtdb;

    let rtdb = RedisRtdb::new(REDIS_URL).await.unwrap();
    let key = test_key("incr");

    // hincrby on non-existent field should initialize to 0 and then increment
    let result = rtdb.hincrby(&key, "counter", 5).await.unwrap();
    assert_eq!(result, 5);

    // Second increment
    let result = rtdb.hincrby(&key, "counter", 3).await.unwrap();
    assert_eq!(result, 8);

    // Cleanup
    rtdb.del(&key).await.ok();
}

#[tokio::test]
async fn test_memory_incrbyfloat_new_key() {
    let rtdb = MemoryRtdb::new();

    // incrbyfloat on non-existent key should initialize to 0.0 and then increment
    let result = rtdb.incrbyfloat("test:float", 2.5).await.unwrap();
    assert!((result - 2.5).abs() < 0.001);

    // Second increment
    let result = rtdb.incrbyfloat("test:float", 1.5).await.unwrap();
    assert!((result - 4.0).abs() < 0.001);
}

#[tokio::test]
#[ignore = "requires Redis"]
async fn test_redis_incrbyfloat_new_key() {
    use voltage_rtdb::RedisRtdb;

    let rtdb = RedisRtdb::new(REDIS_URL).await.unwrap();
    let key = test_key("float");

    // incrbyfloat on non-existent key should initialize to 0.0 and then increment
    let result = rtdb.incrbyfloat(&key, 2.5).await.unwrap();
    assert!((result - 2.5).abs() < 0.001);

    // Second increment
    let result = rtdb.incrbyfloat(&key, 1.5).await.unwrap();
    assert!((result - 4.0).abs() < 0.001);

    // Cleanup
    rtdb.del(&key).await.ok();
}

// ============================================================================
// Hash Multi-Get Operations
// ============================================================================

#[tokio::test]
async fn test_memory_hash_mget() {
    let rtdb = MemoryRtdb::new();

    rtdb.hash_set("test:mget", "f1", Bytes::from("v1"))
        .await
        .unwrap();
    rtdb.hash_set("test:mget", "f2", Bytes::from("v2"))
        .await
        .unwrap();

    let results = rtdb
        .hash_mget("test:mget", &["f1", "f2", "f3"])
        .await
        .unwrap();

    assert_eq!(results.len(), 3);
    assert_eq!(results[0], Some(Bytes::from("v1")));
    assert_eq!(results[1], Some(Bytes::from("v2")));
    assert_eq!(results[2], None); // f3 doesn't exist
}

#[tokio::test]
#[ignore = "requires Redis"]
async fn test_redis_hash_mget() {
    use voltage_rtdb::RedisRtdb;

    let rtdb = RedisRtdb::new(REDIS_URL).await.unwrap();
    let key = test_key("mget");

    rtdb.hash_set(&key, "f1", Bytes::from("v1")).await.unwrap();
    rtdb.hash_set(&key, "f2", Bytes::from("v2")).await.unwrap();

    let results = rtdb.hash_mget(&key, &["f1", "f2", "f3"]).await.unwrap();

    assert_eq!(results.len(), 3);
    assert_eq!(results[0], Some(Bytes::from("v1")));
    assert_eq!(results[1], Some(Bytes::from("v2")));
    assert_eq!(results[2], None); // f3 doesn't exist

    // Cleanup
    rtdb.del(&key).await.ok();
}

// ============================================================================
// Delete Operations
// ============================================================================

#[tokio::test]
async fn test_memory_hash_del() {
    let rtdb = MemoryRtdb::new();

    rtdb.hash_set("test:del", "f1", Bytes::from("v1"))
        .await
        .unwrap();
    rtdb.hash_set("test:del", "f2", Bytes::from("v2"))
        .await
        .unwrap();

    // Delete one field
    let deleted = rtdb.hash_del("test:del", "f1").await.unwrap();
    assert!(deleted);

    // Verify f1 is gone, f2 remains
    assert_eq!(rtdb.hash_get("test:del", "f1").await.unwrap(), None);
    assert_eq!(
        rtdb.hash_get("test:del", "f2").await.unwrap(),
        Some(Bytes::from("v2"))
    );
}

#[tokio::test]
#[ignore = "requires Redis"]
async fn test_redis_hash_del() {
    use voltage_rtdb::RedisRtdb;

    let rtdb = RedisRtdb::new(REDIS_URL).await.unwrap();
    let key = test_key("del");

    rtdb.hash_set(&key, "f1", Bytes::from("v1")).await.unwrap();
    rtdb.hash_set(&key, "f2", Bytes::from("v2")).await.unwrap();

    // Delete one field
    let deleted = rtdb.hash_del(&key, "f1").await.unwrap();
    assert!(deleted);

    // Verify f1 is gone, f2 remains
    assert_eq!(rtdb.hash_get(&key, "f1").await.unwrap(), None);
    assert_eq!(
        rtdb.hash_get(&key, "f2").await.unwrap(),
        Some(Bytes::from("v2"))
    );

    // Cleanup
    rtdb.del(&key).await.ok();
}

// ============================================================================
// Exists and Type Operations
// ============================================================================

#[tokio::test]
async fn test_memory_exists() {
    let rtdb = MemoryRtdb::new();

    assert!(!rtdb.exists("nonexistent").await.unwrap());

    rtdb.set("exists_key", Bytes::from("value")).await.unwrap();
    assert!(rtdb.exists("exists_key").await.unwrap());
}

#[tokio::test]
#[ignore = "requires Redis"]
async fn test_redis_exists() {
    use voltage_rtdb::RedisRtdb;

    let rtdb = RedisRtdb::new(REDIS_URL).await.unwrap();
    let key = test_key("exists");

    assert!(!rtdb.exists(&key).await.unwrap());

    rtdb.set(&key, Bytes::from("value")).await.unwrap();
    assert!(rtdb.exists(&key).await.unwrap());

    // Cleanup
    rtdb.del(&key).await.ok();
}
