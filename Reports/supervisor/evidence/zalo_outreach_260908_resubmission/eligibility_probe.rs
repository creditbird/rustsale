use rusqlite::Connection;
pub(crate) fn sanitize_valid_zalo_uid(uid: Option<&str>) -> Option<String> {
    uid.map(str::trim)
        .filter(|value| {
            let lowercase = value.to_ascii_lowercase();
            !value.is_empty() && !matches!(lowercase.as_str(), "0" | "-1" | "null" | "undefined")
        })
        .map(str::to_string)
}
pub fn is_valid_zalo_lead_target(phone: Option<&str>, zalo_uid: Option<&str>) -> bool {
    if sanitize_valid_zalo_uid(zalo_uid).is_some() {
        return true;
    }
    phone
        .map(|p| p.chars().any(|c| c.is_ascii_digit()))
        .unwrap_or(false)
}
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
                if is_valid_zalo_lead_target(phone_opt.as_deref(), uid_opt.as_deref()) {
                    valid_ids.push(id);
                } else {
                    invalid_ids.push(id);
                }
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                invalid_ids.push(id);
            }
            Err(e) => return Err(e),
        }
    }

    for &id in &invalid_ids {
        conn.execute(
            "UPDATE leads SET has_zalo = 0, zalo_status = 'INVALID_PHONE' WHERE id = ?1",
            [id],
        )?;
    }

    Ok((valid_ids, invalid_ids))
}
pub fn normalize_phone_digits(phone: &str) -> String {
    let phone_part = match phone.find(|c: char| {
        !c.is_ascii_digit()
            && c != ' '
            && c != '+'
            && c != '-'
            && c != '('
            && c != ')'
            && c != '.'
            && c != ','
            && c != '['
            && c != ']'
    }) {
        Some(idx) => &phone[..idx],
        None => phone,
    };

    let digits: String = phone_part.chars().filter(|c| c.is_ascii_digit()).collect();
    if let Some(rest) = digits.strip_prefix("0084") {
        format!("0{rest}")
    } else if let Some(rest) = digits.strip_prefix("84") {
        format!("0{rest}")
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
fn main(){
let eligibility_sql = " AND (l.phone GLOB '*[0-9]*' OR (l.zalo_uid IS NOT NULL AND l.zalo_uid NOT IN ('', '0', '-1', 'null', 'undefined', 'NULL', 'UNDEFINED') AND l.zalo_uid GLOB '*[1-9]*'))";

let conn=Connection::open_in_memory().unwrap();
conn.execute_batch("CREATE TABLE leads(id INTEGER PRIMARY KEY,phone TEXT,zalo_uid TEXT,has_zalo INTEGER,zalo_status TEXT);").unwrap();
for (i,(phone,uid)) in [("",Some(" -1 ")),("",Some("cached-uid")),("+84-90-123",None),("1",None),("0901234567",Some("0")),("\t\n",None),("",Some("0"))].iter().enumerate(){
 let id=i as i32+1;
 conn.execute("INSERT INTO leads VALUES (?1,?2,?3,NULL,'UNTESTED')",rusqlite::params![id,phone,uid]).unwrap();
 let sql_ok:bool=conn.query_row(&format!("SELECT EXISTS(SELECT 1 FROM leads l WHERE id=?1{})",eligibility_sql),[id],|r|r.get(0)).unwrap();
 let preflight=is_valid_zalo_lead_target(Some(phone),*uid);
 let worker=sanitize_valid_zalo_uid(*uid).is_some()||is_vn_mobile(phone);
 println!("phone={phone:?} uid={uid:?}: sql={sql_ok} preflight={preflight} worker_target={worker}");
}
conn.execute_batch("PRAGMA query_only=ON;").unwrap();
println!("helper read-only invalid lead: {:?}",filter_outreachable_zalo_lead_ids(&conn,&[6]));
println!("helper missing table: {:?}",filter_outreachable_zalo_lead_ids(&Connection::open_in_memory().unwrap(),&[1]));
let lead_id=4;
let conn_mutex=std::sync::Mutex::new(conn);
                    let _ = {
                        let conn_guard = conn_mutex.lock();
                        if let Ok(conn) = conn_guard {
                            let _ = conn.execute(
                                "UPDATE leads SET has_zalo = 0, zalo_status = 'INVALID_PHONE' WHERE id = ?1",
                                rusqlite::params![lead_id],
                            );
                        }
                    };
let state:String=conn_mutex.lock().unwrap().query_row("SELECT zalo_status FROM leads WHERE id=?1",[lead_id],|r|r.get(0)).unwrap();
println!("verbatim worker invalid-phone guard after failed read-only write: continued, stored status={state}");
}
