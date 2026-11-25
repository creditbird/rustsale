#![allow(
    unused_imports,
    unused_variables,
    clippy::too_many_arguments,
    clippy::type_complexity
)]
use crate::CoreHandle;
#[allow(unused_imports)]
use rustsale_core::commands::*;
use rustsale_core::conversation::commands::ConversationState;
use rustsale_core::google_maps_jobs::commands::{GoogleMapsJobService, GoogleMapsJobState};
use rustsale_core::google_maps_jobs::types::{
    CreateScrapeJobRequest, ScrapeJob, ScrapeJobStatus, ScrapeMode,
};
use rustsale_core::host::{AppHandle, State};
use rustsale_core::models::{
    Activity, Category, Deal, DiscoveredFacebookPage, EmailTemplate, Lead, ManagedFacebookAccount,
    ManagedSmtpAccount, ManagedZaloAccount, SaveFacebookAccountRequest, SmtpServer,
    ZaloSessionStatus,
};
use std::collections::HashSet;
use std::sync::Mutex;

#[tauri::command]
pub fn get_leads(core: tauri::State<'_, CoreHandle>) -> Result<Vec<Lead>, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::get_leads(st_state)
}

#[tauri::command]
pub fn get_lead_details(core: tauri::State<'_, CoreHandle>, id: i32) -> Result<Lead, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::get_lead_details(id, st_state)
}

#[tauri::command]
pub fn add_lead(
    core: tauri::State<'_, CoreHandle>,
    name: String,
    company: String,
    status: String,
    value: f64,
    owner: String,
    email: String,
    phone: String,
    about: Option<String>,
    address: Option<String>,
    currency: Option<String>,
) -> Result<i32, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::add_lead(
        name, company, status, value, owner, email, phone, about, address, currency, st_state,
    )
}

#[tauri::command]
pub fn update_lead_status(
    core: tauri::State<'_, CoreHandle>,
    id: i32,
    status: String,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::update_lead_status(id, status, st_state)
}

#[tauri::command]
pub fn get_deals(core: tauri::State<'_, CoreHandle>) -> Result<Vec<Deal>, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::get_deals(st_state)
}

#[tauri::command]
pub fn create_deal(
    core: tauri::State<'_, CoreHandle>,
    lead_id: i32,
    title: String,
    value: f64,
    stage: String,
    currency: Option<String>,
) -> Result<Deal, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::create_deal(lead_id, title, value, stage, currency, st_state)
}

#[tauri::command]
pub fn update_deal_stage(
    core: tauri::State<'_, CoreHandle>,
    id: i32,
    stage: String,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::update_deal_stage(id, stage, st_state)
}

#[tauri::command]
pub fn delete_deal(core: tauri::State<'_, CoreHandle>, id: i32) -> Result<(), String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::delete_deal(id, st_state)
}

#[tauri::command]
pub fn get_activities(
    core: tauri::State<'_, CoreHandle>,
    lead_id: i32,
) -> Result<Vec<Activity>, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::get_activities(lead_id, st_state)
}

#[tauri::command]
pub fn get_recent_activities(
    core: tauri::State<'_, CoreHandle>,
    limit: Option<i32>,
) -> Result<Vec<Activity>, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::get_recent_activities(limit, st_state)
}

#[tauri::command]
pub fn add_activity(
    core: tauri::State<'_, CoreHandle>,
    lead_id: i32,
    activity_type: String,
    content: String,
    performed_by: String,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::add_activity(lead_id, activity_type, content, performed_by, st_state)
}

#[tauri::command]
pub fn update_lead(
    core: tauri::State<'_, CoreHandle>,
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
    currency: Option<String>,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::update_lead(
        id, name, company, status, value, owner, email, phone, about, address, currency, st_state,
    )
}

#[tauri::command]
pub fn delete_lead(core: tauri::State<'_, CoreHandle>, id: i32) -> Result<(), String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::delete_lead(id, st_state)
}

#[tauri::command]
pub fn get_currency_settings(
    core: tauri::State<'_, CoreHandle>,
) -> Result<CurrencySettings, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::get_currency_settings(st_state)
}

#[tauri::command]
pub fn update_currency_settings(
    core: tauri::State<'_, CoreHandle>,
    base_currency: String,
    exchange_rate: f64,
) -> Result<CurrencySettings, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::update_currency_settings(base_currency, exchange_rate, st_state)
}

