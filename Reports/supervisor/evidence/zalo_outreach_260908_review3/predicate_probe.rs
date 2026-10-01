use rusqlite::Connection;
pub(crate) fn sanitize_valid_zalo_uid(uid: Option<&str>) -> Option<String> {
    uid.map(str::trim)
        .filter(|value| {
            let lowercase = value.to_ascii_lowercase();
            if value.is_empty()
                || matches!(lowercase.as_str(), "0" | "-0" | "-1" | "null" | "undefined")
            {
                return false;
            }
            if value.chars().all(|c| c == '0') {
                return false;
            }
            true
        })
        .map(str::to_string)
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
pub fn is_vn_mobile(phone: &str) -> bool {
    let normalized = normalize_phone_digits(phone);
    if normalized.len() != 10 {
        return false;
    }

    let prefix = &normalized[0..2];
    matches!(prefix, "03" | "05" | "07" | "08" | "09")
}
pub fn is_valid_zalo_lead_target(phone: Option<&str>, zalo_uid: Option<&str>) -> bool {
    if sanitize_valid_zalo_uid(zalo_uid).is_some() {
        return true;
    }
    phone.map(is_vn_mobile).unwrap_or(false)
}
fn main(){
let sql = " AND (
                (
                    l.zalo_uid IS NOT NULL
                    AND LOWER(TRIM(REPLACE(REPLACE(REPLACE(l.zalo_uid, CHAR(9), ''), CHAR(10), ''), CHAR(13), ''))) NOT IN ('', '0', '-0', '-1', 'null', 'undefined')
                    AND LOWER(TRIM(REPLACE(REPLACE(REPLACE(l.zalo_uid, CHAR(9), ''), CHAR(10), ''), CHAR(13), ''))) GLOB '*[^0]*'
                )
                OR
                (
                    l.phone IS NOT NULL
                    AND (
                        REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(l.phone, ' ', ''), '-', ''), '+', ''), '.', ''), '(', ''), ')', ''), CHAR(9), ''), CHAR(10), '') GLOB '0[35789][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9]'
                        OR
                        REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(l.phone, ' ', ''), '-', ''), '+', ''), '.', ''), '(', ''), ')', ''), CHAR(9), ''), CHAR(10), '') GLOB '0[35789][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9][^0-9]*'
                        OR
                        REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(l.phone, ' ', ''), '-', ''), '+', ''), '.', ''), '(', ''), ')', ''), CHAR(9), ''), CHAR(10), '') GLOB '84[35789][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9]'
                        OR
                        REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(l.phone, ' ', ''), '-', ''), '+', ''), '.', ''), '(', ''), ')', ''), CHAR(9), ''), CHAR(10), '') GLOB '84[35789][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9][^0-9]*'
                        OR
                        REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(l.phone, ' ', ''), '-', ''), '+', ''), '.', ''), '(', ''), ')', ''), CHAR(9), ''), CHAR(10), '') GLOB '0084[35789][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9]'
                        OR
                        REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(l.phone, ' ', ''), '-', ''), '+', ''), '.', ''), '(', ''), ')', ''), CHAR(9), ''), CHAR(10), '') GLOB '0084[35789][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9][^0-9]*'
                    )
                )
            )";
