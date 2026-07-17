use crate::models::{Activity, Deal, Lead};
use rusqlite::Connection;
use std::sync::Mutex;

pub struct DbState {
    pub conn: Mutex<Connection>,
}

#[tauri::command]
pub fn get_leads(state: tauri::State<DbState>) -> Result<Vec<Lead>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    get_leads_internal(&conn)
}

pub fn get_leads_internal(conn: &Connection) -> Result<Vec<Lead>, String> {
    let mut stmt = conn
        .prepare("SELECT id, name, company, status, value, last_activity, owner, email, phone, about FROM leads")
        .map_err(|e| format!("Database query error: {}", e))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(Lead {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                company: row.get(2)?,
                status: row.get(3)?,
                value: row.get(4)?,
                last_activity: row.get(5)?,
                owner: row.get(6)?,
                email: row.get(7)?,
                phone: row.get(8)?,
                about: row.get(9)?,
            })
        })
        .map_err(|e| format!("Database query error: {}", e))?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| format!("Database query error: {}", e))?);
    }
    Ok(result)
}

#[tauri::command]
pub fn get_lead_details(id: i32, state: tauri::State<DbState>) -> Result<Lead, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    get_lead_details_internal(&conn, id)
}

pub fn get_lead_details_internal(conn: &Connection, id: i32) -> Result<Lead, String> {
    conn.query_row(
        "SELECT id, name, company, status, value, last_activity, owner, email, phone, about FROM leads WHERE id = ?1",
        [id],
        |row| {
            Ok(Lead {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                company: row.get(2)?,
                status: row.get(3)?,
                value: row.get(4)?,
                last_activity: row.get(5)?,
                owner: row.get(6)?,
                email: row.get(7)?,
                phone: row.get(8)?,
                about: row.get(9)?,
            })
        }
    ).map_err(|e| format!("Database query error: {}", e))
}

#[tauri::command]
pub fn add_lead(
    name: String,
    company: String,
    status: String,
    value: f64,
    owner: String,
    email: String,
    phone: String,
    about: Option<String>,
    state: tauri::State<DbState>,
) -> Result<i32, String> {
    let mut conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    add_lead_internal(
        &mut conn, name, company, status, value, owner, email, phone, about,
    )
}

pub fn add_lead_internal(
    conn: &mut Connection,
    name: String,
    company: String,
    status: String,
    value: f64,
    owner: String,
    email: String,
    phone: String,
    about: Option<String>,
) -> Result<i32, String> {
    let now: String = conn
        .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
            row.get(0)
        })
        .map_err(|e| format!("Database query error: {}", e))?;
    conn.execute(
        "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone, about) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![name, company, status, value, now, owner, email, phone, about],
    ).map_err(|e| format!("Database query error: {}", e))?;
    let last_id = conn.last_insert_rowid() as i32;
    Ok(last_id)
}

#[tauri::command]
pub fn update_lead_status(
    id: i32,
    status: String,
    state: tauri::State<DbState>,
) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    update_lead_status_internal(&conn, id, status)
}

pub fn update_lead_status_internal(
    conn: &Connection,
    id: i32,
    status: String,
) -> Result<(), String> {
    conn.execute(
        "UPDATE leads SET status = ?1 WHERE id = ?2",
        rusqlite::params![status, id],
    )
    .map_err(|e| format!("Database query error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_deals(state: tauri::State<DbState>) -> Result<Vec<Deal>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    get_deals_internal(&conn)
}

pub fn get_deals_internal(conn: &Connection) -> Result<Vec<Deal>, String> {
    let mut stmt = conn
        .prepare("SELECT id, lead_id, title, value, stage, created_at FROM deals")
        .map_err(|e| format!("Database query error: {}", e))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(Deal {
                id: Some(row.get(0)?),
                lead_id: row.get(1)?,
                title: row.get(2)?,
                value: row.get(3)?,
                stage: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| format!("Database query error: {}", e))?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| format!("Database query error: {}", e))?);
    }
    Ok(result)
}

#[tauri::command]
pub fn update_deal_stage(
    id: i32,
    stage: String,
    state: tauri::State<DbState>,
) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    update_deal_stage_internal(&conn, id, stage)
}

