use crate::models::{Activity, Category, Deal, EmailTemplate, Lead, SmtpServer, ZaloSessionStatus};
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
        .prepare("SELECT id, name, company, status, value, last_activity, owner, email, phone, about, address, has_zalo, zalo_reached, zalo_uid, zalo_status, relationship_status, last_lookup_attempt FROM leads")
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
                address: row.get(10)?,
                has_zalo: row.get(11)?,
                zalo_reached: row.get(12)?,
                zalo_uid: row.get(13)?,
                zalo_status: row.get(14)?,
                relationship_status: row.get(15)?,
                last_lookup_attempt: row.get(16)?,
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
        "SELECT id, name, company, status, value, last_activity, owner, email, phone, about, address, has_zalo, zalo_reached, zalo_uid, zalo_status, relationship_status, last_lookup_attempt FROM leads WHERE id = ?1",
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
                address: row.get(10)?,
                has_zalo: row.get(11)?,
                zalo_reached: row.get(12)?,
                zalo_uid: row.get(13)?,
                zalo_status: row.get(14)?,
                relationship_status: row.get(15)?,
                last_lookup_attempt: row.get(16)?,
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
    address: Option<String>,
    state: tauri::State<DbState>,
) -> Result<i32, String> {
    let mut conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    add_lead_internal(
        &mut conn, name, company, status, value, owner, email, phone, about, address,
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
    address: Option<String>,
) -> Result<i32, String> {
    let now: String = conn
        .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
            row.get(0)
        })
        .map_err(|e| format!("Database query error: {}", e))?;
    conn.execute(
        "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone, about, address, has_zalo, zalo_reached) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL, 0)",
        rusqlite::params![name, company, status, value, now, owner, email, phone, about, address],
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
    address: Option<String>,
    state: tauri::State<DbState>,
) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    conn.execute(
        "UPDATE leads SET name = ?1, company = ?2, status = ?3, value = ?4, owner = ?5, email = ?6, phone = ?7, about = ?8, address = ?9 WHERE id = ?10",
        rusqlite::params![name, company, status, value, owner, email, phone, about, address, id],
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
    districts: Option<Vec<String>>,
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
    } else if let Some(districts_list) = districts {
        // Text mode with Deep Scan (Districts)
        let total_districts = districts_list.len();
        let mut all_results = Vec::new();
        let mut seen_phones = std::collections::HashSet::new();
        let base_loc = location.clone().unwrap_or_else(|| "Vietnam".to_string());

        for (idx, district) in districts_list.iter().enumerate() {
            let combined_loc = format!("{}, {}", district, base_loc);
            
            if let Some(app) = app_handle {
                let progress_val = (idx as f64 / total_districts as f64) * 70.0 + 10.0;
                let _ = app.emit(
                    "scrape-progress",
                    ScrapeProgressPayload {
                        progress: progress_val,
                        status: "running".to_string(),
                        message: format!(
                            "Deep Scan: Searching in {} ({}/{})",
                            district,
                            idx + 1,
                            total_districts
                        ),
                    },
                );
                let _ = app.emit(
                    "scrape-log",
                    ScrapeLogPayload {
                        message: format!("Searching for '{}' in '{}'...", keyword, combined_loc),
                    },
                );
            }

            match scraper.search(&keyword, Some(&combined_loc)).await {
                Ok(res) => {
                    let mut new_count = 0;
                    for biz in res {
                        if !biz.phone.trim().is_empty() {
                            if seen_phones.insert(biz.phone.clone()) {
                                all_results.push(biz);
                                new_count += 1;
                            }
                        } else {
                            all_results.push(biz);
                        }
                    }
                    if let Some(app) = app_handle {
                        let _ = app.emit(
                            "scrape-log",
                            ScrapeLogPayload {
                                message: format!("District '{}' scan finished: found {} businesses ({} new).", district, all_results.len(), new_count),
                            },
                        );
                    }
                }
                Err(e) => {
                    if let Some(app) = app_handle {
                        let _ = app.emit(
                            "scrape-log",
                            ScrapeLogPayload {
                                message: format!("Failed to search in district '{}': {}. Continuing to next district...", district, e),
                            },
                        );
                    }
                }
            }

            // Sleep a small duration to be polite and avoid Google rate limit blocks
            if idx + 1 < total_districts {
                tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
            }
        }
        all_results
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
        let conn_res: Result<usize, String> = (|| {
            let mut conn_guard = conn_mutex
                .lock()
                .map_err(|e| format!("Database lock error: {}", e))?;
            let tx = conn_guard
                .transaction()
                .map_err(|e| format!("Database transaction error: {}", e))?;

            let mut count: usize = 0;
            let now: String = tx
                .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
                    row.get(0)
                })
                .map_err(|e| format!("Database query error: {}", e))?;

            let mut existing_normalized_phones: std::collections::HashSet<String> = std::collections::HashSet::new();
            if let Ok(mut stmt) = tx.prepare("SELECT phone FROM leads") {
                if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
                    for p in rows.flatten() {
                        let norm = normalize_phone_digits(&p);
                        if !norm.is_empty() {
                            existing_normalized_phones.insert(norm);
                        }
                    }
                }
            }

            for biz in &final_businesses {
                let is_mobile = is_vn_mobile(&biz.phone);
                // Currently, Google Maps scraper does not extract email (always empty).
                // If email extraction is added in the future, we can check: let has_email = !biz.email.is_empty();
                let has_email = false;
                
                // Lead must have either a mobile number or an email.
                // If it only has a landline and no email, it is discarded.
                if !is_mobile && !has_email {
                    continue;
                }

                let norm_phone = normalize_phone_digits(&biz.phone);

                // Check if normalized phone already exists in the database (Leads or Contacts)
                if norm_phone.is_empty() || existing_normalized_phones.contains(&norm_phone) {
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

                let has_zalo_val = if biz.zalo.is_some() { Some(1) } else { None };
                let insert_res = tx.execute(
                    "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone, about, address, has_zalo) 
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    rusqlite::params![
                        biz.name,
                        biz.name,
                        "New",
                        0.0,
                        &now,
                        "System",
                        "",
                        &norm_phone,
                        about_str,
                        biz.address,
                        has_zalo_val
                    ]
                );
                if insert_res.is_ok() {
                    count += 1;
                    existing_normalized_phones.insert(norm_phone);
                }
            }
            tx.commit().map_err(|e| format!("Transaction commit error: {}", e))?;
            Ok(count)
        })();

        match conn_res {
            Ok(c) => c,
            Err(e) => {
                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "scrape-log",
                        ScrapeLogPayload {
                            message: format!("Database transaction failed: {}", e),
                        },
                    );
                }
                0
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
pub async fn start_google_maps_scrape(
    app_handle: tauri::AppHandle,
    keyword: String,
    mode: String,
    location: Option<String>,
    bbox: Option<[f64; 4]>,
    step_km: Option<f64>,
    districts: Option<Vec<String>>,
    headless: Option<bool>,
) -> Result<(), String> {
    let app_handle_clone = app_handle.clone();

    tokio::spawn(async move {
        let db_state = app_handle_clone.state::<DbState>();
        let browser_state = app_handle_clone.state::<BrowserState>();

        let manager = browser_state.manager.clone();
        let is_headless = headless.unwrap_or(true);

        let _ = app_handle_clone.emit(
            "scrape-log",
            ScrapeLogPayload {
                message: "Starting browser...".to_string(),
            },
        );

        // 1. Start browser (always close the existing one first to guarantee correct headless mode & clean session pool)
        {
            let _guard = browser_state.start_lock.lock().await;
            let _ = manager.close().await;
            if let Err(e) = manager.start(is_headless).await {
                let _ = app_handle_clone.emit(
                    "scrape-log",
                    ScrapeLogPayload {
                        message: format!("Failed to start browser: {}", e),
                    },
                );
                let _ = app_handle_clone.emit(
                    "scrape-progress",
                    ScrapeProgressPayload {
                        progress: 100.0,
                        status: "failed".to_string(),
                        message: format!("Failed to start browser: {}", e),
                    },
                );
                return;
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
                let _ = app_handle_clone.emit(
                    "scrape-progress",
                    ScrapeProgressPayload {
                        progress: 100.0,
                        status: "failed".to_string(),
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
            districts,
        )
        .await;
    });

    Ok(())
}

#[tauri::command]
pub async fn stop_google_maps_scrape(
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let browser_state = app_handle.state::<BrowserState>();
    let manager = browser_state.manager.clone();

    // Close the browser to immediately abort any active scrape sessions
    let _ = manager.close().await;

    let _ = app_handle.emit(
        "scrape-log",
        ScrapeLogPayload {
            message: "Scrape campaign stopped by user.".to_string(),
        },
    );

    let _ = app_handle.emit(
        "scrape-progress",
        ScrapeProgressPayload {
            progress: 100.0,
            status: "failed".to_string(),
            message: "Campaign stopped by user.".to_string(),
        },
    );

    let _ = app_handle.emit(
        "scrape-complete",
        ScrapeCompletePayload {
            total_scraped: 0,
            message: "Campaign stopped by user.".to_string(),
        },
    );

    Ok(())
}

#[tauri::command]
pub fn get_preset_provinces() -> Result<serde_json::Value, String> {
    let json_data = serde_json::json!({
        "Thành phố Cần Thơ": { "minLat": 9.9, "maxLat": 10.3, "minLng": 105.3, "maxLng": 105.9 },
        "Thành phố Đà Nẵng": { "minLat": 15.9, "maxLat": 16.3, "minLng": 107.9, "maxLng": 108.3 },
        "Thành phố Hà Nội": { "minLat": 20.58, "maxLat": 21.36, "minLng": 105.28, "maxLng": 106.03 },
        "Thành phố Hải Phòng": { "minLat": 20.68, "maxLat": 21.03, "minLng": 106.52, "maxLng": 106.9 },
        "Thành phố Hồ Chí Minh": { "minLat": 10.37, "maxLat": 11.16, "minLng": 106.36, "maxLng": 107.03 },
        "Thành phố Huế": { "minLat": 15.9949, "maxLat": 17.1109, "minLng": 107.0343, "maxLng": 108.4806 },
        "Tỉnh An Giang": { "minLat": 8.9946, "maxLat": 10.9623, "minLng": 103.0522, "maxLng": 105.5756 },
        "Tỉnh Bà Rịa - Vũng Tàu": { "minLat": 10.3, "maxLat": 10.8, "minLng": 107, "maxLng": 107.6 },
        "Tỉnh Bạc Liêu": { "minLat": 8.2912, "maxLat": 9.6375, "minLng": 105.2324, "maxLng": 106.0553 },
        "Tỉnh Bắc Giang": { "minLat": 21.122, "maxLat": 21.6263, "minLng": 105.8806, "maxLng": 107.0336 },
        "Tỉnh Bắc Kạn": { "minLat": 21.8043, "maxLat": 22.7414, "minLng": 105.4311, "maxLng": 106.247 },
        "Tỉnh Bắc Ninh": { "minLat": 20.9693, "maxLat": 21.6263, "minLng": 105.8806, "maxLng": 107.0336 },
        "Tỉnh Bến Tre": { "minLat": 9.4273, "maxLat": 10.3403, "minLng": 106.0275, "maxLng": 106.9934 },
        "Tỉnh Bình Dương": { "minLat": 10.85, "maxLat": 11.52, "minLng": 106.38, "maxLng": 106.91 },
        "Tỉnh Bình Định": { "minLat": 13.5152, "maxLat": 14.7032, "minLng": 108.6011, "maxLng": 109.5849 },
        "Tỉnh Bình Phước": { "minLat": 11.3016, "maxLat": 12.2985, "minLng": 106.4116, "maxLng": 107.4271 },
        "Tỉnh Bình Thuận": { "minLat": 9.378, "maxLat": 11.5561, "minLng": 107.3942, "maxLng": 109.4019 },
        "Tỉnh Cà Mau": { "minLat": 8.1791, "maxLat": 9.6375, "minLng": 103.4821, "maxLng": 106.0553 },
        "Tỉnh Cao Bằng": { "minLat": 22.3569, "maxLat": 23.1188, "minLng": 105.2667, "maxLng": 106.8376 },
        "Tỉnh Đắk Lắk": { "minLat": 12.1605, "maxLat": 13.6953, "minLng": 107.4842, "maxLng": 109.6665 },
        "Tỉnh Đắk Nông": { "minLat": 11.7489, "maxLat": 12.8125, "minLng": 107.2061, "maxLng": 108.1164 },
        "Tỉnh Điện Biên": { "minLat": 20.8927, "maxLat": 22.5479, "minLng": 102.1439, "maxLng": 103.5988 },
        "Tỉnh Đồng Nai": { "minLat": 10.73, "maxLat": 11.45, "minLng": 106.75, "maxLng": 107.58 },
        "Tỉnh Đồng Tháp": { "minLat": 10.1348, "maxLat": 10.9735, "minLng": 105.1856, "maxLng": 106.9934 },
        "Tỉnh Gia Lai": { "minLat": 12.9962, "maxLat": 14.7032, "minLng": 107.4508, "maxLng": 109.5849 },
        "Tỉnh Hà Giang": { "minLat": 22.1656, "maxLat": 23.3927, "minLng": 104.3343, "maxLng": 105.5741 },
        "Tỉnh Hà Nam": { "minLat": 20.3629, "maxLat": 20.7044, "minLng": 105.769, "maxLng": 106.1832 },
        "Tỉnh Hà Tĩnh": { "minLat": 17.9096, "maxLat": 18.767, "minLng": 105.1035, "maxLng": 106.7537 },
        "Tỉnh Hải Dương": { "minLat": 20.6836, "maxLat": 21.237, "minLng": 106.1242, "maxLng": 106.611 },
        "Tỉnh Hậu Giang": { "minLat": 9.5822, "maxLat": 9.9942, "minLng": 105.3285, "maxLng": 105.8968 },
        "Tỉnh Hoà Bình": { "minLat": 20.3054, "maxLat": 21.1128, "minLng": 104.8365, "maxLng": 105.8571 },
        "Tỉnh Hưng Yên": { "minLat": 20.0772, "maxLat": 21.007, "minLng": 105.8953, "maxLng": 107.1193 },
        "Tỉnh Khánh Hòa": { "minLat": 11.72, "maxLat": 12.87, "minLng": 108.68, "maxLng": 109.35 },
        "Tỉnh Kiên Giang": { "minLat": 9.3, "maxLat": 10.5, "minLng": 103.4, "maxLng": 105.4 },
        "Tỉnh Kon Tum": { "minLat": 13.9217, "maxLat": 15.4176, "minLng": 107.3354, "maxLng": 108.5472 },
        "Tỉnh Lai Châu": { "minLat": 21.6861, "maxLat": 22.8139, "minLng": 102.3205, "maxLng": 103.9858 },
        "Tỉnh Lạng Sơn": { "minLat": 21.3252, "maxLat": 22.4615, "minLng": 106.0954, "maxLng": 107.3641 },
        "Tỉnh Lào Cai": { "minLat": 21.3259, "maxLat": 22.8449, "minLng": 103.5294, "maxLng": 105.1004 },
        "Tỉnh Lâm Đồng": { "minLat": 11.18, "maxLat": 12.3, "minLng": 107.27, "maxLng": 108.78 },
        "Tỉnh Long An": { "minLat": 10.2, "maxLat": 11.03, "minLng": 105.5, "maxLng": 106.68 },
        "Tỉnh Nam Định": { "minLat": 19.49, "maxLat": 20.4992, "minLng": 105.924, "maxLng": 106.9524 },
        "Tỉnh Nghệ An": { "minLat": 18.5521, "maxLat": 20.0024, "minLng": 103.8746, "maxLng": 106.2645 },
        "Tỉnh Ninh Bình": { "minLat": 19.4111, "maxLat": 20.7044, "minLng": 105.5417, "maxLng": 106.9524 },
        "Tỉnh Ninh Thuận": { "minLat": 10.7631, "maxLat": 12.1648, "minLng": 108.5515, "maxLng": 109.5374 },
        "Tỉnh Phú Thọ": { "minLat": 20.3054, "maxLat": 21.7196, "minLng": 104.8143, "maxLng": 105.8571 },
        "Tỉnh Phú Yên": { "minLat": 12.7048, "maxLat": 13.6953, "minLng": 108.6725, "maxLng": 109.6665 },
        "Tỉnh Quảng Bình": { "minLat": 16.921, "maxLat": 18.1316, "minLng": 105.6077, "maxLng": 107.2961 },
        "Tỉnh Quảng Nam": { "minLat": 14.9514, "maxLat": 16.1663, "minLng": 107.2109, "maxLng": 109.0236 },
        "Tỉnh Quảng Ngãi": { "minLat": 13.9217, "maxLat": 15.7973, "minLng": 107.3354, "maxLng": 109.4632 },
        "Tỉnh Quảng Ninh": { "minLat": 20.4646, "maxLat": 21.6635, "minLng": 106.4392, "maxLng": 108.2086 },
        "Tỉnh Quảng Trị": { "minLat": 16.3022, "maxLat": 18.1316, "minLng": 105.6077, "maxLng": 107.692 },
        "Tỉnh Sóc Trăng": { "minLat": 8.7025, "maxLat": 9.9381, "minLng": 105.544, "maxLng": 106.4538 },
        "Tỉnh Sơn La": { "minLat": 20.573, "maxLat": 22.0307, "minLng": 103.2123, "maxLng": 105.0253 },
        "Tỉnh Tây Ninh": { "minLat": 10.3948, "maxLat": 11.7829, "minLng": 105.5004, "maxLng": 106.7472 },
        "Tỉnh Thái Bình": { "minLat": 20.0772, "maxLat": 20.729, "minLng": 106.1097, "maxLng": 107.1193 },
        "Tỉnh Thái Nguyên": { "minLat": 21.3265, "maxLat": 22.7414, "minLng": 105.4311, "maxLng": 106.247 },
        "Tỉnh Thanh Hóa": { "minLat": 19.1875, "maxLat": 20.67, "minLng": 104.376, "maxLng": 106.2691 },
        "Tỉnh Tiền Giang": { "minLat": 10.1399, "maxLat": 10.587, "minLng": 105.8158, "maxLng": 106.9934 },
        "Tỉnh Trà Vinh": { "minLat": 9.0801, "maxLat": 10.0822, "minLng": 105.9531, "maxLng": 106.8723 },
        "Tỉnh Tuyên Quang": { "minLat": 21.4976, "maxLat": 23.3927, "minLng": 104.3343, "maxLng": 105.5983 },
        "Tỉnh Vĩnh Long": { "minLat": 9.0801, "maxLat": 10.3403, "minLng": 105.6819, "maxLng": 106.9934 },
        "Tỉnh Vĩnh Phúc": { "minLat": 21.1556, "maxLat": 21.5738, "minLng": 105.3225, "maxLng": 105.7866 },
        "Tỉnh Yên Bái": { "minLat": 21.3259, "maxLat": 22.2919, "minLng": 103.8862, "maxLng": 105.1004 }
    });
    Ok(json_data)
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

#[derive(Clone, serde::Serialize)]
pub struct ZaloQRErrorPayload {
    pub message: String,
}

fn emit_zalo_qr_error(app_handle: &tauri::AppHandle, message: &str) {
    if let Err(error) = app_handle.emit(
        "zalo-qr-error",
        ZaloQRErrorPayload {
            message: message.to_string(),
        },
    ) {
        eprintln!("Failed to emit Zalo QR error event: {}", error);
    }
}

pub fn persist_authenticated_zalo_session(
    conn: &Connection,
    imei: &str,
    user_agent: &str,
    cookies: &[zca_rs::context::ZaloCookie],
) -> Result<ZaloQRSuccessPayload, String> {
    let imei = imei.trim();
    if imei.is_empty() {
        return Err("Zalo returned a session without a device IMEI".to_string());
    }

    let cookies_json = serde_json::to_string(cookies).map_err(|error| error.to_string())?;
    crate::db::save_zalo_session(conn, imei, user_agent, &cookies_json, "vi")
        .map_err(|error| error.to_string())?;

    Ok(ZaloQRSuccessPayload {
        imei: imei.to_string(),
    })
}

#[tauri::command]
pub async fn start_zalo_qr_login(
    app_handle: tauri::AppHandle,
    _state: tauri::State<'_, DbState>,
) -> Result<(), String> {
    let app_handle_clone = app_handle.clone();

    tokio::spawn(async move {
        let callback_app_handle = app_handle_clone.clone();

        let callback =
            move |event: zca_rs::apis::auth::LoginQREvent| -> zca_rs::apis::auth::QRCallbackResult {
                let is_declined = matches!(
                    &event,
                    zca_rs::apis::auth::LoginQREvent::QRCodeDeclined
                );

                if matches!(
                    &event,
                    zca_rs::apis::auth::LoginQREvent::GotLoginInfo { .. }
                ) {
                    return zca_rs::apis::auth::QRCallbackResult::Continue;
                }

                match handle_zalo_qr_event(event, None) {
                    Ok((event_name, payload)) => {
                        let _ = callback_app_handle.emit(&event_name, payload);
                    }
                    Err(e) => {
                        eprintln!("Error handling Zalo QR event: {}", e);
                        emit_zalo_qr_error(
                            &callback_app_handle,
                            "Zalo confirmed the scan but did not create a usable session. Please try again.",
                        );
                    }
                }

                if is_declined {
                    zca_rs::apis::auth::QRCallbackResult::Cancel
                } else {
                    zca_rs::apis::auth::QRCallbackResult::Continue
                }
            };

        let options = zca_rs::Options::default();
        match zca_rs::Zalo::login_qr(options, callback).await {
            Ok(zalo) => {
                let cookies = zalo.ctx.cookie_jar.to_vec();

                let mut display_name = None;
                let mut avatar = None;
                if let Ok(info) = zalo.fetch_account_info().await {
                    let (n, p, a) = extract_zalo_profile_info(&info);
                    display_name = n.or(p);
                    avatar = a;
                }

                let db_state = app_handle_clone.state::<DbState>();
                let conn = match db_state.conn.lock() {
                    Ok(conn) => conn,
                    Err(error) => {
                        eprintln!("Failed to lock database for Zalo session: {}", error);
                        emit_zalo_qr_error(
                            &app_handle_clone,
                            "Zalo login succeeded, but the session could not be saved. Please try again.",
                        );
                        return;
                    }
                };

                match persist_authenticated_zalo_session(
                    &conn,
                    &zalo.ctx.imei,
                    &zalo.ctx.user_agent,
                    &cookies,
                ) {
                    Ok(payload) => {
                        if display_name.is_some() || avatar.is_some() {
                            let _ = crate::db::update_zalo_session_profile(
                                &conn,
                                &zalo.ctx.imei,
                                display_name.as_deref(),
                                avatar.as_deref(),
                            );
                        }
                        match serde_json::to_value(payload) {
                            Ok(payload) => {
                                let _ = app_handle_clone.emit("zalo-qr-success", payload);
                            }
                            Err(error) => {
                                eprintln!("Failed to serialize Zalo QR success event: {}", error);
                                emit_zalo_qr_error(
                                    &app_handle_clone,
                                    "Zalo login succeeded, but the session could not be saved. Please try again.",
                                );
                            }
                        }
                    }
                    Err(error) => {
                        eprintln!("Failed to persist Zalo session: {}", error);
                        emit_zalo_qr_error(
                            &app_handle_clone,
                            "Zalo login succeeded, but the session could not be saved. Please try again.",
                        );
                    }
                }
            }
            Err(error) => {
                eprintln!("Zalo QR login failed or cancelled: {:?}", error);
                if !matches!(
                    error,
                    zca_rs::ZaloError::LoginQRAborted | zca_rs::ZaloError::LoginQRDeclined
                ) {
                    emit_zalo_qr_error(
                        &app_handle_clone,
                        "Zalo could not complete the login session. No session was saved. Please try again.",
                    );
                }
            }
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
            if let Some(c) = conn {
                let payload = persist_authenticated_zalo_session(c, &imei, &user_agent, &cookie)?;
                return serde_json::to_value(payload)
                    .map(|payload| ("zalo-qr-success".to_string(), payload))
                    .map_err(|error| error.to_string());
            }

            let payload = serde_json::to_value(ZaloQRSuccessPayload {
                imei,
            })
            .map_err(|e| e.to_string())?;
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

#[tauri::command]
pub async fn verify_zalo_session(
    imei: String,
    state: tauri::State<'_, DbState>,
) -> Result<ZaloSessionStatus, String> {
    let session = {
        let conn = state
            .conn
            .lock()
            .map_err(|e| format!("Database lock error: {}", e))?;
        crate::db::get_zalo_session(&conn, &imei)
            .map_err(|e| format!("Database error: {}", e))?
            .ok_or_else(|| format!("Session with IMEI {} not found", imei))?
    };

    let status = verify_zalo_session_internal(session).await?;
    if status.is_valid {
        if let Ok(conn) = state.conn.lock() {
            let _ = crate::db::update_zalo_session_profile(
                &conn,
                &imei,
                status.display_name.as_deref(),
                status.avatar.as_deref(),
            );
        }
    }
    Ok(status)
}

fn extract_zalo_profile_info(val: &serde_json::Value) -> (Option<String>, Option<String>, Option<String>) {
    let target = val.get("data").unwrap_or(val);

    let name = target.get("zName")
        .or_else(|| target.get("dpName"))
        .or_else(|| target.get("name"))
        .or_else(|| target.get("display_name"))
        .or_else(|| target.get("username"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let phone = target.get("phone")
        .or_else(|| target.get("phoneNumber"))
        .or_else(|| target.get("mobile"))
        .or_else(|| target.get("sdT"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let avatar = target.get("avatar_240")
        .or_else(|| target.get("avatar_120"))
        .or_else(|| target.get("avatar"))
        .or_else(|| target.get("avatarUrl"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    (name, phone, avatar)
}

pub async fn verify_zalo_session_internal(
    session: crate::models::ZaloSession,
) -> Result<ZaloSessionStatus, String> {
    let cookies: Vec<zca_rs::context::ZaloCookie> = match serde_json::from_str(&session.cookies_json) {
        Ok(c) => c,
        Err(e) => {
            return Ok(ZaloSessionStatus {
                imei: session.imei,
                is_valid: false,
                display_name: session.display_name,
                avatar: session.avatar,
                error_message: Some(format!("Invalid cookies format: {}", e)),
            });
        }
    };

    let options = zca_rs::Options::default();
    match zca_rs::Zalo::login(session.imei.clone(), session.user_agent.clone(), cookies, options).await {
        Ok(zalo) => {
            let (name, phone, avatar) = match zalo.fetch_account_info().await {
                Ok(info) => extract_zalo_profile_info(&info),
                Err(e) => {
                    eprintln!("fetch_account_info error for IMEI {}: {:?}", session.imei, e);
                    (None, None, None)
                }
            };

            let display_name = name.or(phone).or(session.display_name);
            let avatar = avatar.or(session.avatar);

            Ok(ZaloSessionStatus {
                imei: session.imei,
                is_valid: true,
                display_name,
                avatar,
                error_message: None,
            })
        }
        Err(e) => Ok(ZaloSessionStatus {
            imei: session.imei,
            is_valid: false,
            display_name: session.display_name,
            avatar: session.avatar,
            error_message: Some(format!("Login failed: {}", e)),
        }),
    }
}

#[tauri::command]
pub async fn verify_all_zalo_sessions(
    state: tauri::State<'_, DbState>,
) -> Result<Vec<ZaloSessionStatus>, String> {
    let sessions = {
        let conn = state
            .conn
            .lock()
            .map_err(|e| format!("Database lock error: {}", e))?;
        crate::db::get_all_zalo_sessions(&conn).map_err(|e| format!("Database error: {}", e))?
    };

    let mut results = Vec::new();
    for session in sessions {
        let imei = session.imei.clone();
        match verify_zalo_session_internal(session).await {
            Ok(status) => {
                if status.is_valid {
                    if let Ok(conn) = state.conn.lock() {
                        let _ = crate::db::update_zalo_session_profile(
                            &conn,
                            &imei,
                            status.display_name.as_deref(),
                            status.avatar.as_deref(),
                        );
                    }
                }
                results.push(status);
            }
            Err(e) => {
                results.push(ZaloSessionStatus {
                    imei: imei.clone(),
                    is_valid: false,
                    display_name: None,
                    avatar: None,
                    error_message: Some(e),
                });
            }
        }
    }
    Ok(results)
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
pub async fn send_zalo_outreach(
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

pub fn normalize_phone_digits(phone: &str) -> String {
    let phone_part = match phone.find(|c: char| {
        !c.is_ascii_digit() && c != ' ' && c != '+' && c != '-' && c != '(' && c != ')' && c != '.' && c != ',' && c != '[' && c != ']'
    }) {
        Some(idx) => &phone[..idx],
        None => phone,
    };

    let digits: String = phone_part.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.starts_with("0084") {
        format!("0{}", &digits[4..])
    } else if digits.starts_with("84") {
        format!("0{}", &digits[2..])
    } else {
        digits
    }
}

fn is_vn_mobile(phone: &str) -> bool {
    let normalized = normalize_phone_digits(phone);
    if normalized.len() != 10 {
        return false;
    }
    
    let prefix = &normalized[0..2];
    matches!(prefix, "03" | "05" | "07" | "08" | "09")
}

fn render_zalo_template(template: &str, lead_name: &str) -> String {
    template
        .replace("[name]", lead_name)
        .replace("{name}", lead_name)
}

fn should_rotate_zalo_session(error: &zca_rs::ZaloError) -> bool {
    match error {
        zca_rs::ZaloError::ApiError { message, code } => {
            if matches!(code, Some(102 | 401 | 403 | 429)) {
                return true;
            }

            let message = message.to_ascii_lowercase();
            [
                "rate limit",
                "too many request",
                "session",
                "login",
                "unauthorized",
                "forbidden",
                "expired",
            ]
            .iter()
            .any(|needle| message.contains(needle))
        }
        zca_rs::ZaloError::Crypto(message) => {
            let message = message.to_ascii_lowercase();
            message.contains("secret key") || message.contains("session")
        }
        _ => false,
    }
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

            let sanitized_phone: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();

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

            if phone != "not-found" && !is_vn_mobile(&phone) {
                let msg = format!("Skipping non-mobile number: {}", phone);
                println!("{}", msg);
                let _ = {
                    let conn_guard = conn_mutex.lock().unwrap();
                    conn_guard.execute(
                        "UPDATE leads SET has_zalo = 0 WHERE id = ?1",
                        rusqlite::params![lead_id],
                    )
                };
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
                let _ = {
                    let conn_guard = conn_mutex.lock().unwrap();
                    conn_guard.execute(
                        "UPDATE leads SET has_zalo = 0 WHERE id = ?1",
                        rusqlite::params![lead_id],
                    )
                };
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

            let formatted_content = render_zalo_template(&template_content, &lead_name);
            total_sent += 1;
            let log_msg = format!("Message sent to {} ({}) (Mock)", lead_name, sanitized_phone);
            if let Some(app) = app_handle {
                let _ = app.emit("outreach-log", OutreachLogPayload { message: log_msg });
            }

            let act_res: Result<(), String> = {
                let mut conn_guard = conn_mutex
                    .lock()
                    .map_err(|e| format!("Database lock error: {}", e))?;
                let tx = conn_guard
                    .transaction()
                    .map_err(|e| format!("Database transaction error: {}", e))?;

                let now: String = tx
                    .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
                        row.get(0)
                    })
                    .map_err(|e| format!("Database query error: {}", e))?;

                tx.execute(
                    "INSERT INTO activities (lead_id, activity_type, content, created_at, performed_by) VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![
                        lead_id,
                        "Outreach",
                        format!("Zalo message sent: {}", formatted_content),
                        &now,
                        "System (Zalo Outreach)"
                    ]
                ).map_err(|e| format!("Database query error: {}", e))?;

                tx.execute(
                    "UPDATE leads SET last_activity = ?1, has_zalo = 1, zalo_reached = 1 WHERE id = ?2",
                    rusqlite::params![&now, lead_id],
                )
                .map_err(|e| format!("Database query error: {}", e))?;

                tx.commit()
                    .map_err(|e| format!("Database transaction commit error: {}", e))?;
                Ok(())
            };

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

        let sanitized_phone: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();

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

        if !is_vn_mobile(&phone) {
            let msg = format!("Skipping non-mobile number: {}", phone);
            println!("{}", msg);
            let _ = {
                let conn_guard = conn_mutex.lock().unwrap();
                conn_guard.execute(
                    "UPDATE leads SET has_zalo = 0 WHERE id = ?1",
                    rusqlite::params![lead_id],
                )
            };
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
                    message: format!(
                        "Looking up Zalo UID for {} ({})",
                        lead_name, sanitized_phone
                    ),
                },
            );
        }

        let user_opt = match zalo.find_user(&sanitized_phone, None).await {
            Ok(opt) => opt,
            Err(e) => {
                let msg = format!(
                    "Failed to look up phone {} for {}: {:?}",
                    sanitized_phone, lead_name, e
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
            let formatted_content = render_zalo_template(&template_content, &lead_name);

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
                    let log_msg = format!("Message sent to {} ({})", lead_name, sanitized_phone);
                    if let Some(app) = app_handle {
                        let _ = app.emit("outreach-log", OutreachLogPayload { message: log_msg });
                    }

                    let act_res: Result<(), String> = {
                        let mut conn_guard = conn_mutex
                            .lock()
                            .map_err(|e| format!("Database lock error: {}", e))?;
                        let tx = conn_guard
                            .transaction()
                            .map_err(|e| format!("Database transaction error: {}", e))?;

                        let now: String = tx
                            .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
                                row.get(0)
                            })
                            .map_err(|e| format!("Database query error: {}", e))?;

                        tx.execute(
                            "INSERT INTO activities (lead_id, activity_type, content, created_at, performed_by) VALUES (?1, ?2, ?3, ?4, ?5)",
                            rusqlite::params![
                                lead_id,
                                "Outreach",
                                format!("Zalo message sent: {}", formatted_content),
                                &now,
                                "System (Zalo Outreach)"
                            ]
                        ).map_err(|e| format!("Database query error: {}", e))?;

                        tx.execute(
                            "UPDATE leads SET last_activity = ?1, has_zalo = 1, zalo_reached = 1 WHERE id = ?2",
                            rusqlite::params![&now, lead_id],
                        )
                        .map_err(|e| format!("Database query error: {}", e))?;

                        tx.commit()
                            .map_err(|e| format!("Database transaction commit error: {}", e))?;
                        Ok(())
                    };

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
                }
                Err(e) => {
                    let msg = format!(
                        "Failed to send Zalo message to {} ({}): {:?}",
                        lead_name, sanitized_phone, e
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
            let msg = format!(
                "Phone number {} for {} not found on Zalo",
                sanitized_phone, lead_name
            );
            let _ = {
                let conn_guard = conn_mutex.lock();
                match conn_guard {
                    Ok(conn) => {
                        let _ = conn.execute(
                            "UPDATE leads SET has_zalo = 0 WHERE id = ?1",
                            rusqlite::params![lead_id],
                        );
                    }
                    Err(_) => {}
                }
            };
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

/// Retrieves all categories from the database.
#[tauri::command]
pub fn get_categories(state: tauri::State<'_, DbState>) -> Result<Vec<Category>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::get_categories(&conn).map_err(|e| format!("Database error: {}", e))
}

/// Creates a new category in the database.
///
/// Returns an error if the category name is empty or contains only whitespace.
#[tauri::command]
pub fn create_category(state: tauri::State<'_, DbState>, name: String) -> Result<i32, String> {
    if name.trim().is_empty() {
        return Err("Category name cannot be empty".to_string());
    }
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::create_category(&conn, &name).map_err(|e| format!("Database error: {}", e))
}

/// Deletes a category by its ID.
#[tauri::command]
pub fn delete_category(state: tauri::State<'_, DbState>, id: i32) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::delete_category(&conn, id).map_err(|e| format!("Database error: {}", e))
}

/// Associates multiple leads with a category.
///
/// Returns early if the `lead_ids` list is empty.
#[tauri::command]
pub fn add_leads_to_category(
    state: tauri::State<'_, DbState>,
    lead_ids: Vec<i32>,
    category_id: i32,
) -> Result<(), String> {
    if lead_ids.is_empty() {
        return Ok(());
    }
    let mut conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    
    let tx = conn
        .transaction()
        .map_err(|e| format!("Transaction error: {}", e))?;

    for lead_id in lead_ids {
        crate::db::add_lead_to_category(&tx, lead_id, category_id)
            .map_err(|e| format!("Database error: {}", e))?;
    }

    tx.commit().map_err(|e| format!("Transaction commit error: {}", e))?;
    Ok(())
}

/// Removes multiple leads from a category.
///
/// Returns early if the `lead_ids` list is empty.
#[tauri::command]
pub fn remove_leads_from_category(
    state: tauri::State<'_, DbState>,
    lead_ids: Vec<i32>,
    category_id: i32,
) -> Result<(), String> {
    if lead_ids.is_empty() {
        return Ok(());
    }
    let mut conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    
    let tx = conn
        .transaction()
        .map_err(|e| format!("Transaction error: {}", e))?;

    for lead_id in lead_ids {
        crate::db::remove_lead_from_category(&tx, lead_id, category_id)
            .map_err(|e| format!("Database error: {}", e))?;
    }

    tx.commit().map_err(|e| format!("Transaction commit error: {}", e))?;
    Ok(())
}

/// Retrieves all leads associated with a specific category.
#[tauri::command]
pub fn get_leads_by_category(
    state: tauri::State<'_, DbState>,
    category_id: i32,
) -> Result<Vec<Lead>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::get_leads_by_category(&conn, category_id).map_err(|e| format!("Database error: {}", e))
}



#[derive(Clone, serde::Serialize)]
pub struct CampaignProgressPayload {
    pub progress: f64,
    pub status: String,
    pub message: String,
}

#[derive(Clone, serde::Serialize)]
pub struct CampaignLogPayload {
    pub message: String,
}

#[derive(Clone, serde::Serialize)]
pub struct CampaignCompletePayload {
    pub total_sent: i32,
    pub message: String,
}

#[tauri::command]
pub async fn send_zalo_campaign(
    app_handle: tauri::AppHandle,
    category_id: i32,
    template_id: i32,
    session_imeis: Vec<String>,
) -> Result<(), String> {
    let app_handle_clone = app_handle.clone();
    tokio::spawn(async move {
        let db_state = app_handle_clone.state::<DbState>();
        let _ = send_zalo_campaign_internal(
            Some(&app_handle_clone),
            &db_state.conn,
            category_id,
            template_id,
            session_imeis,
        )
        .await;
    });
    Ok(())
}

pub async fn send_zalo_campaign_internal(
    app_handle: Option<&tauri::AppHandle>,
    conn_mutex: &std::sync::Mutex<Connection>,
    category_id: i32,
    template_id: i32,
    session_imeis: Vec<String>,
) -> Result<(), String> {
    let mut is_mock = false;
    if session_imeis.iter().any(|i| i.starts_with("mock-")) {
        is_mock = true;
    }

    if let Some(app) = app_handle {
        let _ = app.emit(
            "campaign-progress",
            CampaignProgressPayload {
                progress: 0.0,
                status: "running".to_string(),
                message: "Starting campaign...".to_string(),
            },
        );
    }

    let leads = {
        let conn_guard = conn_mutex.lock().unwrap();
        crate::db::get_leads_by_category(&conn_guard, category_id).unwrap_or_default()
    };

    if leads.is_empty() {
        if let Some(app) = app_handle {
            let _ = app.emit(
                "campaign-progress",
                CampaignProgressPayload {
                    progress: 100.0,
                    status: "completed".to_string(),
                    message: "No leads found in category.".to_string(),
                },
            );
        }
        return Ok(());
    }

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

    let mut eligible_leads = Vec::new();
    for lead in leads {
        let is_valid_phone = if is_mock {
            lead.phone == "not-found"
                || lead.phone == "mock-rate-limit"
                || lead.phone == "mock-send-fail"
                || is_vn_mobile(&lead.phone)
        } else {
            is_vn_mobile(&lead.phone)
        };
        if !is_valid_phone {
            continue;
        }
        if let Some(status) = &lead.zalo_status {
            if status == "VERIFIED_NOT_EXIST" {
                continue;
            }
            if status == "RATE_LIMITED" {
                if let Some(last_lookup) = &lead.last_lookup_attempt {
                    let recent = if let Ok(parsed_time) = chrono::DateTime::parse_from_rfc3339(last_lookup) {
                        let now = chrono::Utc::now();
                        let diff = now - parsed_time.with_timezone(&chrono::Utc);
                        diff.num_seconds() < 3600
                    } else {
                        false
                    };
                    if recent {
                        continue;
                    }
                }
            }
        }

        let has_success = {
            let conn_guard = conn_mutex.lock().unwrap();
            conn_guard
                .query_row(
                    "SELECT COUNT(*) FROM outreach_history WHERE template_id = ?1 AND lead_id = ?2 AND status = 'SUCCESS'",
                    rusqlite::params![template_id, lead.id.unwrap()],
                    |row| row.get::<_, i32>(0),
                )
                .unwrap_or(0) > 0
        };
        if has_success {
            continue;
        }

        eligible_leads.push(lead);
    }

    if eligible_leads.is_empty() {
        if let Some(app) = app_handle {
            let _ = app.emit(
                "campaign-progress",
                CampaignProgressPayload {
                    progress: 100.0,
                    status: "completed".to_string(),
                    message: "No eligible leads to process.".to_string(),
                },
            );
        }
        return Ok(());
    }

    let mut active_clients: Vec<zca_rs::Zalo> = Vec::new();
    let mut active_imeis = Vec::new();

    if !is_mock {
        let mut sessions_to_login = Vec::new();
        {
            let conn_guard = conn_mutex.lock().unwrap();
            for imei in &session_imeis {
                if let Ok(Some(session)) = crate::db::get_zalo_session(&conn_guard, imei) {
                    sessions_to_login.push(session);
                }
            }
        }
        for session in sessions_to_login {
            if let Ok(cookies) = serde_json::from_str(&session.cookies_json) {
                let options = zca_rs::Options::default();
                if let Ok(zalo) = zca_rs::Zalo::login(session.imei.clone(), session.user_agent.clone(), cookies, options).await {
                    active_clients.push(zalo);
                    active_imeis.push(session.imei.clone());
                }
            }
        }
    } else {
        for imei in &session_imeis {
            active_imeis.push(imei.clone());
        }
    }

    if active_imeis.is_empty() {
        if let Some(app) = app_handle {
            let _ = app.emit(
                "campaign-log",
                CampaignLogPayload {
                    message: "No valid sessions available for campaign".to_string(),
                },
            );
            let _ = app.emit(
                "campaign-progress",
                CampaignProgressPayload {
                    progress: 100.0,
                    status: "failed".to_string(),
                    message: "No valid sessions".to_string(),
                },
            );
        }
        return Err("No valid sessions".to_string());
    }

    let mut active_index = 0;
    let mut total_sent = 0;

    for (idx, lead) in eligible_leads.iter().enumerate() {
        if active_imeis.is_empty() {
            if let Some(app) = app_handle {
                let _ = app.emit(
                    "campaign-log",
                    CampaignLogPayload {
                        message: "All sessions exhausted. Stopping campaign.".to_string(),
                    },
                );
            }
            break;
        }

        active_index = active_index % active_imeis.len();
        let current_imei = active_imeis[active_index].clone();
        
        let sanitized_phone: String = lead.phone.chars().filter(|c| c.is_ascii_digit()).collect();
        let formatted_content = render_zalo_template(&template_content, &lead.name);
        
        let mut uid = lead.zalo_uid.clone();
        
        if uid.is_none() || uid.as_deref() == Some("") {
            let mut found = false;
            let mut lookup_failed = false;
            
            if is_mock {
                if lead.phone == "not-found" {
                    found = false;
                } else if lead.phone == "mock-rate-limit" {
                    lookup_failed = true;
                } else {
                    uid = Some(format!("uid-{}", sanitized_phone));
                    found = true;
                }
            } else {
                let zalo = &active_clients[active_index];
                match zalo.find_user(&sanitized_phone, None).await {
                    Ok(Some(u)) => {
                        uid = Some(u.uid);
                        found = true;
                    },
                    Ok(None) => {
                        found = false;
                    },
                    Err(_) => {
                        lookup_failed = true;
                    }
                }
            }

            let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
            
            if lookup_failed {
                let _ = {
                    let conn_guard = conn_mutex.lock().unwrap();
                    conn_guard.execute(
                        "UPDATE leads SET zalo_status = 'RATE_LIMITED', last_lookup_attempt = ?1 WHERE id = ?2",
                        rusqlite::params![now, lead.id.unwrap()],
                    )
                };
                let history = crate::models::OutreachHistory {
                    id: None,
                    lead_id: lead.id.unwrap(),
                    template_id,
                    status: "FAILED".to_string(),
                    error_message: Some("Rate limited or lookup error".to_string()),
                    sent_at: now,
                    session_imei: current_imei.clone(),
                };
                let _ = {
                    let conn_guard = conn_mutex.lock().unwrap();
                    crate::db::log_outreach_attempt(&conn_guard, &history)
                };
                
                // Evict failed session
                active_imeis.remove(active_index);
                if !is_mock {
                    active_clients.remove(active_index);
                }

                if active_imeis.is_empty() {
                    if let Some(app) = app_handle {
                        let _ = app.emit(
                            "campaign-log",
                            CampaignLogPayload {
                                message: "All sessions exhausted. Stopping campaign.".to_string(),
                            },
                        );
                    }
                    break;
                }
                active_index %= active_imeis.len();

                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                continue;
            }
            
            if found {
                let _ = {
                    let conn_guard = conn_mutex.lock().unwrap();
                    conn_guard.execute(
                        "UPDATE leads SET zalo_status = 'VERIFIED_EXIST', zalo_uid = ?1, has_zalo = 1, last_lookup_attempt = ?2 WHERE id = ?3",
                        rusqlite::params![uid, now, lead.id.unwrap()],
                    )
                };
            } else {
                let _ = {
                    let conn_guard = conn_mutex.lock().unwrap();
                    conn_guard.execute(
                        "UPDATE leads SET zalo_status = 'VERIFIED_NOT_EXIST', has_zalo = 0, last_lookup_attempt = ?1 WHERE id = ?2",
                        rusqlite::params![now, lead.id.unwrap()],
                    )
                };
                
                let history = crate::models::OutreachHistory {
                    id: None,
                    lead_id: lead.id.unwrap(),
                    template_id,
                    status: "FAILED".to_string(),
                    error_message: Some("User not found on Zalo".to_string()),
                    sent_at: now,
                    session_imei: current_imei.clone(),
                };
                let _ = {
                    let conn_guard = conn_mutex.lock().unwrap();
                    crate::db::log_outreach_attempt(&conn_guard, &history)
                };
                
                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "campaign-log",
                        CampaignLogPayload {
                            message: format!("Lead {} not found on Zalo.", lead.name),
                        },
                    );
                }
                continue;
            }
        }
        
        let send_result: Result<(), zca_rs::ZaloError> = if is_mock {
            if current_imei == "mock-fail-imei" {
                Err(zca_rs::ZaloError::ApiError {
                    message: "Mock session expired".to_string(),
                    code: Some(102),
                })
            } else if lead.phone == "mock-send-fail" {
                Err(zca_rs::ZaloError::PayloadFormat(
                    "Mock lead send failure".to_string(),
                ))
            } else {
                Ok(())
            }
        } else {
            let zalo = &active_clients[active_index];
            let msg_content = zca_rs::models::MessageContent {
                msg: formatted_content.clone(),
                styles: None,
                urgency: None,
                quote: None,
                mentions: None,
                attachments: None,
                ttl: None,
            };
            if let Some(u) = &uid {
                zalo
                    .send_message(msg_content, u, zca_rs::models::ThreadType::User)
                    .await
                    .map(|_| ())
            } else {
                Err(zca_rs::ZaloError::PayloadFormat(
                    "Missing Zalo UID for message send".to_string(),
                ))
            }
        };
        
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        
        if send_result.is_ok() {
            total_sent += 1;
            let history = crate::models::OutreachHistory {
                id: None,
                lead_id: lead.id.unwrap(),
                template_id,
                status: "SUCCESS".to_string(),
                error_message: None,
                sent_at: now.clone(),
                session_imei: current_imei.clone(),
            };
            let _ = {
                let conn_guard = conn_mutex.lock().unwrap();
                let _ = crate::db::log_outreach_attempt(&conn_guard, &history);
                let _ = conn_guard.execute(
                    "UPDATE leads SET zalo_reached = 1, has_zalo = 1, last_activity = ?1 WHERE id = ?2",
                    rusqlite::params![now, lead.id.unwrap()],
                );
            };
            
            if let Some(app) = app_handle {
                let _ = app.emit(
                    "campaign-log",
                    CampaignLogPayload {
                        message: format!("Sent to {} using session {}", lead.name, current_imei),
                    },
                );
            }
            
            if !is_mock {
                use rand::Rng;
                let sleep_ms = {
                    let mut rng = rand::thread_rng();
                    rng.gen_range(8000..=12000)
                };
                tokio::time::sleep(tokio::time::Duration::from_millis(sleep_ms)).await;
            } else {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }

            active_index = (active_index + 1) % active_imeis.len();
        } else {
            let send_error = send_result.expect_err("send result checked as error");
            let send_error_message = send_error.to_string();
            let rotate_session = should_rotate_zalo_session(&send_error);
            let history = crate::models::OutreachHistory {
                id: None,
                lead_id: lead.id.unwrap(),
                template_id,
                status: "FAILED".to_string(),
                error_message: Some(send_error_message.clone()),
                sent_at: now,
                session_imei: current_imei.clone(),
            };
            let _ = {
                let conn_guard = conn_mutex.lock().unwrap();
                let _ = crate::db::log_outreach_attempt(&conn_guard, &history);
                let _ = conn_guard.execute(
                    "UPDATE leads SET zalo_reached = 0 WHERE id = ?1",
                    rusqlite::params![lead.id.unwrap()],
                );
            };
            
            if let Some(app) = app_handle {
                let _ = app.emit(
                    "campaign-log",
                    CampaignLogPayload {
                        message: format!(
                            "Failed sending to {} using session {}: {}",
                            lead.name, current_imei, send_error_message
                        ),
                    },
                );
            }

            if rotate_session {
                active_imeis.remove(active_index);
                if !is_mock {
                    active_clients.remove(active_index);
                }

                if active_imeis.is_empty() {
                    if let Some(app) = app_handle {
                        let _ = app.emit(
                            "campaign-log",
                            CampaignLogPayload {
                                message: "All sessions exhausted. Stopping campaign.".to_string(),
                            },
                        );
                    }
                    break;
                }
                active_index %= active_imeis.len();
            } else {
                active_index = (active_index + 1) % active_imeis.len();
            }

            let cooldown_ms = if is_mock { 10 } else { 5000 };
            tokio::time::sleep(tokio::time::Duration::from_millis(cooldown_ms)).await;
        }
        
        if let Some(app) = app_handle {
            let progress_val = ((idx + 1) as f64 / eligible_leads.len() as f64) * 100.0;
            let _ = app.emit(
                "campaign-progress",
                CampaignProgressPayload {
                    progress: progress_val,
                    status: "running".to_string(),
                    message: format!("Processed {}/{} leads", idx + 1, eligible_leads.len()),
                },
            );
        }
    }
    
    let final_msg = format!("Campaign completed. Total sent: {}", total_sent);
    if let Some(app) = app_handle {
        let _ = app.emit(
            "campaign-progress",
            CampaignProgressPayload {
                progress: 100.0,
                status: "completed".to_string(),
                message: final_msg.clone(),
            },
        );
        let _ = app.emit(
            "campaign-complete",
            CampaignCompletePayload {
                total_sent,
                message: final_msg,
            },
        );
    }

    Ok(())
}

#[tauri::command]
pub async fn get_campaign_leads(
    state: tauri::State<'_, DbState>,
    category_id: i32,
    segmentation: String,
    filter_template_id: Option<i32>,
) -> Result<Vec<crate::models::Lead>, String> {
    let conn = state.conn.lock().unwrap();
    
    let query_str = match segmentation.as_str() {
        "never_sent" => {
            "SELECT l.id, l.name, l.company, l.status, l.value, l.last_activity, l.owner, l.email, l.phone, l.about, l.address, l.has_zalo, l.zalo_reached, l.zalo_uid, l.zalo_status, l.relationship_status, l.last_lookup_attempt FROM leads l
             JOIN lead_categories lc ON l.id = lc.lead_id
             WHERE lc.category_id = ?1
               AND l.id NOT IN (SELECT DISTINCT lead_id FROM outreach_history WHERE status = 'SUCCESS')"
        },
        "template_success" => {
            if let Some(_) = filter_template_id {
                "SELECT DISTINCT l.id, l.name, l.company, l.status, l.value, l.last_activity, l.owner, l.email, l.phone, l.about, l.address, l.has_zalo, l.zalo_reached, l.zalo_uid, l.zalo_status, l.relationship_status, l.last_lookup_attempt FROM leads l
                 JOIN lead_categories lc ON l.id = lc.lead_id
                 JOIN outreach_history oh ON l.id = oh.lead_id
                 WHERE lc.category_id = ?1
                   AND oh.template_id = ?2
                   AND oh.status = 'SUCCESS'"
            } else {
                return Err("Template ID is required for success filtering".to_string());
            }
        },
        "last_failed" => {
            "SELECT l.id, l.name, l.company, l.status, l.value, l.last_activity, l.owner, l.email, l.phone, l.about, l.address, l.has_zalo, l.zalo_reached, l.zalo_uid, l.zalo_status, l.relationship_status, l.last_lookup_attempt FROM leads l
             JOIN lead_categories lc ON l.id = lc.lead_id
             WHERE lc.category_id = ?1
               AND l.id IN (
                   SELECT lead_id FROM (
                       SELECT lead_id, status, MAX(sent_at) FROM outreach_history GROUP BY lead_id
                   ) WHERE status = 'FAILED'
               )"
        },
        _ => {
            "SELECT l.id, l.name, l.company, l.status, l.value, l.last_activity, l.owner, l.email, l.phone, l.about, l.address, l.has_zalo, l.zalo_reached, l.zalo_uid, l.zalo_status, l.relationship_status, l.last_lookup_attempt FROM leads l
             JOIN lead_categories lc ON l.id = lc.lead_id
             WHERE lc.category_id = ?1"
        }
    };

    let mut leads = Vec::new();
    if segmentation == "template_success" {
        let mut stmt = conn.prepare(query_str).map_err(|e| e.to_string())?;
        let lead_rows = stmt.query_map(rusqlite::params![category_id, filter_template_id.unwrap()], |row| {
            Ok(crate::models::Lead {
                id: row.get(0)?,
                name: row.get(1)?,
                company: row.get(2)?,
                status: row.get(3)?,
                value: row.get(4)?,
                last_activity: row.get(5)?,
                owner: row.get(6)?,
                email: row.get(7)?,
                phone: row.get(8)?,
                about: row.get(9)?,
                address: row.get(10)?,
                has_zalo: row.get(11)?,
                zalo_reached: row.get(12)?,
                zalo_uid: row.get(13)?,
                zalo_status: row.get(14)?,
                relationship_status: row.get(15)?,
                last_lookup_attempt: row.get(16)?,
            })
        }).map_err(|e| e.to_string())?;
        for row in lead_rows {
            leads.push(row.map_err(|e| e.to_string())?);
        }
    } else {
        let mut stmt = conn.prepare(query_str).map_err(|e| e.to_string())?;
        let lead_rows = stmt.query_map([category_id], |row| {
            Ok(crate::models::Lead {
                id: row.get(0)?,
                name: row.get(1)?,
                company: row.get(2)?,
                status: row.get(3)?,
                value: row.get(4)?,
                last_activity: row.get(5)?,
                owner: row.get(6)?,
                email: row.get(7)?,
                phone: row.get(8)?,
                about: row.get(9)?,
                address: row.get(10)?,
                has_zalo: row.get(11)?,
                zalo_reached: row.get(12)?,
                zalo_uid: row.get(13)?,
                zalo_status: row.get(14)?,
                relationship_status: row.get(15)?,
                last_lookup_attempt: row.get(16)?,
            })
        }).map_err(|e| e.to_string())?;
        for row in lead_rows {
            leads.push(row.map_err(|e| e.to_string())?);
        }
    }
    
    Ok(leads)
}

#[tauri::command]
pub fn get_smtp_servers(state: tauri::State<DbState>) -> Result<Vec<SmtpServer>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::get_smtp_servers(&conn).map_err(|e| format!("Database error: {}", e))
}

#[tauri::command]
pub fn save_smtp_server(
    server: SmtpServer,
    state: tauri::State<DbState>,
) -> Result<i32, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::save_smtp_server(&conn, &server).map_err(|e| format!("Database error: {}", e))
}

#[tauri::command]
pub fn delete_smtp_server(id: i32, state: tauri::State<DbState>) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::delete_smtp_server(&conn, id).map_err(|e| format!("Database error: {}", e))
}

#[tauri::command]
pub async fn test_smtp_connection(host: String, port: i32) -> Result<String, String> {
    if port <= 0 || port > 65535 {
        return Err("Invalid port number".to_string());
    }
    let addr = format!("{}:{}", host, port);
    let connect_timeout = std::time::Duration::from_secs(5);
    match tokio::time::timeout(connect_timeout, tokio::net::TcpStream::connect(&addr)).await {
        Ok(Ok(_stream)) => Ok(format!("Successfully connected to {}:{}", host, port)),
        Ok(Err(e)) => Err(format!("Failed to connect to {}:{}: {}", host, port, e)),
        Err(_) => Err(format!("Connection to {}:{} timed out", host, port)),
    }
}

#[tauri::command]
pub fn get_email_templates(state: tauri::State<DbState>) -> Result<Vec<EmailTemplate>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::get_email_templates(&conn).map_err(|e| format!("Database error: {}", e))
}

#[tauri::command]
pub fn create_email_template(
    name: String,
    subject: String,
    content: String,
    state: tauri::State<DbState>,
) -> Result<i32, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::create_email_template(&conn, &name, &subject, &content)
        .map_err(|e| format!("Database error: {}", e))
}

#[tauri::command]
pub fn update_email_template(
    id: i32,
    name: String,
    subject: String,
    content: String,
    state: tauri::State<DbState>,
) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::update_email_template(&conn, id, &name, &subject, &content)
        .map_err(|e| format!("Database error: {}", e))
}

#[tauri::command]
pub fn delete_email_template(id: i32, state: tauri::State<DbState>) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::delete_email_template(&conn, id).map_err(|e| format!("Database error: {}", e))
}

pub fn replace_email_template_variables(template: &str, lead: &Lead) -> String {
    template
        .replace("{{name}}", &lead.name)
        .replace("{{company}}", &lead.company)
        .replace("{{email}}", &lead.email)
        .replace("{{phone}}", &lead.phone)
}

#[tauri::command]
pub async fn send_email_outreach(
    app_handle: tauri::AppHandle,
    smtp_id: i32,
    template_id: i32,
    lead_ids: Vec<i32>,
) -> Result<(), String> {
    let app_handle_clone = app_handle.clone();
    tokio::spawn(async move {
        let db_state = app_handle_clone.state::<DbState>();
        let _ = send_email_outreach_internal(
            Some(&app_handle_clone),
            &db_state.conn,
            smtp_id,
            template_id,
            lead_ids,
        )
        .await;
    });
    Ok(())
}

pub async fn send_email_outreach_internal(
    app_handle: Option<&tauri::AppHandle>,
    conn_mutex: &std::sync::Mutex<Connection>,
    smtp_id: i32,
    template_id: i32,
    lead_ids: Vec<i32>,
) -> Result<(), String> {
    use lettre::message::header::ContentType;
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
    use crate::models::{
        EmailOutreachCompletePayload, EmailOutreachHistory, EmailOutreachLogPayload,
        EmailOutreachProgressPayload,
    };

    if let Some(app) = app_handle {
        let _ = app.emit(
            "email-outreach-progress",
            EmailOutreachProgressPayload {
                progress: 0.0,
                status: "running".to_string(),
                message: "Starting email outreach...".to_string(),
            },
        );
    }

    let (smtp_server, template) = {
        let conn_guard = conn_mutex
            .lock()
            .map_err(|e| format!("Database lock error: {}", e))?;
        let smtp = crate::db::get_smtp_server_by_id(&conn_guard, smtp_id)
            .map_err(|e| format!("Database error: {}", e))?
            .ok_or_else(|| format!("SMTP server with ID {} not found", smtp_id))?;
        let tpl = crate::db::get_email_template_by_id(&conn_guard, template_id)
            .map_err(|e| format!("Database error: {}", e))?
            .ok_or_else(|| format!("Email template with ID {} not found", template_id))?;
        (smtp, tpl)
    };

    if lead_ids.is_empty() {
        if let Some(app) = app_handle {
            let _ = app.emit(
                "email-outreach-progress",
                EmailOutreachProgressPayload {
                    progress: 100.0,
                    status: "completed".to_string(),
                    message: "No leads selected.".to_string(),
                },
            );
        }
        return Ok(());
    }

    let mut total_sent = 0;

    for (idx, &lead_id) in lead_ids.iter().enumerate() {
        let lead_res = {
            let conn_guard = conn_mutex.lock().unwrap();
            get_lead_details_internal(&conn_guard, lead_id)
        };

        let lead = match lead_res {
            Ok(l) => l,
            Err(e) => {
                let err_msg = format!("Lead ID {} not found: {}", lead_id, e);
                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "email-outreach-log",
                        EmailOutreachLogPayload {
                            message: err_msg.clone(),
                        },
                    );
                }
                continue;
            }
        };

        let interpolated_subject = replace_email_template_variables(&template.subject, &lead);
        let interpolated_body = replace_email_template_variables(&template.content, &lead);

        let from_str = if smtp_server.from_name.trim().is_empty() {
            smtp_server.from_email.clone()
        } else {
            format!("{} <{}>", smtp_server.from_name, smtp_server.from_email)
        };

        let to_str = if lead.name.trim().is_empty() {
            lead.email.clone()
        } else {
            format!("{} <{}>", lead.name, lead.email)
        };

        let email_builder = Message::builder()
            .from(from_str.parse().map_err(|e| format!("Invalid from header: {}", e))?)
            .to(to_str.parse().unwrap_or_else(|_| lead.email.parse().unwrap()))
            .subject(interpolated_subject)
            .header(ContentType::TEXT_HTML)
            .body(interpolated_body);

        let send_result = match email_builder {
            Ok(email_msg) => {
                let security_upper = smtp_server.security.to_uppercase();
                let creds = Credentials::new(smtp_server.username.clone(), smtp_server.password.clone());

                let transport_res: Result<AsyncSmtpTransport<Tokio1Executor>, String> =
                    if security_upper == "NONE" {
                        Ok(AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&smtp_server.host)
                            .port(smtp_server.port as u16)
                            .credentials(creds)
                            .build())
                    } else if security_upper == "SSL_TLS" || security_upper == "SSL/TLS" || security_upper == "TLS" || security_upper == "SSL" {
                        AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp_server.host)
                            .map_err(|e| format!("SMTP TLS relay error: {}", e))
                            .map(|b| b.port(smtp_server.port as u16).credentials(creds).build())
                    } else {
                        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&smtp_server.host)
                            .map_err(|e| format!("SMTP STARTTLS relay error: {}", e))
                            .map(|b| b.port(smtp_server.port as u16).credentials(creds).build())
                    };

                match transport_res {
                    Ok(transport) => transport.send(email_msg).await.map_err(|e| e.to_string()),
                    Err(e) => Err(e),
                }
            }
            Err(e) => Err(format!("Failed to build email message: {}", e)),
        };

        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

        match send_result {
            Ok(_) => {
                total_sent += 1;
                let history = EmailOutreachHistory {
                    id: None,
                    lead_id,
                    template_id,
                    smtp_id,
                    status: "SUCCESS".to_string(),
                    error_message: None,
                    sent_at: now.clone(),
                };
                {
                    let conn_guard = conn_mutex.lock().unwrap();
                    let _ = crate::db::log_email_outreach_attempt(&conn_guard, &history);
                    let _ = conn_guard.execute(
                        "INSERT INTO activities (lead_id, activity_type, content, created_at, performed_by) VALUES (?1, ?2, ?3, ?4, ?5)",
                        rusqlite::params![
                            lead_id,
                            "Email Outreach",
                            format!("Sent email template '{}' to {}", template.name, lead.email),
                            &now,
                            "System"
                        ],
                    );
                    let _ = conn_guard.execute(
                        "UPDATE leads SET last_activity = ?1 WHERE id = ?2",
                        rusqlite::params![&now, lead_id],
                    );
                }

                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "email-outreach-log",
                        EmailOutreachLogPayload {
                            message: format!("Successfully sent email to {} ({})", lead.name, lead.email),
                        },
                    );
                }
            }
            Err(err_msg) => {
                let history = EmailOutreachHistory {
                    id: None,
                    lead_id,
                    template_id,
                    smtp_id,
                    status: "FAILED".to_string(),
                    error_message: Some(err_msg.clone()),
                    sent_at: now.clone(),
                };
                {
                    let conn_guard = conn_mutex.lock().unwrap();
                    let _ = crate::db::log_email_outreach_attempt(&conn_guard, &history);
                    let _ = conn_guard.execute(
                        "INSERT INTO activities (lead_id, activity_type, content, created_at, performed_by) VALUES (?1, ?2, ?3, ?4, ?5)",
                        rusqlite::params![
                            lead_id,
                            "Email Outreach",
                            format!("Failed to send email to {}: {}", lead.email, err_msg),
                            &now,
                            "System"
                        ],
                    );
                }

                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "email-outreach-log",
                        EmailOutreachLogPayload {
                            message: format!("Failed sending email to {} ({}): {}", lead.name, lead.email, err_msg),
                        },
                    );
                }
            }
        }

        if let Some(app) = app_handle {
            let progress_val = ((idx + 1) as f64 / lead_ids.len() as f64) * 100.0;
            let _ = app.emit(
                "email-outreach-progress",
                EmailOutreachProgressPayload {
                    progress: progress_val,
                    status: "running".to_string(),
                    message: format!("Processed {}/{} leads", idx + 1, lead_ids.len()),
                },
            );
        }
    }

    if let Some(app) = app_handle {
        let _ = app.emit(
            "email-outreach-complete",
            EmailOutreachCompletePayload {
                total_sent,
                message: format!("Email outreach completed. {} emails sent.", total_sent),
            },
        );
    }

    Ok(())
}