let conn=Connection::open_in_memory().unwrap();
conn.execute_batch("CREATE TABLE leads(phone TEXT,zalo_uid TEXT);").unwrap();
{let phone="1";let uid:Option<&str>=None;conn.execute("DELETE FROM leads",[]).unwrap();conn.execute("INSERT INTO leads VALUES(?1,?2)",rusqlite::params![phone,uid]).unwrap();let sql_ok:bool=conn.query_row(&format!("SELECT EXISTS(SELECT 1 FROM leads l WHERE 1=1{})",sql),[],|r|r.get(0)).unwrap();println!("phone={phone:?} uid={uid:?}: sql={sql_ok}, rust={}",is_valid_zalo_lead_target(Some(phone),uid));}
{let phone="+84-90-123";let uid:Option<&str>=None;conn.execute("DELETE FROM leads",[]).unwrap();conn.execute("INSERT INTO leads VALUES(?1,?2)",rusqlite::params![phone,uid]).unwrap();let sql_ok:bool=conn.query_row(&format!("SELECT EXISTS(SELECT 1 FROM leads l WHERE 1=1{})",sql),[],|r|r.get(0)).unwrap();println!("phone={phone:?} uid={uid:?}: sql={sql_ok}, rust={}",is_valid_zalo_lead_target(Some(phone),uid));}
{let phone="+84 901234567";let uid:Option<&str>=None;conn.execute("DELETE FROM leads",[]).unwrap();conn.execute("INSERT INTO leads VALUES(?1,?2)",rusqlite::params![phone,uid]).unwrap();let sql_ok:bool=conn.query_row(&format!("SELECT EXISTS(SELECT 1 FROM leads l WHERE 1=1{})",sql),[],|r|r.get(0)).unwrap();println!("phone={phone:?} uid={uid:?}: sql={sql_ok}, rust={}",is_valid_zalo_lead_target(Some(phone),uid));}
{let phone="\t0901234567";let uid:Option<&str>=None;conn.execute("DELETE FROM leads",[]).unwrap();conn.execute("INSERT INTO leads VALUES(?1,?2)",rusqlite::params![phone,uid]).unwrap();let sql_ok:bool=conn.query_row(&format!("SELECT EXISTS(SELECT 1 FROM leads l WHERE 1=1{})",sql),[],|r|r.get(0)).unwrap();println!("phone={phone:?} uid={uid:?}: sql={sql_ok}, rust={}",is_valid_zalo_lead_target(Some(phone),uid));}
{let phone="090[123]4567";let uid:Option<&str>=None;conn.execute("DELETE FROM leads",[]).unwrap();conn.execute("INSERT INTO leads VALUES(?1,?2)",rusqlite::params![phone,uid]).unwrap();let sql_ok:bool=conn.query_row(&format!("SELECT EXISTS(SELECT 1 FROM leads l WHERE 1=1{})",sql),[],|r|r.get(0)).unwrap();println!("phone={phone:?} uid={uid:?}: sql={sql_ok}, rust={}",is_valid_zalo_lead_target(Some(phone),uid));}
{let phone="090,1234567";let uid:Option<&str>=None;conn.execute("DELETE FROM leads",[]).unwrap();conn.execute("INSERT INTO leads VALUES(?1,?2)",rusqlite::params![phone,uid]).unwrap();let sql_ok:bool=conn.query_row(&format!("SELECT EXISTS(SELECT 1 FROM leads l WHERE 1=1{})",sql),[],|r|r.get(0)).unwrap();println!("phone={phone:?} uid={uid:?}: sql={sql_ok}, rust={}",is_valid_zalo_lead_target(Some(phone),uid));}
{let phone="";let uid:Option<&str>=Some(" -1 ");conn.execute("DELETE FROM leads",[]).unwrap();conn.execute("INSERT INTO leads VALUES(?1,?2)",rusqlite::params![phone,uid]).unwrap();let sql_ok:bool=conn.query_row(&format!("SELECT EXISTS(SELECT 1 FROM leads l WHERE 1=1{})",sql),[],|r|r.get(0)).unwrap();println!("phone={phone:?} uid={uid:?}: sql={sql_ok}, rust={}",is_valid_zalo_lead_target(Some(phone),uid));}
{let phone="";let uid:Option<&str>=Some("cached-uid");conn.execute("DELETE FROM leads",[]).unwrap();conn.execute("INSERT INTO leads VALUES(?1,?2)",rusqlite::params![phone,uid]).unwrap();let sql_ok:bool=conn.query_row(&format!("SELECT EXISTS(SELECT 1 FROM leads l WHERE 1=1{})",sql),[],|r|r.get(0)).unwrap();println!("phone={phone:?} uid={uid:?}: sql={sql_ok}, rust={}",is_valid_zalo_lead_target(Some(phone),uid));}
{let phone="";let uid:Option<&str>=Some("0\t0");conn.execute("DELETE FROM leads",[]).unwrap();conn.execute("INSERT INTO leads VALUES(?1,?2)",rusqlite::params![phone,uid]).unwrap();let sql_ok:bool=conn.query_row(&format!("SELECT EXISTS(SELECT 1 FROM leads l WHERE 1=1{})",sql),[],|r|r.get(0)).unwrap();println!("phone={phone:?} uid={uid:?}: sql={sql_ok}, rust={}",is_valid_zalo_lead_target(Some(phone),uid));}
{let phone="";let uid:Option<&str>=Some(" 0 ");conn.execute("DELETE FROM leads",[]).unwrap();conn.execute("INSERT INTO leads VALUES(?1,?2)",rusqlite::params![phone,uid]).unwrap();let sql_ok:bool=conn.query_row(&format!("SELECT EXISTS(SELECT 1 FROM leads l WHERE 1=1{})",sql),[],|r|r.get(0)).unwrap();println!("phone={phone:?} uid={uid:?}: sql={sql_ok}, rust={}",is_valid_zalo_lead_target(Some(phone),uid));}
}
