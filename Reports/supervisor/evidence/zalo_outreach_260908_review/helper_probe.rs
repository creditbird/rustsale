use rusqlite::Connection;
pub fn filter_outreachable_zalo_lead_ids(
    conn: &Connection,
    lead_ids: &[i32],
) -> Result<(Vec<i32>, Vec<i32>), rusqlite::Error> {
    if lead_ids.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let mut valid_ids = Vec::new();
    let mut invalid_ids = Vec::new();

    for &id in lead_ids {
        let res: Result<(Option<String>, Option<String>), rusqlite::Error> = conn.query_row(
            "SELECT phone, zalo_uid FROM leads WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        );
        match res {
            Ok((phone_opt, uid_opt)) => {
                let has_phone = phone_opt
                    .map(|p| !p.trim().is_empty())
                    .unwrap_or(false);
                let has_uid = uid_opt
                    .map(|u| !u.trim().is_empty())
                    .unwrap_or(false);
                if has_phone || has_uid {
                    valid_ids.push(id);
                } else {
                    invalid_ids.push(id);
                }
            }
            Err(_) => invalid_ids.push(id),
        }
    }

    for &id in &invalid_ids {
        let _ = conn.execute(
            "UPDATE leads SET has_zalo = 0, zalo_status = 'INVALID_PHONE' WHERE id = ?1",
            [id],
        );
    }

    Ok((valid_ids, invalid_ids))
}
pub(crate) fn sanitize_valid_zalo_uid(uid: Option<&str>) -> Option<String> {
    uid.map(str::trim)
        .filter(|value| {
            let lowercase = value.to_ascii_lowercase();
            !value.is_empty() && !matches!(lowercase.as_str(), "0" | "-1" | "null" | "undefined")
        })
        .map(str::to_string)
}
fn main() {
 let conn=Connection::open_in_memory().unwrap();
 conn.execute_batch("CREATE TABLE leads(id INTEGER PRIMARY KEY, phone TEXT, zalo_uid TEXT, has_zalo INTEGER, zalo_status TEXT); INSERT INTO leads VALUES (1,'','0',NULL,'UNTESTED'),(2,'',NULL,NULL,'UNTESTED');").unwrap();
 let (valid,invalid)=filter_outreachable_zalo_lead_ids(&conn,&[1]).unwrap();
 println!("sentinel UID: valid={valid:?} invalid={invalid:?}, runtime_uid={:?}",sanitize_valid_zalo_uid(Some("0")));
 conn.execute_batch("PRAGMA query_only=ON;").unwrap();
 let result=filter_outreachable_zalo_lead_ids(&conn,&[2]);
 let status: String=conn.query_row("SELECT zalo_status FROM leads WHERE id=2",[],|r|r.get(0)).unwrap();
 println!("read-only persistence: result={result:?}, stored_status={status}");
 let empty=Connection::open_in_memory().unwrap();
 println!("missing table: {:?}",filter_outreachable_zalo_lead_ids(&empty,&[1]));
}