#[tauri::command]
pub async fn start_google_maps_scrape(
    core: tauri::State<'_, CoreHandle>,
    keyword: String,
    mode: String,
    location: Option<String>,
    bbox: Option<[f64; 4]>,
    step_km: Option<f64>,
    districts: Option<Vec<String>>,
    headless: Option<bool>,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app
        .state::<GoogleMapsJobState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::commands::start_google_maps_scrape(
        st_state, keyword, mode, location, bbox, step_km, districts, headless,
    )
    .await
}

#[tauri::command]
pub async fn stop_google_maps_scrape(core: tauri::State<'_, CoreHandle>) -> Result<(), String> {
    let app = core.handle();
    let st_state = app
        .state::<GoogleMapsJobState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::commands::stop_google_maps_scrape(st_state).await
}

#[tauri::command]
pub fn get_preset_provinces(
    core: tauri::State<'_, CoreHandle>,
) -> Result<serde_json::Value, String> {
    let app = core.handle();
    rustsale_core::commands::get_preset_provinces()
}

#[tauri::command]
pub fn plan_grid_crawl(
    core: tauri::State<'_, CoreHandle>,
    bbox: [f64; 4],
    step_km: f64,
) -> Result<rustsale_core::scrapers::grid_planner::GridPlan, String> {
    let app = core.handle();
    rustsale_core::commands::plan_grid_crawl(bbox, step_km)
}

#[tauri::command]
pub async fn start_zalo_qr_login(core: tauri::State<'_, CoreHandle>) -> Result<(), String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::start_zalo_qr_login(app.clone(), st_state).await
}

#[tauri::command]
pub fn get_managed_zalo_accounts(
    core: tauri::State<'_, CoreHandle>,
) -> Result<Vec<ManagedZaloAccount>, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::get_managed_zalo_accounts(st_state)
}

#[tauri::command]
pub fn get_managed_smtp_accounts(
    core: tauri::State<'_, CoreHandle>,
) -> Result<Vec<ManagedSmtpAccount>, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::get_managed_smtp_accounts(st_state)
}

#[tauri::command]
pub fn get_managed_facebook_accounts(
    core: tauri::State<'_, CoreHandle>,
) -> Result<Vec<ManagedFacebookAccount>, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::get_managed_facebook_accounts(st_state)
}

#[tauri::command]
pub async fn save_managed_facebook_account(
    core: tauri::State<'_, CoreHandle>,
    request: SaveFacebookAccountRequest,
) -> Result<ManagedFacebookAccount, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::save_managed_facebook_account(st_state, request).await
}

#[tauri::command]
pub async fn fetch_facebook_pages_from_token(
    core: tauri::State<'_, CoreHandle>,
    token: String,
    app_id: Option<String>,
) -> Result<Vec<DiscoveredFacebookPage>, String> {
    let app = core.handle();
    rustsale_core::commands::fetch_facebook_pages_from_token(token, app_id).await
}

#[tauri::command]
pub fn delete_facebook_account(
    core: tauri::State<'_, CoreHandle>,
    page_id: String,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::delete_facebook_account(st_state, page_id)
}

#[tauri::command]
pub async fn delete_zalo_session(
    core: tauri::State<'_, CoreHandle>,
    imei: String,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::delete_zalo_session(app.clone(), imei, st_state).await
}

#[tauri::command]
pub async fn verify_zalo_session(
    core: tauri::State<'_, CoreHandle>,
    imei: String,
) -> Result<ZaloSessionStatus, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::verify_zalo_session(imei, st_state).await
}

#[tauri::command]
pub async fn verify_all_zalo_sessions(
    core: tauri::State<'_, CoreHandle>,
) -> Result<Vec<ZaloSessionStatus>, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::verify_all_zalo_sessions(st_state).await
}

#[tauri::command]
pub fn get_outreach_templates(
    core: tauri::State<'_, CoreHandle>,
) -> Result<Vec<rustsale_core::models::OutreachTemplate>, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::get_outreach_templates(st_state)
}

#[tauri::command]
pub fn create_outreach_template(
    core: tauri::State<'_, CoreHandle>,
    name: String,
    content: String,
    image_url: Option<String>,
) -> Result<i32, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::create_outreach_template(name, content, image_url, st_state)
}