pub fn update_deal_stage_internal(conn: &Connection, id: i32, stage: String) -> Result<(), String> {
    conn.execute(
        "UPDATE deals SET stage = ?1 WHERE id = ?2",
        rusqlite::params![stage, id],
    )
    .map_err(|e| format!("Database query error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_activities(lead_id: i32, state: tauri::State<DbState>) -> Result<Vec<Activity>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    get_activities_internal(&conn, lead_id)
}

pub fn get_activities_internal(conn: &Connection, lead_id: i32) -> Result<Vec<Activity>, String> {
    let mut stmt = conn
        .prepare("SELECT id, lead_id, activity_type, content, created_at, performed_by FROM activities WHERE lead_id = ?1")
        .map_err(|e| format!("Database query error: {}", e))?;

    let rows = stmt
        .query_map([lead_id], |row| {
            Ok(Activity {
                id: Some(row.get(0)?),
                lead_id: row.get(1)?,
                activity_type: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
                performed_by: row.get(5)?,
            })
        })
        .map_err(|e| format!("Database query error: {}", e))?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| format!("Database query error: {}", e))?);
    }
    Ok(result)
}

#[tauri::command]
pub fn add_activity(
    lead_id: i32,
    activity_type: String,
    content: String,
    performed_by: String,
    state: tauri::State<DbState>,
) -> Result<(), String> {
    let mut conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    add_activity_internal(&mut conn, lead_id, activity_type, content, performed_by)
}

