use anyhow::Result;
use chrono::Local;
use redis::AsyncCommands;
use slint::Model; // bring Model trait into scope for row_count/row_data
use slint::{ModelRc, SharedString, VecModel};
use sqlx::{sqlite::SqliteRow, Row, SqlitePool};
use tokio::time::{interval, Duration};
// Use centralized keyspace + point typing from voltage-config
use voltage_config::{KeySpaceConfig, PointType};

slint::include_modules!();

fn now_hms() -> SharedString {
    Local::now().format("%H:%M:%S").to_string().into()
}

// Convert epoch-like numeric string to local time HH:MM:SS.
// Heuristic: choose the unit (s/ms/us/ns) whose seconds value is closest to 'now'.
fn fmt_ts_local(ts_raw: &str) -> SharedString {
    let s = ts_raw.trim();
    if s.is_empty() {
        return SharedString::from("");
    }
    let Ok(num) = s.parse::<i64>() else {
        return SharedString::from(ts_raw);
    };
    let now_secs = Local::now().timestamp();
    let candidates: &[(i64, i64)] = &[
        (1, 0),             // seconds
        (1_000, 1_000_000), // milliseconds (rem * 1_000_000 -> nanos)
        (1_000_000, 1_000), // microseconds
        (1_000_000_000, 1), // nanoseconds
    ];

    let mut best = None::<(i64, u32, i64)>; // (secs, nanos, diff)
    for &(factor, rem_to_nanos) in candidates {
        let secs = num / factor;
        let rem = (num % factor).abs();
        let nanos_i64 = rem.saturating_mul(rem_to_nanos);
        if nanos_i64 > (u32::MAX as i64) {
            continue;
        }
        let diff = (secs - now_secs).abs();
        match best {
            None => best = Some((secs, nanos_i64 as u32, diff)),
            Some((_, _, best_diff)) if diff < best_diff => {
                best = Some((secs, nanos_i64 as u32, diff))
            },
            _ => {},
        }
    }

    if let Some((secs, nanos, _)) = best {
        if let Some(utc_dt) = chrono::DateTime::from_timestamp(secs, nanos) {
            let dt = utc_dt.with_timezone(&Local);
            return SharedString::from(dt.format("%H:%M:%S").to_string());
        }
    }
    SharedString::from(ts_raw)
}

async fn load_channels_sqlite(db_path: &str) -> Result<Vec<ChannelRow>> {
    let pool = SqlitePool::connect(&format!("sqlite://{}?mode=ro", db_path)).await?;
    // comsrv 官方表结构：channels(channel_id, name, protocol, enabled, config)
    let mut rows: Vec<SqliteRow> = sqlx::query(
        "SELECT channel_id AS id, name, protocol, enabled, '' AS address FROM channels ORDER BY channel_id",
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();
    let mut out = Vec::with_capacity(rows.len());
    for r in rows.drain(..) {
        let enabled: i64 = r.try_get("enabled").unwrap_or(1);
        out.push(ChannelRow {
            id: r.try_get::<i64, _>("id").unwrap_or(0) as i32,
            name: r.try_get::<String, _>("name").unwrap_or_default().into(),
            protocol: r
                .try_get::<String, _>("protocol")
                .unwrap_or_default()
                .into(),
            enabled: enabled != 0,
            address: r.try_get::<String, _>("address").unwrap_or_default().into(),
        });
    }
    Ok(out)
}

async fn load_points_sqlite(db_path: &str, channel_id: i32, typ: &str) -> Result<Vec<PointRow>> {
    let pool = SqlitePool::connect(&format!("sqlite://{}?mode=ro", db_path)).await?;
    let table = match typ {
        "T" => "telemetry_points",
        "S" => "signal_points",
        "C" => "control_points",
        "A" => "adjustment_points",
        _ => "telemetry_points",
    };
    let sql = format!("SELECT point_id, signal_name AS name, COALESCE(scale,0) AS scale, COALESCE(offset,0) AS offset, COALESCE(unit,'') AS unit FROM {table} WHERE channel_id = ? ORDER BY point_id");
    let rows = sqlx::query(&sql).bind(channel_id).fetch_all(&pool).await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        out.push(PointRow {
            point_id: r.try_get::<i64, _>("point_id").unwrap_or(0) as i32,
            name: r.try_get::<String, _>("name").unwrap_or_default().into(),
            scale: r.try_get::<f64, _>("scale").unwrap_or(0.0) as f32,
            offset: r.try_get::<f64, _>("offset").unwrap_or(0.0) as f32,
            value: SharedString::from(""),
            unit: r.try_get::<String, _>("unit").unwrap_or_default().into(),
            ts: SharedString::default(),
        });
    }
    Ok(out)
}

