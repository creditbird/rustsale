import re

with open("src-tauri/src/commands.rs", "r") as f:
    content = f.read()

new_code = """
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
    let now_utc = chrono::Utc::now();
    for lead in leads {
        if !is_vn_mobile(&lead.phone) {
            continue;
        }
        if let Some(status) = &lead.zalo_status {
            if status == "VERIFIED_NOT_EXIST" {
                continue;
            }
            if status == "RATE_LIMITED" {
                if let Some(last_lookup) = &lead.last_lookup_attempt {
                    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(last_lookup) {
                        if now_utc.signed_duration_since(dt).num_hours() < 1 {
                            continue;
                        }
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

    let mut is_mock = false;
    if session_imeis.iter().any(|i| i.starts_with("mock-")) {
        is_mock = true;
    }

    let mut active_clients: Vec<zca_rs::Zalo> = Vec::new();
    let mut active_imeis = Vec::new();

    if !is_mock {
        let conn_guard = conn_mutex.lock().unwrap();
        for imei in &session_imeis {
            if let Ok(Some(session)) = crate::db::get_zalo_session(&conn_guard, imei) {
                if let Ok(cookies) = serde_json::from_str(&session.cookies_json) {
                    let options = zca_rs::Options::default();
                    if let Ok(zalo) = zca_rs::Zalo::login(session.imei.clone(), session.user_agent.clone(), cookies, options).await {
                        active_clients.push(zalo);
                        active_imeis.push(session.imei.clone());
                    }
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
        let formatted_content = template_content.replace("{name}", &lead.name);
        
        let mut uid = lead.zalo_uid.clone();
        
        if uid.is_none() || uid.as_deref() == Some("") {
            let mut found = false;
            let mut lookup_failed = false;
            
            if is_mock {
                if lead.phone == "not-found" {
                    found = false;
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

            let now = chrono::Utc::now().to_rfc3339();
            
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
                active_index += 1;
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
        
        let mut send_success = false;
        
        if is_mock {
            if current_imei == "mock-fail-imei" {
                send_success = false;
            } else {
                send_success = true;
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
                match zalo.send_message(msg_content, u, zca_rs::models::ThreadType::User).await {
                    Ok(_) => send_success = true,
                    Err(_) => send_success = false,
                }
            }
        }
        
        let now = chrono::Utc::now().to_rfc3339();
        
        if send_success {
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
                let mut rng = rand::thread_rng();
                let sleep_ms = rng.gen_range(2000..=4000);
                tokio::time::sleep(tokio::time::Duration::from_millis(sleep_ms)).await;
            } else {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
        } else {
            let history = crate::models::OutreachHistory {
                id: None,
                lead_id: lead.id.unwrap(),
                template_id,
                status: "FAILED".to_string(),
                error_message: Some("Failed to send message".to_string()),
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
                        message: format!("Failed sending to {} using session {}. Rotating session.", lead.name, current_imei),
                    },
                );
            }
            active_index += 1;
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
"""

with open("src-tauri/src/commands.rs", "w") as f:
    f.write(content + "\n" + new_code)