pub fn add_activity_internal(
    conn: &mut Connection,
    lead_id: i32,
    activity_type: String,
    content: String,
    performed_by: String,
) -> Result<(), String> {
    let now: String = conn
        .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
            row.get(0)
        })
        .map_err(|e| format!("Database query error: {}", e))?;

    let tx = conn
        .transaction()
        .map_err(|e| format!("Database transaction error: {}", e))?;
    tx.execute(
        "INSERT INTO activities (lead_id, activity_type, content, created_at, performed_by) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![lead_id, activity_type, content, &now, performed_by],
    ).map_err(|e| format!("Database query error: {}", e))?;

    tx.execute(
        "UPDATE leads SET last_activity = ?1 WHERE id = ?2",
        rusqlite::params![&now, lead_id],
    )
    .map_err(|e| format!("Database query error: {}", e))?;

    tx.commit()
        .map_err(|e| format!("Database transaction commit error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn update_lead(
    id: i32,
    name: String,
    company: String,
    status: String,
    value: f64,
    owner: String,
    email: String,
    phone: String,
    about: Option<String>,
    state: tauri::State<DbState>,
) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    conn.execute(
        "UPDATE leads SET name = ?1, company = ?2, status = ?3, value = ?4, owner = ?5, email = ?6, phone = ?7, about = ?8 WHERE id = ?9",
        rusqlite::params![name, company, status, value, owner, email, phone, about, id],
    ).map_err(|e| format!("Database query error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn delete_lead(id: i32, state: tauri::State<DbState>) -> Result<(), String> {
    let mut conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    let tx = conn
        .transaction()
        .map_err(|e| format!("Database transaction error: {}", e))?;

    tx.execute("DELETE FROM activities WHERE lead_id = ?1", [id])
        .map_err(|e| format!("Database query error: {}", e))?;

    tx.execute("DELETE FROM deals WHERE lead_id = ?1", [id])
        .map_err(|e| format!("Database query error: {}", e))?;

    tx.execute("DELETE FROM leads WHERE id = ?1", [id])
        .map_err(|e| format!("Database query error: {}", e))?;

    tx.commit()
        .map_err(|e| format!("Database transaction commit error: {}", e))?;
    Ok(())
}

use crate::scrapers::browser::BrowserManager;
use tauri::Emitter;
use tauri::Manager;

pub struct BrowserState {
    pub manager: std::sync::Arc<BrowserManager>,
    pub start_lock: tokio::sync::Mutex<()>,
}

#[derive(Clone, serde::Serialize)]
pub struct ScrapeProgressPayload {
    pub progress: f64,
    pub status: String,
    pub message: String,
}

#[derive(Clone, serde::Serialize)]
pub struct ScrapeLogPayload {
    pub message: String,
}

#[derive(Clone, serde::Serialize)]
pub struct ScrapeCompletePayload {
    pub total_scraped: usize,
    pub message: String,
}

pub async fn run_google_maps_scrape_task(
    app_handle: Option<&tauri::AppHandle>,
    conn_mutex: &std::sync::Mutex<Connection>,
    _manager: std::sync::Arc<BrowserManager>,
    scraper: crate::scrapers::google_maps::GoogleMapsScraper,
    keyword: String,
    mode: String,
    location: Option<String>,
    bbox: Option<[f64; 4]>,
    step_km: Option<f64>,
) -> Result<(), String> {
    let final_businesses = if mode == "grid" {
        let bbox_arr = match bbox {
            Some(b) => b,
            None => {
                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "scrape-log",
                        ScrapeLogPayload {
                            message: "Error: Bounding box is required for grid mode".to_string(),
                        },
                    );
                }
                return Err("Bounding box is required for grid mode".to_string());
            }
        };
        let step = step_km.unwrap_or(2.0);

        let grid_bbox = crate::scrapers::grid::BoundingBox {
            lat_min: bbox_arr[0],
            lat_max: bbox_arr[1],
            lng_min: bbox_arr[2],
            lng_max: bbox_arr[3],
        };

        let initial_cells = crate::scrapers::grid::generate_grid(grid_bbox, step);
        let grid_scraper = crate::scrapers::grid::GridScraper::new(scraper);

        let app_clone = app_handle.map(|a| a.clone());
        let scrape_res = grid_scraper
            .search_grid(
                &keyword,
                initial_cells,
                move |_cell, processed, total, msg| {
                    if let Some(app) = &app_clone {
                        let progress_percent = if total > 0 {
                            (processed as f64 / total as f64) * 100.0
                        } else {
                            0.0
                        };
                        let _ = app.emit(
                            "scrape-progress",
                            ScrapeProgressPayload {
                                progress: progress_percent,
                                status: "running".to_string(),
                                message: msg.to_string(),
                            },
                        );
                        let _ = app.emit(
                            "scrape-log",
                            ScrapeLogPayload {
                                message: msg.to_string(),
                            },
                        );
                    }
                },
            )
            .await;

        match scrape_res {
            Ok(res) => res,
            Err(e) => {
                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "scrape-log",
                        ScrapeLogPayload {
                            message: format!("Grid scraping failed: {}", e),
                        },
                    );
                }
                return Err(format!("Grid scraping failed: {}", e));
            }
        }
    } else {
        // Text mode
        let loc_str = location.as_deref();
        if let Some(app) = app_handle {
            let _ = app.emit(
                "scrape-progress",
                ScrapeProgressPayload {
                    progress: 10.0,
                    status: "running".to_string(),
                    message: format!("Searching for '{}' in '{:?}'", keyword, location),
                },
            );
            let _ = app.emit(
                "scrape-log",
                ScrapeLogPayload {
                    message: format!("Navigating to Google Maps search page for '{}'...", keyword),
                },
            );
        }

        match scraper.search(&keyword, loc_str).await {
            Ok(res) => {
                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "scrape-progress",
                        ScrapeProgressPayload {
                            progress: 80.0,
                            status: "running".to_string(),
                            message: format!(
                                "Search completed. Extracted {} businesses.",
                                res.len()
                            ),
                        },
                    );
                }
                res
            }
            Err(e) => {
                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "scrape-log",
                        ScrapeLogPayload {
                            message: format!("Search scraping failed: {}", e),
                        },
                    );
                }
                return Err(format!("Search scraping failed: {}", e));
            }
        }
    };

    // 3. Write all extracted businesses to the SQLite database
    let inserted_count = {
        let conn_guard = conn_mutex.lock();
        match conn_guard {
            Ok(conn) => {
                let mut count = 0;
                let now: String = conn
                    .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
                        row.get(0)
                    })
                    .unwrap_or_else(|_| "".to_string());

                for biz in &final_businesses {
                    let exists: i64 = conn
                        .query_row(
                            "SELECT COUNT(*) FROM leads WHERE phone = ?1",
                            [&biz.phone],
                            |row| row.get(0),
                        )
                        .unwrap_or(0);

                    if exists > 0 {
                        continue;
                    }

                    let mut about_parts = Vec::new();
                    if let Some(addr) = &biz.address {
                        about_parts.push(format!("Address: {}", addr));
                    }
                    if let Some(web) = &biz.website {
                        about_parts.push(format!("Website: {}", web));
                    }
                    if let Some(fb) = &biz.facebook {
                        about_parts.push(format!("Facebook: {}", fb));
                    }
                    if let Some(zalo) = &biz.zalo {
                        about_parts.push(format!("Zalo: {}", zalo));
                    }
                    let about_str = if about_parts.is_empty() {
                        None
                    } else {
                        Some(about_parts.join("\n"))
                    };

                    let insert_res = conn.execute(
                        "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone, about) 
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                        rusqlite::params![
                            biz.name,
                            biz.name,
                            "New",
                            0.0,
                            &now,
                            "System",
                            "",
                            biz.phone,
                            about_str
                        ]
                    );
                    if insert_res.is_ok() {
                        count += 1;
                    }
                }
                count
            }
            Err(e) => {
                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "scrape-log",
                        ScrapeLogPayload {
                            message: format!("Failed to lock database: {}", e),
                        },
                    );
                }
                return Err(format!("Failed to lock database: {}", e));
            }
        }
    };

    // 4. Emit complete event
    let msg = format!(
        "Scrape complete! {} new leads added out of {} unique scraped businesses.",
        inserted_count,
        final_businesses.len()
    );
    if let Some(app) = app_handle {
        let _ = app.emit(
            "scrape-progress",
            ScrapeProgressPayload {
                progress: 100.0,
                status: "completed".to_string(),
                message: msg.clone(),
            },
        );
        let _ = app.emit(
            "scrape-log",
            ScrapeLogPayload {
                message: msg.clone(),
            },
        );
        let _ = app.emit(
            "scrape-complete",
            ScrapeCompletePayload {
                total_scraped: inserted_count,
                message: msg,
            },
        );
    }

    Ok(())
}

