use crate::google_maps_jobs::email::normalize_email;
use crate::google_maps_jobs::types::{ScrapeErrorKind, ScrapeJobError};
use rusqlite::{params, Connection, TransactionBehavior};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImportSummary {
    pub created: u64,
    pub duplicates: u64,
    pub rejected: u64,
}

#[derive(Debug)]
struct PendingResult {
    id: String,
    name: String,
    phone: Option<String>,
    normalized_phone: Option<String>,
    email: Option<String>,
    normalized_email: Option<String>,
    address: Option<String>,
}

pub fn normalize_vn_mobile(value: &str) -> Option<String> {
    let digits = value
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>();
    let normalized = if let Some(rest) = digits.strip_prefix("0084") {
        format!("0{rest}")
    } else if let Some(rest) = digits.strip_prefix("84") {
        format!("0{rest}")
    } else {
        digits
    };

    if normalized.len() == 10
        && ["03", "05", "07", "08", "09"]
            .iter()
            .any(|prefix| normalized.starts_with(prefix))
    {
        Some(normalized)
    } else {
        None
    }
}

pub fn qualifies_for_lead(phone: Option<&str>, email: Option<&str>) -> bool {
    phone.and_then(normalize_vn_mobile).is_some() || email.and_then(normalize_email).is_some()
}

pub fn import_pending_results(
    connection: &mut Connection,
    job_id: &str,
) -> Result<ImportSummary, ScrapeJobError> {
    import_results(connection, job_id, false)
}

pub fn import_ready_results(
    connection: &mut Connection,
    job_id: &str,
) -> Result<ImportSummary, ScrapeJobError> {
    import_results(connection, job_id, true)
}

fn import_results(
    connection: &mut Connection,
    job_id: &str,
    ready_only: bool,
) -> Result<ImportSummary, ScrapeJobError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(database_error)?;

    let job_exists = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM google_maps_scrape_jobs WHERE id=?1)",
            params![job_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(database_error)?
        != 0;
    if !job_exists {
        return Err(database_error(format!(
            "Google Maps scrape job {job_id} was not found"
        )));
    }

    let pending_results = {
        let mut statement = transaction
            .prepare(
                "SELECT id, name, phone, normalized_phone, email, normalized_email, address
                 FROM google_maps_scrape_results
                 WHERE job_id=?1 AND import_status='pending' AND inside_requested_area=1
                   AND (?2=0 OR enrichment_status IN (
                       'found', 'not_found', 'failed', 'skipped_outside_area'
                   ))
                 ORDER BY id",
            )
            .map_err(database_error)?;
        let rows = statement
            .query_map(params![job_id, i64::from(ready_only)], |row| {
                Ok(PendingResult {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    phone: row.get(2)?,
                    normalized_phone: row.get(3)?,
                    email: row.get(4)?,
                    normalized_email: row.get(5)?,
                    address: row.get(6)?,
                })
            })
            .map_err(database_error)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(database_error)?
    };

    if pending_results.is_empty() {
        transaction.commit().map_err(database_error)?;
        return Ok(ImportSummary::default());
    }

    let mut known_phones = {
        let mut statement = transaction
            .prepare("SELECT phone FROM leads WHERE trim(phone) <> ''")
            .map_err(database_error)?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(database_error)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(database_error)?
            .into_iter()
            .filter_map(|phone| normalize_vn_mobile(&phone))
            .collect::<HashSet<_>>()
    };
    let mut batch_emails = HashSet::new();
    let mut summary = ImportSummary::default();
    let now = chrono::Utc::now().to_rfc3339();

    {
        let mut phone_lookup = transaction
            .prepare("SELECT EXISTS(SELECT 1 FROM leads WHERE phone=?1)")
            .map_err(database_error)?;
        let mut email_lookup = transaction
            .prepare("SELECT EXISTS(SELECT 1 FROM leads WHERE lower(trim(email))=?1)")
            .map_err(database_error)?;
        let mut insert_lead = transaction
            .prepare(
                "INSERT INTO leads (
                    name, company, status, value, last_activity, owner, email, phone, address
                 ) VALUES (?1, ?2, 'New', 0, ?3, 'Google Maps', ?4, ?5, ?6)",
            )
            .map_err(database_error)?;
        let mut update_result = transaction
            .prepare(
                "UPDATE google_maps_scrape_results
                 SET normalized_phone=?1, normalized_email=?2, import_status=?3,
                     last_error_kind=NULL, last_error_message=NULL, last_seen_at=?4
                 WHERE id=?5 AND job_id=?6 AND import_status='pending'",
            )
            .map_err(database_error)?;

        for result in pending_results {
            let normalized_phone = result
                .normalized_phone
                .as_deref()
                .and_then(normalize_vn_mobile)
                .or_else(|| result.phone.as_deref().and_then(normalize_vn_mobile));
            let normalized_email = result
                .normalized_email
                .as_deref()
                .and_then(normalize_email)
                .or_else(|| result.email.as_deref().and_then(normalize_email));

            if normalized_phone.is_none() && normalized_email.is_none() {
                update_result
                    .execute(params![
                        normalized_phone,
                        normalized_email,
                        "rejected",
                        now,
                        result.id,
                        job_id,
                    ])
                    .map_err(database_error)?;
                summary.rejected += 1;
                continue;
            }

            let duplicate_phone = if let Some(phone) = normalized_phone.as_deref() {
                known_phones.contains(phone)
                    || phone_lookup
                        .query_row(params![phone], |row| row.get::<_, i64>(0))
                        .map_err(database_error)?
                        != 0
            } else {
                false
            };
            let duplicate_email = if let Some(email) = normalized_email.as_deref() {
                batch_emails.contains(email)
                    || email_lookup
                        .query_row(params![email], |row| row.get::<_, i64>(0))
                        .map_err(database_error)?
                        != 0
            } else {
                false
            };

            if duplicate_phone || duplicate_email {
                update_result
                    .execute(params![
                        normalized_phone,
                        normalized_email,
                        "duplicate",
                        now,
                        result.id,
                        job_id,
                    ])
                    .map_err(database_error)?;
                if let Some(phone) = normalized_phone {
                    known_phones.insert(phone);
                }
                if let Some(email) = normalized_email {
                    batch_emails.insert(email);
                }
                summary.duplicates += 1;
                continue;
            }

            let name = result.name.trim();
            let name = if name.is_empty() {
                "Google Maps Lead"
            } else {
                name
            };
            let phone = normalized_phone.as_deref().unwrap_or("");
            let email = normalized_email.as_deref().unwrap_or("");
            insert_lead
                .execute(params![name, name, now, email, phone, result.address])
                .map_err(database_error)?;
            update_result
                .execute(params![
                    normalized_phone,
                    normalized_email,
                    "imported",
                    now,
                    result.id,
                    job_id,
                ])
                .map_err(database_error)?;

            if !phone.is_empty() {
                known_phones.insert(phone.to_string());
            }
            if !email.is_empty() {
                batch_emails.insert(email.to_string());
            }
            summary.created += 1;
        }
    }

    let changed = transaction
        .execute(
            "UPDATE google_maps_scrape_jobs
             SET businesses_qualified=businesses_qualified + ?1,
                 leads_created=leads_created + ?2,
                 event_sequence=event_sequence + 1,
                 version=version + 1,
                 updated_at=?3
             WHERE id=?4",
            params![
                (summary.created + summary.duplicates) as i64,
                summary.created as i64,
                now,
                job_id,
            ],
        )
        .map_err(database_error)?;
    if changed != 1 {
        return Err(database_error(format!(
            "Google Maps scrape job {job_id} changed during lead import"
        )));
    }

    transaction.commit().map_err(database_error)?;
    Ok(summary)
}