#[tauri::command]
pub fn update_outreach_template(
    core: tauri::State<'_, CoreHandle>,
    id: i32,
    name: String,
    content: String,
    image_url: Option<String>,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::update_outreach_template(id, name, content, image_url, st_state)
}

#[tauri::command]
pub fn delete_outreach_template(core: tauri::State<'_, CoreHandle>, id: i32) -> Result<(), String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::delete_outreach_template(id, st_state)
}

#[tauri::command]
pub async fn send_zalo_outreach(
    core: tauri::State<'_, CoreHandle>,
    imei: String,
    lead_ids: Vec<i32>,
    template_id: i32,
) -> Result<(), String> {
    let app = core.handle();
    rustsale_core::commands::send_zalo_outreach(app.clone(), imei, lead_ids, template_id).await
}

#[tauri::command]
pub async fn start_zalo_outreach(
    core: tauri::State<'_, CoreHandle>,
    imeis: Vec<String>,
    template_id: i32,
    lead_ids: Vec<i32>,
    enable_burst_limit: Option<bool>,
    burst_max_messages: Option<u32>,
    burst_delay_seconds: Option<u64>,
    inter_message_delay_seconds: Option<u64>,
) -> Result<rustsale_core::commands::ZaloOutreachStartResult, String> {
    let app = core.handle();
    rustsale_core::commands::start_zalo_outreach(
        app.clone(),
        imeis,
        template_id,
        lead_ids,
        enable_burst_limit,
        burst_max_messages,
        burst_delay_seconds,
        inter_message_delay_seconds,
    )
    .await
}

#[tauri::command]
pub fn get_categories(core: tauri::State<'_, CoreHandle>) -> Result<Vec<Category>, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::get_categories(st_state)
}

#[tauri::command]
pub fn get_categories_for_lead(
    core: tauri::State<'_, CoreHandle>,
    lead_id: i32,
) -> Result<Vec<Category>, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::get_categories_for_lead(st_state, lead_id)
}

#[tauri::command]
pub fn create_category(core: tauri::State<'_, CoreHandle>, name: String) -> Result<i32, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::create_category(st_state, name)
}

#[tauri::command]
pub fn delete_category(core: tauri::State<'_, CoreHandle>, id: i32) -> Result<(), String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::delete_category(st_state, id)
}

#[tauri::command]
pub fn add_leads_to_category(
    core: tauri::State<'_, CoreHandle>,
    lead_ids: Vec<i32>,
    category_id: i32,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::add_leads_to_category(st_state, lead_ids, category_id)
}

#[tauri::command]
pub fn remove_leads_from_category(
    core: tauri::State<'_, CoreHandle>,
    lead_ids: Vec<i32>,
    category_id: i32,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::remove_leads_from_category(st_state, lead_ids, category_id)
}

#[tauri::command]
pub fn get_leads_by_category(
    core: tauri::State<'_, CoreHandle>,
    category_id: i32,
) -> Result<Vec<Lead>, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::get_leads_by_category(st_state, category_id)
}

#[tauri::command]
pub async fn send_zalo_campaign(
    core: tauri::State<'_, CoreHandle>,
    category_id: i32,
    template_id: i32,
    session_imeis: Vec<String>,
) -> Result<(), String> {
    let app = core.handle();
    rustsale_core::commands::send_zalo_campaign(
        app.clone(),
        category_id,
        template_id,
        session_imeis,
    )
    .await
}

#[tauri::command]
pub async fn get_campaign_leads(
    core: tauri::State<'_, CoreHandle>,
    category_id: i32,
    segmentation: String,
    filter_template_id: Option<i32>,
    channel: Option<String>,
) -> Result<Vec<rustsale_core::models::Lead>, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::get_campaign_leads(
        st_state,
        category_id,
        segmentation,
        filter_template_id,
        channel,
    )
    .await
}