#[tauri::command]
pub fn start_google_maps_scrape(
    app_handle: tauri::AppHandle,
    keyword: String,
    mode: String,
    location: Option<String>,
    bbox: Option<[f64; 4]>,
    step_km: Option<f64>,
) -> Result<(), String> {
    let app_handle_clone = app_handle.clone();

    tokio::spawn(async move {
        let db_state = app_handle_clone.state::<DbState>();
        let browser_state = app_handle_clone.state::<BrowserState>();

        let manager = browser_state.manager.clone();

        let _ = app_handle_clone.emit(
            "scrape-log",
            ScrapeLogPayload {
                message: "Starting browser...".to_string(),
            },
        );

        // 1. Start browser if not running
        {
            let _guard = browser_state.start_lock.lock().await;
            if manager.process_id().await.is_none() {
                if let Err(e) = manager.start(true).await {
                    let _ = app_handle_clone.emit(
                        "scrape-log",
                        ScrapeLogPayload {
                            message: format!("Failed to start browser: {}", e),
                        },
                    );
                    return;
                }
            }
        }

        // 2. Acquire session
        let session = match manager.acquire().await {
            Ok(s) => s,
            Err(e) => {
                let _ = app_handle_clone.emit(
                    "scrape-log",
                    ScrapeLogPayload {
                        message: format!("Failed to acquire browser session: {}", e),
                    },
                );
                return;
            }
        };

        let scraper = crate::scrapers::google_maps::GoogleMapsScraper::new(session);
        let _ = run_google_maps_scrape_task(
            Some(&app_handle_clone),
            &db_state.conn,
            manager,
            scraper,
            keyword,
            mode,
            location,
            bbox,
            step_km,
        )
        .await;
    });

    Ok(())
}

#[derive(Clone, serde::Serialize)]
pub struct ZaloQRCodePayload {
    pub image_base64: String,
    pub token: String,
    pub code: String,
}

#[derive(Clone, serde::Serialize)]
pub struct ZaloQRScannedPayload {
    pub avatar: String,
    pub display_name: String,
}

#[derive(Clone, serde::Serialize)]
pub struct ZaloQRSuccessPayload {
    pub imei: String,
}

#[tauri::command]
pub fn start_zalo_qr_login(
    app_handle: tauri::AppHandle,
    _state: tauri::State<DbState>,
) -> Result<(), String> {
    let app_handle_clone = app_handle.clone();

    tokio::spawn(async move {
        let callback_app_handle = app_handle_clone.clone();

        let callback =
            move |event: zca_rs::apis::auth::LoginQREvent| -> zca_rs::apis::auth::QRCallbackResult {
                let is_got_login_info =
                    matches!(event, zca_rs::apis::auth::LoginQREvent::GotLoginInfo { .. });
                let is_declined = matches!(event, zca_rs::apis::auth::LoginQREvent::QRCodeDeclined);

                let db_state;
                let mut conn_guard = None;
                let mut lock_err = None;

                if is_got_login_info {
                    db_state = callback_app_handle.state::<DbState>();
                    match db_state.conn.lock() {
                        Ok(guard) => {
                            conn_guard = Some(guard);
                        }
                        Err(e) => {
                            lock_err =
                                Some(format!("Failed to lock database for Zalo session: {}", e));
                        }
                    }
                }

                let handle_res = if let Some(err) = lock_err {
                    Err(err)
                } else {
                    let conn_ref = conn_guard.as_deref();
                    handle_zalo_qr_event(event, conn_ref)
                };

                match handle_res {
                    Ok((event_name, payload)) => {
                        let _ = callback_app_handle.emit(&event_name, payload);
                    }
                    Err(e) => {
                        eprintln!("Error handling Zalo QR event: {}", e);
                    }
                }

                if is_declined {
                    zca_rs::apis::auth::QRCallbackResult::Cancel
                } else {
                    zca_rs::apis::auth::QRCallbackResult::Continue
                }
            };

        let options = zca_rs::Options::default();
        if let Err(e) = zca_rs::Zalo::login_qr(options, callback).await {
            eprintln!("Zalo QR login failed or cancelled: {:?}", e);
        }
    });

    Ok(())
}