fn database_error(error: impl std::fmt::Display) -> ScrapeJobError {
    ScrapeJobError {
        kind: ScrapeErrorKind::Database,
        message: format!("Google Maps lead import database error: {error}"),
        retryable: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use rusqlite::{params, Connection};

    fn connection() -> Connection {
        let connection = init_db(":memory:").unwrap();
        connection
            .execute(
                "INSERT INTO google_maps_scrape_jobs (
                    id, mode, keyword, headless, profile_dir, status, stage, created_at, updated_at
                 ) VALUES ('job-1', 'keyword', 'coffee', 1, '/tmp/job-1', 'running',
                           'importing', '2026-07-26T00:00:00Z', '2026-07-26T00:00:00Z')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO google_maps_scrape_units (
                    id, job_id, unit_key, unit_type, payload_json, status, created_at, updated_at
                 ) VALUES ('unit-1', 'job-1', 'keyword:coffee', 'keyword', '{}', 'completed',
                           '2026-07-26T00:00:00Z', '2026-07-26T00:00:00Z')",
                [],
            )
            .unwrap();
        connection
    }

    fn insert_result(
        connection: &Connection,
        id: &str,
        name: &str,
        phone: Option<&str>,
        email: Option<&str>,
    ) {
        connection
            .execute(
                "INSERT INTO google_maps_scrape_results (
                    id, job_id, source_unit_id, dedup_key, name, phone, normalized_phone,
                    email, normalized_email, inside_requested_area, detail_status,
                    enrichment_status, import_status, first_seen_at, last_seen_at
                 ) VALUES (?1, 'job-1', 'unit-1', ?2, ?3, ?4, ?5, ?6, ?7, 1,
                           'completed', 'found', 'pending',
                           '2026-07-26T00:00:00Z', '2026-07-26T00:00:00Z')",
                params![
                    id,
                    format!("result:{id}"),
                    name,
                    phone,
                    phone.and_then(normalize_vn_mobile),
                    email,
                    email.and_then(crate::google_maps_jobs::email::normalize_email),
                ],
            )
            .unwrap();
    }

    fn lead_count(connection: &Connection) -> i64 {
        connection
            .query_row("SELECT COUNT(*) FROM leads", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn normalizes_supported_vietnamese_mobile_formats() {
        assert_eq!(
            normalize_vn_mobile("090 123 4567"),
            Some("0901234567".into())
        );
        assert_eq!(
            normalize_vn_mobile("+84 90 123 4567"),
            Some("0901234567".into())
        );
        assert_eq!(
            normalize_vn_mobile("0084 90 123 4567"),
            Some("0901234567".into())
        );
        assert_eq!(normalize_vn_mobile("028 1234 5678"), None);
        assert_eq!(normalize_vn_mobile("090123456"), None);
    }

    #[test]
    fn creates_lead_for_mobile_without_email() {
        let mut connection = connection();
        insert_result(
            &connection,
            "mobile",
            "Mobile Lead",
            Some("090 123 4567"),
            None,
        );

        let summary = import_pending_results(&mut connection, "job-1").unwrap();

        assert_eq!(summary.created, 1);
        let (phone, email): (String, String) = connection
            .query_row("SELECT phone, email FROM leads", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        assert_eq!(phone, "0901234567");
        assert_eq!(email, "");
    }

    #[test]
    fn ready_import_preserves_results_with_pending_enrichment() {
        let mut connection = connection();
        insert_result(
            &connection,
            "ready",
            "Ready Lead",
            Some("090 123 4567"),
            None,
        );
        insert_result(
            &connection,
            "pending",
            "Pending Lead",
            Some("091 234 5678"),
            None,
        );
        connection
            .execute(
                "UPDATE google_maps_scrape_results SET enrichment_status='pending' WHERE id='pending'",
                [],
            )
            .unwrap();

        let summary = import_ready_results(&mut connection, "job-1").unwrap();

        assert_eq!(summary.created, 1);
        assert_eq!(lead_count(&connection), 1);
        let statuses = connection
            .prepare("SELECT id, import_status FROM google_maps_scrape_results ORDER BY id")
            .unwrap()
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            statuses,
            vec![
                ("pending".to_string(), "pending".to_string()),
                ("ready".to_string(), "imported".to_string()),
            ]
        );
    }

    #[test]
    fn creates_lead_for_email_without_mobile() {
        let mut connection = connection();
        insert_result(
            &connection,
            "email",
            "Email Lead",
            None,
            Some("Sales@Company.Test"),
        );

        let summary = import_pending_results(&mut connection, "job-1").unwrap();

        assert_eq!(summary.created, 1);
        let (phone, email): (String, String) = connection
            .query_row("SELECT phone, email FROM leads", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        assert_eq!(phone, "");
        assert_eq!(email, "sales@company.test");
    }

    #[test]
    fn rejects_landline_without_email() {
        let mut connection = connection();
        insert_result(
            &connection,
            "landline",
            "Landline",
            Some("028 1234 5678"),
            None,
        );

        let summary = import_pending_results(&mut connection, "job-1").unwrap();

        assert_eq!(summary.rejected, 1);
        assert_eq!(lead_count(&connection), 0);
        let status: String = connection
            .query_row(
                "SELECT import_status FROM google_maps_scrape_results WHERE id='landline'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "rejected");
    }

    #[test]
    fn deduplicates_by_normalized_phone() {
        let mut connection = connection();
        connection
            .execute(
                "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone)
                 VALUES ('Existing', 'Existing', 'New', 0, 'now', 'Google Maps', '', '0901234567')",
                [],
            )
            .unwrap();
        insert_result(
            &connection,
            "phone-dup",
            "Duplicate",
            Some("+84 90 123 4567"),
            None,
        );

        let summary = import_pending_results(&mut connection, "job-1").unwrap();

        assert_eq!(summary.duplicates, 1);
        assert_eq!(lead_count(&connection), 1);
    }

    #[test]
    fn deduplicates_by_case_insensitive_email() {
        let mut connection = connection();
        connection
            .execute(
                "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone)
                 VALUES ('Existing', 'Existing', 'New', 0, 'now', 'Google Maps', 'SALES@COMPANY.TEST', '')",
                [],
            )
            .unwrap();
        insert_result(
            &connection,
            "email-dup",
            "Duplicate",
            None,
            Some("sales@company.test"),
        );

        let summary = import_pending_results(&mut connection, "job-1").unwrap();

        assert_eq!(summary.duplicates, 1);
        assert_eq!(lead_count(&connection), 1);
    }

    #[test]
    fn does_not_create_second_lead_when_phone_and_email_match_different_existing_rows() {
        let mut connection = connection();
        connection
            .execute_batch(
                "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone)
                 VALUES ('Phone', 'Phone', 'New', 0, 'now', 'Google Maps', '', '0901234567');
                 INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone)
                 VALUES ('Email', 'Email', 'New', 0, 'now', 'Google Maps', 'sales@company.test', '');",
            )
            .unwrap();
        insert_result(
            &connection,
            "split-dup",
            "Duplicate",
            Some("0901234567"),
            Some("sales@company.test"),
        );

        let summary = import_pending_results(&mut connection, "job-1").unwrap();

        assert_eq!(summary.duplicates, 1);
        assert_eq!(lead_count(&connection), 2);
    }

    #[test]
    fn duplicate_result_reserves_all_contact_keys_in_the_batch() {
        let mut connection = connection();
        connection
            .execute(
                "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone)
                 VALUES ('Existing', 'Existing', 'New', 0, 'now', 'Google Maps', '', '0901234567')",
                [],
            )
            .unwrap();
        insert_result(
            &connection,
            "a-duplicate",
            "Duplicate Phone",
            Some("0901234567"),
            Some("shared@company.test"),
        );
        insert_result(
            &connection,
            "b-email",
            "Same Contact",
            None,
            Some("shared@company.test"),
        );

        let summary = import_pending_results(&mut connection, "job-1").unwrap();

        assert_eq!(summary.created, 0);
        assert_eq!(summary.duplicates, 2);
        assert_eq!(lead_count(&connection), 1);
    }

    #[test]
    fn rolls_back_batch_when_insert_fails() {
        let mut connection = connection();
        insert_result(&connection, "first", "First", Some("0901234567"), None);
        insert_result(&connection, "fail", "Fail Lead", Some("0911234567"), None);
        connection
            .execute_batch(
                "CREATE TRIGGER reject_fail_lead BEFORE INSERT ON leads
                 WHEN NEW.name='Fail Lead'
                 BEGIN SELECT RAISE(ABORT, 'forced insert failure'); END;",
            )
            .unwrap();

        let error = import_pending_results(&mut connection, "job-1").unwrap_err();

        assert_eq!(error.kind, ScrapeErrorKind::Database);
        assert_eq!(lead_count(&connection), 0);
        let pending: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM google_maps_scrape_results WHERE import_status='pending'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(pending, 2);
    }

    #[test]
    fn returns_database_error_when_existing_contact_data_cannot_be_read() {
        let mut connection = connection();
        connection
            .execute(
                "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone)
                 VALUES ('Corrupt', 'Corrupt', 'New', 0, 'now', 'Google Maps', '', X'80')",
                [],
            )
            .unwrap();
        insert_result(
            &connection,
            "candidate",
            "Candidate",
            Some("0901234567"),
            None,
        );

        let error = import_pending_results(&mut connection, "job-1").unwrap_err();

        assert_eq!(error.kind, ScrapeErrorKind::Database);
        assert_eq!(lead_count(&connection), 1);
        let status: String = connection
            .query_row(
                "SELECT import_status FROM google_maps_scrape_results WHERE id='candidate'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "pending");
    }

    #[test]
    fn replaying_import_is_idempotent() {
        let mut connection = connection();
        insert_result(&connection, "replay", "Replay", Some("0901234567"), None);

        let first = import_pending_results(&mut connection, "job-1").unwrap();
        let second = import_pending_results(&mut connection, "job-1").unwrap();

        assert_eq!(first.created, 1);
        assert_eq!(second.created, 0);
        assert_eq!(lead_count(&connection), 1);
    }

    #[test]
    fn updates_job_counters_and_version_in_the_same_batch() {
        let mut connection = connection();
        connection
            .execute(
                "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone)
                 VALUES ('Existing', 'Existing', 'New', 0, 'now', 'Google Maps', '', '0901234567')",
                [],
            )
            .unwrap();
        insert_result(&connection, "created", "Created", Some("0911234567"), None);
        insert_result(
            &connection,
            "duplicate",
            "Duplicate",
            Some("0901234567"),
            None,
        );
        insert_result(
            &connection,
            "rejected",
            "Rejected",
            Some("0281234567"),
            None,
        );

        let summary = import_pending_results(&mut connection, "job-1").unwrap();

        assert_eq!(summary.created, 1);
        assert_eq!(summary.duplicates, 1);
        assert_eq!(summary.rejected, 1);
        let counters: (i64, i64, i64, i64) = connection
            .query_row(
                "SELECT businesses_qualified, leads_created, event_sequence, version
                 FROM google_maps_scrape_jobs WHERE id='job-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(counters, (2, 1, 1, 1));
    }
}