fn typ_to_point_type(typ: &str) -> PointType {
    match typ {
        "T" => PointType::Telemetry,
        "S" => PointType::Signal,
        "C" => PointType::Control,
        "A" => PointType::Adjustment,
        _ => PointType::Telemetry,
    }
}

async fn refresh_values_redis(ui_weak: slint::Weak<AppWindow>, redis_url: String) {
    let client = match redis::Client::open(redis_url.clone()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Redis connect error (invalid URL?): {}", e);
            return;
        },
    };
    let mut interval = interval(Duration::from_millis(1000));
    // Select keyspace mode from env: KEYSPACE=test -> test keyspace, else production
    let keyspace_mode = std::env::var("KEYSPACE").unwrap_or_default();
    let ks = if keyspace_mode.eq_ignore_ascii_case("test") {
        KeySpaceConfig::test()
    } else {
        KeySpaceConfig::production()
    };
    loop {
        interval.tick().await;
        let (channel_id, typ) = if let Some(ui) = ui_weak.upgrade() {
            (
                ui.get_selected_channel_id(),
                ui.get_selected_four_remote().to_string(),
            )
        } else {
            break;
        };
        if channel_id <= 0 {
            continue;
        }
        let mut conn = match client.get_multiplexed_async_connection().await {
            Ok(c) => c,
            Err(_e) => continue,
        };
        let pt = typ_to_point_type(&typ);
        let data_key = ks.channel_key(channel_id as u16, pt);
        let ts_key = ks.channel_ts_key(channel_id as u16, pt);
        let vals: redis::RedisResult<Vec<(String, String)>> = conn.hgetall(data_key.as_ref()).await;
        let tss: redis::RedisResult<Vec<(String, String)>> = conn.hgetall(ts_key.as_ref()).await;
        let map_v: std::collections::HashMap<_, _> = vals.unwrap_or_default().into_iter().collect();
        let map_t: std::collections::HashMap<_, _> = tss.unwrap_or_default().into_iter().collect();

        eprintln!(
            "Redis read key={} ({} fields), ts_key={} ({} fields)",
            data_key.as_ref(),
            map_v.len(),
            ts_key.as_ref(),
            map_t.len()
        );
        let ui_weak2 = ui_weak.clone();
        #[allow(clippy::disallowed_methods)] // GUI event loop - unwrap safe with valid index
        slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_weak2.upgrade() {
                // 重建行：根据现有行顺序，用 Redis 值/时间覆盖
                let cur = ui.get_channel_points();
                let mut vec: Vec<PointRow> = Vec::new();
                let mut dbg_samples: Vec<String> = Vec::new();
                for i in 0..cur.row_count() {
                    let r = cur.row_data(i).unwrap();
                    let pid = r.point_id as i64;
                    let key = pid.to_string();
                    let val = map_v.get(&key).cloned().unwrap_or_default().into();
                    // ts 字段采用 "{point_id}:ts" 命名
                    let ts_field = format!("{}:ts", key);
                    let ts_raw = map_t.get(&ts_field)
                        .cloned()
                        .or_else(|| map_t.get(&key).cloned()) // 少量历史数据为纯数字键
                        .unwrap_or_default();
                    let ts = fmt_ts_local(&ts_raw);
                    if dbg_samples.len() < 3 {
                        dbg_samples.push(format!(
                            "id={} v='{}' ts='{}'",
                            key,
                            String::from(&val),
                            String::from(&ts)
                        ));
                    }
                    vec.push(PointRow {
                        point_id: r.point_id,
                        name: r.name.clone(),
                        scale: r.scale,
                        offset: r.offset,
                        value: val,
                        unit: r.unit.clone(),
                        ts,
                    });
                }
                ui.set_channel_points(ModelRc::new(VecModel::from(vec)));
                ui.set_last_update_time(now_hms());
                if !dbg_samples.is_empty() {
                    eprintln!("UI refresh sample: {}", dbg_samples.join(", "));
                }
            }
        })
        .ok();
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let ui = AppWindow::new()?;

    // 读取环境变量
    let db_env = std::env::var("VOLTAGE_DB_PATH").ok();
    let mut db_path = db_env
        .clone()
        .unwrap_or_else(|| "data/voltage.db".to_string());
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    eprintln!("Starting EMS Console UI");

    // 确认数据库文件存在，避免误连到新建空库；若默认相对路径未找到，尝试常见上级路径
    if !std::path::Path::new(&db_path).exists() {
        let cwd = std::env::current_dir().unwrap_or_default();
        let fallback1 = cwd.join("../../data/voltage.db");
        let fallback2 = cwd.join("../data/voltage.db");
        if db_env.is_none() && fallback1.exists() {
            db_path = fallback1.to_string_lossy().to_string();
            eprintln!("INFO: Using fallback DB path: {}", db_path);
        } else if db_env.is_none() && fallback2.exists() {
            db_path = fallback2.to_string_lossy().to_string();
            eprintln!("INFO: Using fallback DB path: {}", db_path);
        } else {
            eprintln!("ERROR: Database not found at {}", db_path);
            eprintln!(
                "提示: 请先运行 monarch init all && monarch sync all，或设置 VOLTAGE_DB_PATH"
            );
        }
    }

    // 加载通道
    let channels = match load_channels_sqlite(&db_path).await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("加载 channels 失败: {}", e);
            Vec::new()
        },
    };
    ui.set_channels(ModelRc::new(VecModel::from(channels.clone())));
    eprintln!(
        "loaded channels path=\"{}\" count={}",
        db_path,
        channels.len()
    );
    if let Some(first) = channels.first() {
        ui.set_selected_channel_id(first.id);
    }
    ui.set_selected_four_remote("T".into());

    // 加载点位定义
    {
        let id = ui.get_selected_channel_id();
        let typ = ui.get_selected_four_remote();
        let rows = match load_points_sqlite(&db_path, id, &typ).await {
            Ok(v) => v,
            Err(e) => {
                eprintln!("加载 points 失败: {}", e);
                Vec::new()
            },
        };
        ui.set_channel_points(ModelRc::new(VecModel::from(rows)));
        // 切换数据源后将页码重置为 1，避免停留在无效页
        ui.set_channel_current_page(1);
        let current = ui.get_channel_points();
        // Model::row_count available via trait import
        eprintln!(
            "loaded points channel_id={} type={} count={}",
            id,
            typ,
            current.row_count()
        );
    }
    ui.set_last_update_time(now_hms());

    // 绑定回调：选择通道
    {
        let uiw = ui.as_weak();
        let db_path = db_path.clone();
        ui.on_channel_selected(move |id| {
            let uiw2 = uiw.clone();
            let db = db_path.clone();
            let _ = slint::spawn_local(async move {
                if let Some(ui) = uiw2.upgrade() {
                    let typ = ui.get_selected_four_remote();
                    let rows = match load_points_sqlite(&db, id, &typ).await {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("加载 points 失败: {}", e);
                            Vec::new()
                        },
                    };
                    let cnt = rows.len();
                    slint::invoke_from_event_loop(move || {
                        if let Some(ui) = uiw2.upgrade() {
                            ui.set_selected_channel_id(id);
                            ui.set_channel_points(ModelRc::new(VecModel::from(rows)));
                            // 切换通道时重置页码，防止落在越界页
                            ui.set_channel_current_page(1);
                            eprintln!(
                                "reload points after channel_selected: channel_id={} count={}",
                                id, cnt
                            );
                        }
                    })
                    .ok();
                }
            });
        });
    }

    // 绑定回调：切换四遥
    {
        let uiw = ui.as_weak();
        let db_path = db_path.clone();
        ui.on_four_remote_selected(move |typ| {
            let uiw2 = uiw.clone();
            let db = db_path.clone();
            let _ = slint::spawn_local(async move {
                if let Some(ui) = uiw2.upgrade() {
                    let id = ui.get_selected_channel_id();
                    let rows = match load_points_sqlite(&db, id, &typ).await {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("加载 points 失败: {}", e);
                            Vec::new()
                        }
                    };
                    let cnt = rows.len();
                    slint::invoke_from_event_loop(move || {
                        if let Some(ui) = uiw2.upgrade() {
                            ui.set_selected_four_remote(typ);
                            ui.set_channel_points(ModelRc::new(VecModel::from(rows)));
                            // 切换四遥类型时重置页码
                            ui.set_channel_current_page(1);
                            eprintln!("reload points after four_remote_selected: channel_id={} type={} count={}", id, ui.get_selected_four_remote(), cnt);
                        }
                    }).ok();
                }
            });
        });
    }

    // 后台刷新 Redis 值
    {
        let uiw = ui.as_weak();
        let redis_url = redis_url.clone();
        tokio::spawn(async move { refresh_values_redis(uiw, redis_url).await });
    }

    eprintln!("🚀 Starting UI event loop");
    ui.run()?;
    Ok(())
}