pub fn handle_zalo_qr_event(
    event: zca_rs::apis::auth::LoginQREvent,
    conn: Option<&Connection>,
) -> Result<(String, serde_json::Value), String> {
    match event {
        zca_rs::apis::auth::LoginQREvent::QRCodeGenerated {
            code,
            image_base64,
            token,
        } => {
            let payload = serde_json::to_value(ZaloQRCodePayload {
                image_base64,
                token,
                code,
            })
            .map_err(|e| e.to_string())?;
            Ok(("zalo-qr-code".to_string(), payload))
        }
        zca_rs::apis::auth::LoginQREvent::QRCodeScanned {
            avatar,
            display_name,
        } => {
            let payload = serde_json::to_value(ZaloQRScannedPayload {
                avatar,
                display_name,
            })
            .map_err(|e| e.to_string())?;
            Ok(("zalo-qr-scanned".to_string(), payload))
        }
        zca_rs::apis::auth::LoginQREvent::QRCodeExpired => {
            Ok(("zalo-qr-expired".to_string(), serde_json::Value::Null))
        }
        zca_rs::apis::auth::LoginQREvent::QRCodeDeclined => {
            Ok(("zalo-qr-declined".to_string(), serde_json::Value::Null))
        }
        zca_rs::apis::auth::LoginQREvent::GotLoginInfo {
            cookie,
            imei,
            user_agent,
        } => {
            let cookies_json = serde_json::to_string(&cookie).map_err(|e| e.to_string())?;
            if let Some(c) = conn {
                crate::db::save_zalo_session(c, &imei, &user_agent, &cookies_json, "vi")
                    .map_err(|e| e.to_string())?;
            }

            let payload =
                serde_json::to_value(ZaloQRSuccessPayload { imei }).map_err(|e| e.to_string())?;
            Ok(("zalo-qr-success".to_string(), payload))
        }
    }
}

#[tauri::command]
pub fn get_zalo_sessions(
    state: tauri::State<DbState>,
) -> Result<Vec<crate::models::ZaloSession>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    get_zalo_sessions_internal(&conn)
}

pub fn get_zalo_sessions_internal(
    conn: &Connection,
) -> Result<Vec<crate::models::ZaloSession>, String> {
    crate::db::get_all_zalo_sessions(conn).map_err(|e| format!("Database error: {}", e))
}

#[tauri::command]
pub fn delete_zalo_session(imei: String, state: tauri::State<DbState>) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    delete_zalo_session_internal(&conn, &imei)
}

pub fn delete_zalo_session_internal(conn: &Connection, imei: &str) -> Result<(), String> {
    crate::db::delete_zalo_session(conn, imei).map_err(|e| format!("Database error: {}", e))
}

#[derive(Clone, serde::Serialize)]
pub struct OutreachProgressPayload {
    pub progress: f64,
    pub status: String,
    pub message: String,
}

#[derive(Clone, serde::Serialize)]
pub struct OutreachLogPayload {
    pub message: String,
}

#[derive(Clone, serde::Serialize)]
pub struct OutreachCompletePayload {
    pub total_sent: usize,
    pub message: String,
}

#[tauri::command]
pub fn get_outreach_templates(
    state: tauri::State<DbState>,
) -> Result<Vec<crate::models::OutreachTemplate>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::get_outreach_templates(&conn).map_err(|e| format!("Database error: {}", e))
}

#[tauri::command]
pub fn create_outreach_template(
    name: String,
    content: String,
    state: tauri::State<DbState>,
) -> Result<i32, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::create_outreach_template(&conn, &name, &content)
        .map_err(|e| format!("Database error: {}", e))
}

#[tauri::command]
pub fn update_outreach_template(
    id: i32,
    name: String,
    content: String,
    state: tauri::State<DbState>,
) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::update_outreach_template(&conn, id, &name, &content)
        .map_err(|e| format!("Database error: {}", e))
}

#[tauri::command]
pub fn delete_outreach_template(id: i32, state: tauri::State<DbState>) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::delete_outreach_template(&conn, id).map_err(|e| format!("Database error: {}", e))
}

#[tauri::command]
pub fn send_zalo_outreach(
    app_handle: tauri::AppHandle,
    imei: String,
    lead_ids: Vec<i32>,
    template_id: i32,
) -> Result<(), String> {
    let app_handle_clone = app_handle.clone();
    tokio::spawn(async move {
        let db_state = app_handle_clone.state::<DbState>();
        let _ = send_zalo_outreach_internal(
            Some(&app_handle_clone),
            &db_state.conn,
            imei,
            lead_ids,
            template_id,
        )
        .await;
    });
    Ok(())
}