#[tauri::command]
pub fn get_outreach_leads_page(
    core: tauri::State<'_, CoreHandle>,
    category_id: Option<i32>,
    segmentation: String,
    filter_template_id: Option<i32>,
    channel: String,
    search: Option<String>,
    filter_verified_zalo: bool,
    filter_unreached: bool,
    filter_mobile_only: bool,
    cursor: Option<i32>,
    limit: u32,
) -> Result<OutreachLeadsPage, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::get_outreach_leads_page(
        st_state,
        category_id,
        segmentation,
        filter_template_id,
        channel,
        search,
        filter_verified_zalo,
        filter_unreached,
        filter_mobile_only,
        cursor,
        limit,
    )
}

#[tauri::command]
pub fn resolve_outreach_lead_ids(
    core: tauri::State<'_, CoreHandle>,
    category_id: Option<i32>,
    segmentation: String,
    filter_template_id: Option<i32>,
    channel: String,
    search: Option<String>,
    filter_verified_zalo: bool,
    filter_unreached: bool,
    filter_mobile_only: bool,
    excluded_lead_ids: Vec<i32>,
) -> Result<Vec<i32>, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::resolve_outreach_lead_ids(
        st_state,
        category_id,
        segmentation,
        filter_template_id,
        channel,
        search,
        filter_verified_zalo,
        filter_unreached,
        filter_mobile_only,
        excluded_lead_ids,
    )
}

#[tauri::command]
pub fn save_smtp_server(
    core: tauri::State<'_, CoreHandle>,
    server: SmtpServer,
) -> Result<i32, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::save_smtp_server(server, st_state)
}

#[tauri::command]
pub fn delete_smtp_server(core: tauri::State<'_, CoreHandle>, id: i32) -> Result<(), String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::delete_smtp_server(id, st_state)
}

#[tauri::command]
pub async fn test_smtp_connection(
    core: tauri::State<'_, CoreHandle>,
    host: String,
    port: i32,
) -> Result<String, String> {
    let app = core.handle();
    rustsale_core::commands::test_smtp_connection(host, port).await
}

#[tauri::command]
pub fn get_email_templates(
    core: tauri::State<'_, CoreHandle>,
) -> Result<Vec<EmailTemplate>, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::get_email_templates(st_state)
}

#[tauri::command]
pub fn create_email_template(
    core: tauri::State<'_, CoreHandle>,
    name: String,
    subject: String,
    content: String,
) -> Result<i32, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::create_email_template(name, subject, content, st_state)
}

#[tauri::command]
pub fn update_email_template(
    core: tauri::State<'_, CoreHandle>,
    id: i32,
    name: String,
    subject: String,
    content: String,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::update_email_template(id, name, subject, content, st_state)
}

#[tauri::command]
pub fn delete_email_template(core: tauri::State<'_, CoreHandle>, id: i32) -> Result<(), String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::delete_email_template(id, st_state)
}

#[tauri::command]
pub fn get_templates(
    core: tauri::State<'_, CoreHandle>,
    filter: Option<rustsale_core::models::TemplateFilter>,
) -> Result<Vec<rustsale_core::models::TemplateRecord>, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::get_templates(filter, st_state)
}

#[tauri::command]
pub fn get_template_by_id(
    core: tauri::State<'_, CoreHandle>,
    id: i64,
) -> Result<Option<rustsale_core::models::TemplateRecord>, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::get_template_by_id(id, st_state)
}

#[tauri::command]
pub fn create_template(
    core: tauri::State<'_, CoreHandle>,
    input: rustsale_core::models::CreateTemplateInput,
) -> Result<i64, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::create_template(input, st_state)
}

#[tauri::command]
pub fn update_template(
    core: tauri::State<'_, CoreHandle>,
    input: rustsale_core::models::UpdateTemplateInput,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::update_template(input, st_state)
}

#[tauri::command]
pub fn delete_template(core: tauri::State<'_, CoreHandle>, id: i64) -> Result<(), String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::delete_template(id, st_state)
}

#[tauri::command]
pub fn get_template_folders(
    core: tauri::State<'_, CoreHandle>,
) -> Result<Vec<rustsale_core::models::TemplateFolderRecord>, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::get_template_folders(st_state)
}

#[tauri::command]
pub fn create_template_folder(
    core: tauri::State<'_, CoreHandle>,
    name: String,
    color: Option<String>,
) -> Result<i64, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::create_template_folder(name, color, st_state)
}

#[tauri::command]
pub fn delete_template_folder(core: tauri::State<'_, CoreHandle>, id: i64) -> Result<(), String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::delete_template_folder(id, st_state)
}

#[tauri::command]
pub fn increment_template_usage(core: tauri::State<'_, CoreHandle>, id: i64) -> Result<(), String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::increment_template_usage(id, st_state)
}

#[tauri::command]
pub async fn send_email_outreach(
    core: tauri::State<'_, CoreHandle>,
    smtp_id: i32,
    template_id: i32,
    lead_ids: Vec<i32>,
) -> Result<(), String> {
    let app = core.handle();
    rustsale_core::commands::send_email_outreach(app.clone(), smtp_id, template_id, lead_ids).await
}

#[tauri::command]
pub fn get_group_members_unrestricted(
    core: tauri::State<'_, CoreHandle>,
    conversation_id: i64,
) -> Result<Vec<GroupMemberDetail>, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::get_group_members_unrestricted(st_state, conversation_id)
}

#[tauri::command]
pub fn batch_import_group_members_to_leads(
    core: tauri::State<'_, CoreHandle>,
    request: BatchImportGroupMembersRequest,
) -> Result<BatchImportGroupMembersResult, String> {
    let app = core.handle();
    let st_conversation_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::commands::batch_import_group_members_to_leads(st_conversation_state, request)
}

#[tauri::command]
pub async fn sync_and_deep_scan_group_members(
    core: tauri::State<'_, CoreHandle>,
    conversation_id: i64,
) -> Result<GroupMemberScanResult, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    let st_conversation_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::commands::sync_and_deep_scan_group_members(
        st_state,
        st_conversation_state,
        conversation_id,
    )
    .await
}

#[tauri::command]
pub fn get_hardware_id(core: tauri::State<'_, CoreHandle>) -> Result<HardwareInfo, String> {
    let app = core.handle();
    rustsale_core::commands::get_hardware_id()
}

#[tauri::command]
pub async fn export_leads_csv_file(
    core: tauri::State<'_, CoreHandle>,
    default_filename: String,
    content: String,
) -> Result<Option<String>, String> {
    let app = core.handle();
    rustsale_core::commands::export_leads_csv_file(app.clone(), default_filename, content).await
}

#[tauri::command]
pub async fn pick_lead_import_file(
    core: tauri::State<'_, CoreHandle>,
) -> Result<Option<String>, String> {
    let app = core.handle();
    rustsale_core::commands::pick_lead_import_file(app.clone()).await
}

#[tauri::command]
pub async fn inspect_lead_import_file(
    core: tauri::State<'_, CoreHandle>,
    file_path: String,
    sheet_name: Option<String>,
) -> Result<rustsale_core::lead_importer::types::ImportInspection, String> {
    let app = core.handle();
    rustsale_core::commands::inspect_lead_import_file(file_path, sheet_name).await
}

#[tauri::command]
pub async fn execute_lead_import(
    core: tauri::State<'_, CoreHandle>,
    options: rustsale_core::lead_importer::types::LeadImportOptions,
    on_progress: tauri::ipc::Channel<rustsale_core::lead_importer::types::ImportProgress>,
) -> Result<rustsale_core::lead_importer::types::ImportSummaryResult, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::commands::execute_lead_import(
        app.clone(),
        options,
        rustsale_core::host::Channel::new(move |v| {
            on_progress
                .send(v)
                .map_err(|e| rustsale_core::host::PortError::failed(e.to_string()))
        }),
        st_state,
    )
    .await
}

#[tauri::command]
pub async fn download_lead_import_template(
    core: tauri::State<'_, CoreHandle>,
    format: String,
) -> Result<Option<String>, String> {
    let app = core.handle();
    rustsale_core::commands::download_lead_import_template(app.clone(), format).await
}

#[tauri::command]
pub async fn save_image_file(
    core: tauri::State<'_, CoreHandle>,
    default_filename: Option<String>,
    url: Option<String>,
    base64_data: Option<String>,
) -> Result<Option<String>, String> {
    let app = core.handle();
    rustsale_core::commands::save_image_file(app.clone(), default_filename, url, base64_data).await
}

#[tauri::command]
pub async fn install_portable_linux_update(
    core: tauri::State<'_, CoreHandle>,
    version: String,
) -> Result<bool, String> {
    let app = core.handle();
    rustsale_core::commands::install_portable_linux_update(version).await
}