pub async fn send_zalo_outreach_internal(
    app_handle: Option<&tauri::AppHandle>,
    conn_mutex: &std::sync::Mutex<Connection>,
    imei: String,
    lead_ids: Vec<i32>,
    template_id: i32,
) -> Result<(), String> {
    // Mock flow helper
    if imei == "mock-imei" || imei.starts_with("mock-") {
        let template_content = {
            let conn_guard = conn_mutex.lock().unwrap();
            conn_guard
                .query_row(
                    "SELECT content FROM outreach_templates WHERE id = ?1",
                    [template_id],
                    |row| row.get::<_, String>(0),
                )
                .unwrap_or_default()
        };

        let mut total_sent = 0;
        for (idx, lead_id) in lead_ids.iter().enumerate() {
            let (lead_name, phone) = {
                let conn_guard = conn_mutex.lock().unwrap();
                let res = conn_guard.query_row(
                    "SELECT name, phone FROM leads WHERE id = ?1",
                    [lead_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                );
                match res {
                    Ok(val) => val,
                    Err(e) => {
                        let msg = format!("Failed to load lead {}: {}", lead_id, e);
                        if let Some(app) = app_handle {
                            let _ = app.emit("outreach-log", OutreachLogPayload { message: msg });
                        }
                        continue;
                    }
                }
            };

            if phone.trim().is_empty() {
                let msg = format!("Lead {} is missing phone number", lead_id);
                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "outreach-log",
                        OutreachLogPayload {
                            message: msg.clone(),
                        },
                    );
                    let progress_val = ((idx + 1) as f64 / lead_ids.len() as f64) * 100.0;
                    let _ = app.emit(
                        "outreach-progress",
                        OutreachProgressPayload {
                            progress: progress_val,
                            status: "running".to_string(),
                            message: msg,
                        },
                    );
                }
                continue;
            }

            if phone == "not-found" {
                let msg = format!("Phone number {} not found on Zalo (Mock)", phone);
                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "outreach-log",
                        OutreachLogPayload {
                            message: msg.clone(),
                        },
                    );
                    let progress_val = ((idx + 1) as f64 / lead_ids.len() as f64) * 100.0;
                    let _ = app.emit(
                        "outreach-progress",
                        OutreachProgressPayload {
                            progress: progress_val,
                            status: "running".to_string(),
                            message: msg,
                        },
                    );
                }
                continue;
            }

            let formatted_content = template_content.replace("{name}", &lead_name);
            total_sent += 1;
            let log_msg = format!("Message sent to {} ({}) (Mock)", lead_name, phone);
            if let Some(app) = app_handle {
                let _ = app.emit("outreach-log", OutreachLogPayload { message: log_msg });
            }

            let now: String = {
                let conn_guard = conn_mutex.lock().unwrap();
                conn_guard
                    .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
                        row.get(0)
                    })
                    .unwrap_or_else(|_| "".to_string())
            };

            {
                let conn_guard = conn_mutex.lock().unwrap();
                let act_res = conn_guard.execute(
                    "INSERT INTO activities (lead_id, activity_type, content, created_at, performed_by) VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![
                        lead_id,
                        "Outreach",
                        format!("Zalo message sent: {}", formatted_content),
                        &now,
                        "System (Zalo Outreach)"
                    ]
                );
                if let Err(e) = act_res {
                    if let Some(app) = app_handle {
                        let _ = app.emit(
                            "outreach-log",
                            OutreachLogPayload {
                                message: format!("Failed to record activity log: {}", e),
                            },
                        );
                    }
                }
                let _ = conn_guard.execute(
                    "UPDATE leads SET last_activity = ?1 WHERE id = ?2",
                    rusqlite::params![&now, lead_id],
                );
            }

            if let Some(app) = app_handle {
                let progress_val = ((idx + 1) as f64 / lead_ids.len() as f64) * 100.0;
                let _ = app.emit(
                    "outreach-progress",
                    OutreachProgressPayload {
                        progress: progress_val,
                        status: "running".to_string(),
                        message: format!("Processed lead {}/{} (Mock)", idx + 1, lead_ids.len()),
                    },
                );
            }

            if idx + 1 < lead_ids.len() {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }

        let final_msg = format!(
            "Outreach complete! Sent {} messages out of {} leads. (Mock)",
            total_sent,
            lead_ids.len()
        );
        if let Some(app) = app_handle {
            let _ = app.emit(
                "outreach-progress",
                OutreachProgressPayload {
                    progress: 100.0,
                    status: "completed".to_string(),
                    message: final_msg.clone(),
                },
            );
            let _ = app.emit(
                "outreach-complete",
                OutreachCompletePayload {
                    total_sent,
                    message: final_msg,
                },
            );
        }

        return Ok(());
    }

    // Real Zalo flow
    let session_res = {
        let conn_guard = conn_mutex.lock().unwrap();
        crate::db::get_zalo_session(&conn_guard, &imei)
    };
    let session = match session_res {
        Ok(Some(s)) => s,
        Ok(None) => {
            let msg = format!("Session not found for IMEI: {}", imei);
            if let Some(app) = app_handle {
                let _ = app.emit(
                    "outreach-log",
                    OutreachLogPayload {
                        message: msg.clone(),
                    },
                );
                let _ = app.emit(
                    "outreach-progress",
                    OutreachProgressPayload {
                        progress: 0.0,
                        status: "failed".to_string(),
                        message: msg,
                    },
                );
            }
            return Err("Session not found".to_string());
        }
        Err(e) => {
            let msg = format!("Database error loading session: {}", e);
            if let Some(app) = app_handle {
                let _ = app.emit(
                    "outreach-log",
                    OutreachLogPayload {
                        message: msg.clone(),
                    },
                );
                let _ = app.emit(
                    "outreach-progress",
                    OutreachProgressPayload {
                        progress: 0.0,
                        status: "failed".to_string(),
                        message: msg,
                    },
                );
            }
            return Err(e.to_string());
        }
    };

    let cookies: Vec<zca_rs::context::ZaloCookie> =
        match serde_json::from_str(&session.cookies_json) {
            Ok(c) => c,
            Err(e) => {
                let msg = format!("Failed to parse session cookies: {}", e);
                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "outreach-log",
                        OutreachLogPayload {
                            message: msg.clone(),
                        },
                    );
                    let _ = app.emit(
                        "outreach-progress",
                        OutreachProgressPayload {
                            progress: 0.0,
                            status: "failed".to_string(),
                            message: msg,
                        },
                    );
                }
                return Err(e.to_string());
            }
        };

    let template_content = {
        let conn_guard = conn_mutex.lock().unwrap();
        let res = conn_guard.query_row(
            "SELECT content FROM outreach_templates WHERE id = ?1",
            [template_id],
            |row| row.get::<_, String>(0),
        );
        match res {
            Ok(c) => c,
            Err(e) => {
                let msg = format!("Failed to load outreach template: {}", e);
                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "outreach-log",
                        OutreachLogPayload {
                            message: msg.clone(),
                        },
                    );
                    let _ = app.emit(
                        "outreach-progress",
                        OutreachProgressPayload {
                            progress: 0.0,
                            status: "failed".to_string(),
                            message: msg,
                        },
                    );
                }
                return Err(e.to_string());
            }
        }
    };

    let options = zca_rs::Options::default();
    let zalo = match zca_rs::Zalo::login(
        session.imei.clone(),
        session.user_agent.clone(),
        cookies,
        options,
    )
    .await
    {
        Ok(z) => z,
        Err(e) => {
            let msg = format!("Zalo login failed: {:?}", e);
            if let Some(app) = app_handle {
                let _ = app.emit(
                    "outreach-log",
                    OutreachLogPayload {
                        message: msg.clone(),
                    },
                );
                let _ = app.emit(
                    "outreach-progress",
                    OutreachProgressPayload {
                        progress: 0.0,
                        status: "failed".to_string(),
                        message: msg,
                    },
                );
            }
            return Err(format!("{:?}", e));
        }
    };

    let mut total_sent = 0;
    for (idx, lead_id) in lead_ids.iter().enumerate() {
        let (lead_name, phone) = {
            let conn_guard = conn_mutex.lock().unwrap();
            let res = conn_guard.query_row(
                "SELECT name, phone FROM leads WHERE id = ?1",
                [lead_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            );
            match res {
                Ok(val) => val,
                Err(e) => {
                    let msg = format!("Failed to load lead {}: {}", lead_id, e);
                    if let Some(app) = app_handle {
                        let _ = app.emit("outreach-log", OutreachLogPayload { message: msg });
                    }
                    continue;
                }
            }
        };

        if phone.trim().is_empty() {
            let msg = format!("Lead {} ({}) is missing phone number", lead_name, lead_id);
            if let Some(app) = app_handle {
                let _ = app.emit(
                    "outreach-log",
                    OutreachLogPayload {
                        message: msg.clone(),
                    },
                );
                let progress_val = ((idx + 1) as f64 / lead_ids.len() as f64) * 100.0;
                let _ = app.emit(
                    "outreach-progress",
                    OutreachProgressPayload {
                        progress: progress_val,
                        status: "running".to_string(),
                        message: msg,
                    },
                );
            }
            continue;
        }

        if let Some(app) = app_handle {
            let _ = app.emit(
                "outreach-log",
                OutreachLogPayload {
                    message: format!("Looking up Zalo UID for {} ({})", lead_name, phone),
                },
            );
        }

        let user_opt = match zalo.find_user(&phone, None).await {
            Ok(opt) => opt,
            Err(e) => {
                let msg = format!(
                    "Failed to look up phone {} for {}: {:?}",
                    phone, lead_name, e
                );
                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "outreach-log",
                        OutreachLogPayload {
                            message: msg.clone(),
                        },
                    );
                    let progress_val = ((idx + 1) as f64 / lead_ids.len() as f64) * 100.0;
                    let _ = app.emit(
                        "outreach-progress",
                        OutreachProgressPayload {
                            progress: progress_val,
                            status: "running".to_string(),
                            message: msg,
                        },
                    );
                }
                continue;
            }
        };

        if let Some(user) = user_opt {
            let formatted_content = template_content.replace("{name}", &lead_name);

            let msg_content = zca_rs::models::MessageContent {
                msg: formatted_content.clone(),
                styles: None,
                urgency: None,
                quote: None,
                mentions: None,
                attachments: None,
                ttl: None,
            };

            match zalo
                .send_message(msg_content, &user.uid, zca_rs::models::ThreadType::User)
                .await
            {
                Ok(_) => {
                    total_sent += 1;
                    let log_msg = format!("Message sent to {} ({})", lead_name, phone);
                    if let Some(app) = app_handle {
                        let _ = app.emit("outreach-log", OutreachLogPayload { message: log_msg });
                    }

                    let now: String = {
                        let conn_guard = conn_mutex.lock().unwrap();
                        conn_guard
                            .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
                                row.get(0)
                            })
                            .unwrap_or_else(|_| "".to_string())
                    };

                    {
                        let conn_guard = conn_mutex.lock().unwrap();
                        let act_res = conn_guard.execute(
                            "INSERT INTO activities (lead_id, activity_type, content, created_at, performed_by) VALUES (?1, ?2, ?3, ?4, ?5)",
                            rusqlite::params![
                                lead_id,
                                "Outreach",
                                format!("Zalo message sent: {}", formatted_content),
                                &now,
                                "System (Zalo Outreach)"
                            ]
                        );
                        if let Err(e) = act_res {
                            if let Some(app) = app_handle {
                                let _ = app.emit(
                                    "outreach-log",
                                    OutreachLogPayload {
                                        message: format!("Failed to record activity log: {}", e),
                                    },
                                );
                            }
                        }
                        let _ = conn_guard.execute(
                            "UPDATE leads SET last_activity = ?1 WHERE id = ?2",
                            rusqlite::params![&now, lead_id],
                        );
                    }
                }
                Err(e) => {
                    let msg = format!(
                        "Failed to send Zalo message to {} ({}): {:?}",
                        lead_name, phone, e
                    );
                    if let Some(app) = app_handle {
                        let _ = app.emit(
                            "outreach-log",
                            OutreachLogPayload {
                                message: msg.clone(),
                            },
                        );
                    }
                }
            }
        } else {
            let msg = format!("Phone number {} for {} not found on Zalo", phone, lead_name);
            if let Some(app) = app_handle {
                let _ = app.emit(
                    "outreach-log",
                    OutreachLogPayload {
                        message: msg.clone(),
                    },
                );
            }
        }

        if let Some(app) = app_handle {
            let progress_val = ((idx + 1) as f64 / lead_ids.len() as f64) * 100.0;
            let _ = app.emit(
                "outreach-progress",
                OutreachProgressPayload {
                    progress: progress_val,
                    status: "running".to_string(),
                    message: format!("Processed lead {}/{}", idx + 1, lead_ids.len()),
                },
            );
        }

        if idx + 1 < lead_ids.len() {
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        }
    }

    let final_msg = format!(
        "Outreach complete! Sent {} messages out of {} leads.",
        total_sent,
        lead_ids.len()
    );
    if let Some(app) = app_handle {
        let _ = app.emit(
            "outreach-progress",
            OutreachProgressPayload {
                progress: 100.0,
                status: "completed".to_string(),
                message: final_msg.clone(),
            },
        );
        let _ = app.emit(
            "outreach-complete",
            OutreachCompletePayload {
                total_sent,
                message: final_msg,
            },
        );
    }

    Ok(())
}
