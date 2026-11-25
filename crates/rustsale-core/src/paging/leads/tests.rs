//! Synthetic data only.

use super::*;
use crate::db::init_db;
use std::collections::HashSet;

fn conn() -> Connection {
    init_db(":memory:").expect("in-memory db")
}

fn insert(conn: &Connection, name: &str, company: &str, status: &str, value: f64) -> i64 {
    conn.execute(
        "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone)
         VALUES (?1, ?2, ?3, ?4, '2026-01-01', 'owner', ?5, '0900000000')",
        rusqlite::params![name, company, status, value, format!("{name}@example.test")],
    )
    .expect("insert lead");
    conn.last_insert_rowid()
}

fn seed(conn: &Connection, n: usize) {
    for i in 0..n {
        // Many duplicates in the sort keys on purpose.
        insert(
            conn,
            &format!("Lead {}", i % 4),
            &format!("Company {}", i % 3),
            if i % 2 == 0 { "New" } else { "Won" },
            f64::from(u32::try_from(i % 5).expect("small")),
        );
    }
}

fn walk(conn: &Connection, query: &LeadQuery, limit: u32) -> Vec<i32> {
    let mut ids = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let page = get_leads_page_internal(conn, query, cursor.as_deref(), limit).expect("page");
        ids.extend(page.items.iter().filter_map(|l| l.id));
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    ids
}

#[test]
fn every_sort_walks_all_rows_once_in_both_directions() {
    let conn = conn();
    seed(&conn, 23);
    for sort in [
        LeadSort::Id,
        LeadSort::Name,
        LeadSort::Company,
        LeadSort::Status,
        LeadSort::Value,
        LeadSort::Owner,
        LeadSort::LastActivity,
    ] {
        for descending in [false, true] {
            let query = LeadQuery {
                sort,
                descending,
                ..LeadQuery::default()
            };
            let ids = walk(&conn, &query, 5);
            let unique: HashSet<_> = ids.iter().copied().collect();
            assert_eq!(ids.len(), 23, "{sort:?} desc={descending}: gap or repeat");
            assert_eq!(unique.len(), 23, "{sort:?} desc={descending}: repeat");
        }
    }
}

#[test]
fn paged_order_equals_the_unpaged_order() {
    let conn = conn();
    seed(&conn, 31);
    let query = LeadQuery {
        sort: LeadSort::Name,
        ..LeadQuery::default()
    };
    let whole = get_leads_page_internal(&conn, &query, None, 200).expect("whole");
    assert!(whole.next_cursor.is_none());
    let whole_ids: Vec<_> = whole.items.iter().filter_map(|l| l.id).collect();
    assert_eq!(walk(&conn, &query, 4), whole_ids);
    assert_eq!(whole.total, 31);
}

#[test]
fn descending_is_the_reverse_of_ascending() {
    let conn = conn();
    seed(&conn, 17);
    let asc = LeadQuery {
        sort: LeadSort::Value,
        ..LeadQuery::default()
    };
    let desc = LeadQuery {
        descending: true,
        ..asc.clone()
    };
    let mut forward = walk(&conn, &asc, 3);
    forward.reverse();
    assert_eq!(walk(&conn, &desc, 3), forward);
}

#[test]
fn rows_inserted_mid_scroll_do_not_shift_later_pages() {
    let conn = conn();
    seed(&conn, 10);
    let by_name = LeadQuery {
        sort: LeadSort::Name,
        ..LeadQuery::default()
    };
    let expected = walk(&conn, &by_name, 200);
    let first = get_leads_page_internal(&conn, &by_name, None, 4).expect("first");
    // A row that sorts before the cursor appears while the user scrolls.
    insert(&conn, "A first", "X", "New", 1.0);
    let second =
        get_leads_page_internal(&conn, &by_name, first.next_cursor.as_deref(), 4).expect("second");
    let first_ids: Vec<_> = first.items.iter().filter_map(|l| l.id).collect();
    let second_ids: Vec<_> = second.items.iter().filter_map(|l| l.id).collect();
    assert_eq!(first_ids, expected[..4]);
    assert_eq!(
        second_ids,
        expected[4..8],
        "page 2 must not shift or repeat"
    );
}

#[test]
fn search_is_case_insensitive_and_wildcards_are_literal() {
    let conn = conn();
    insert(&conn, "Alpha", "Acme", "New", 1.0);
    insert(&conn, "100% real", "Acme", "New", 1.0);
    insert(&conn, "1000 real", "Acme", "New", 1.0);
    let by = |text: &str| {
        let query = LeadQuery {
            search: Some(text.to_string()),
            ..LeadQuery::default()
        };
        get_leads_page_internal(&conn, &query, None, 50).expect("page")
    };
    assert_eq!(by("ALPHA").total, 1);
    assert_eq!(by("100%").total, 1, "% must match literally");
    assert_eq!(by("acme").total, 3);
    assert_eq!(by("   ").total, 3, "blank search means no filter");
    assert_eq!(by("zzz").items.len(), 0);
}

#[test]
fn search_matches_vietnamese_accented_letters_in_both_case_directions() {
    let conn = conn();
    conn.execute(
        "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone)
         VALUES (?1, ?2, 'New', 1.0, '2026-01-01', ?3, ?4, ?5)",
        rusqlite::params![
            "Đặng Văn B",
            "Công ty Ổ Khóa",
            "Nguyễn Thị Ánh",
            "Đặng.B@example.test",
            "0901112222"
        ],
    )
    .expect("insert upper-accented lead");
    conn.execute(
        "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone)
         VALUES (?1, ?2, 'New', 2.0, '2026-01-01', ?3, ?4, ?5)",
        rusqlite::params![
            "đặng thị c",
            "xưởng ổ cắm",
            "trần ánh dương",
            "ánh.c@example.test",
            "0903334444"
        ],
    )
    .expect("insert lower-accented lead");
    insert(&conn, "Khách Thường", "Acme", "New", 3.0);

    let by = |text: &str| {
        let query = LeadQuery {
            search: Some(text.to_string()),
            ..LeadQuery::default()
        };
        get_leads_page_internal(&conn, &query, None, 50).expect("page")
    };

    // Lowercase queries match uppercase Vietnamese columns and vice versa.
    assert_eq!(by("đặng").total, 2);
    assert_eq!(by("ĐẶNG").total, 2);
    assert_eq!(by("Đặng").total, 2);
    assert_eq!(by("đặng văn b").total, 1);
    assert_eq!(by("ĐẶNG THỊ C").total, 1);

    // Owner and email with "Ánh" / "ánh" in both directions.
    assert_eq!(by("ánh").total, 2);
    assert_eq!(by("ÁNH").total, 2);
    assert_eq!(by("Ánh").total, 2);
    assert_eq!(by("nguyễn thị ánh").total, 1);
    assert_eq!(by("TRẦN ÁNH DƯƠNG").total, 1);
    assert_eq!(by("đặng.b@example.test").total, 1);
    assert_eq!(by("ÁNH.C@EXAMPLE.TEST").total, 1);

    // Company with "Ổ" / "ổ" in both directions.
    assert_eq!(by("ổ").total, 2);
    assert_eq!(by("Ổ").total, 2);
    assert_eq!(by("công ty ổ khóa").total, 1);
    assert_eq!(by("XƯỞNG Ổ CẮM").total, 1);

    // Phone still matches and unrelated text returns nothing.
    assert_eq!(by("0901112222").total, 1);
    assert_eq!(by("đặng%").total, 0, "% must stay literal");
}

#[test]
fn unicode_sort_folds_vietnamese_case_consistently_across_pages() {
    let conn = conn();
    // Insert out of sorted order so ORDER BY unicode_lower(...) must reorder them.
    let id_lower_dang = insert(&conn, "đặng bình", "đặng ltd", "New", 4.0);
    let id_upper_o = insert(&conn, "Ổ Khóa", "Ánh Co", "New", 5.0);
    let id_lower_anh = insert(&conn, "ánh mai", "ổ cắm b", "New", 2.0);
    let id_upper_dang = insert(&conn, "Đặng An", "Đặng Corp", "New", 3.0);
    let id_lower_o = insert(&conn, "ổ đĩa", "ánh inc", "New", 6.0);
    let id_upper_anh = insert(&conn, "Ánh Dương", "Ổ Cắm A", "New", 1.0);

    for sort in [LeadSort::Name, LeadSort::Company] {
        for descending in [false, true] {
            let query = LeadQuery {
                sort,
                descending,
                ..LeadQuery::default()
            };
            let whole = get_leads_page_internal(&conn, &query, None, 50).expect("whole");
            let whole_ids: Vec<_> = whole.items.iter().filter_map(|l| l.id).collect();
            assert_eq!(
                walk(&conn, &query, 2),
                whole_ids,
                "{sort:?} desc={descending}: paged order must match unpaged"
            );
        }
    }

    // By Name ascending: unicode_lower folds uppercase and lowercase accented letters together.
    let by_name = LeadQuery {
        sort: LeadSort::Name,
        ..LeadQuery::default()
    };
    let names: Vec<String> = get_leads_page_internal(&conn, &by_name, None, 50)
        .expect("sorted names")
        .items
        .into_iter()
        .map(|l| l.name)
        .collect();
    let expected_ids = vec![
        i32::try_from(id_upper_anh).expect("id"),
        i32::try_from(id_lower_anh).expect("id"),
        i32::try_from(id_upper_dang).expect("id"),
        i32::try_from(id_lower_dang).expect("id"),
        i32::try_from(id_upper_o).expect("id"),
        i32::try_from(id_lower_o).expect("id"),
    ];
    assert_eq!(walk(&conn, &by_name, 2), expected_ids);
    assert_eq!(
        names,
        vec![
            "Ánh Dương",
            "ánh mai",
            "Đặng An",
            "đặng bình",
            "Ổ Khóa",
            "ổ đĩa",
        ]
    );
}

#[test]
fn status_and_category_filters_apply_before_paging() {
    let conn = conn();
    seed(&conn, 12);
    conn.execute("INSERT INTO categories (name) VALUES ('cat')", [])
        .expect("category");
    let category = i32::try_from(conn.last_insert_rowid()).expect("small id");
    for id in 1..=5 {
        conn.execute(
            "INSERT INTO lead_categories (lead_id, category_id) VALUES (?1, ?2)",
            rusqlite::params![id, category],
        )
        .expect("link");
    }
    let by_category = LeadQuery {
        category_id: Some(category),
        ..LeadQuery::default()
    };
    assert_eq!(walk(&conn, &by_category, 2), vec![1, 2, 3, 4, 5]);

    let both = LeadQuery {
        category_id: Some(category),
        status: Some("New".to_string()),
        ..LeadQuery::default()
    };
    let page = get_leads_page_internal(&conn, &both, None, 50).expect("page");
    assert_eq!(page.total, 3);
    assert!(page.items.iter().all(|l| l.status == "New"));
}

#[test]
fn search_and_status_and_cursor_use_distinct_parameters() {
    let conn = conn();
    seed(&conn, 20);
    let query = LeadQuery {
        search: Some("lead".to_string()),
        status: Some("Won".to_string()),
        sort: LeadSort::Name,
        ..LeadQuery::default()
    };
    let ids = walk(&conn, &query, 3);
    assert_eq!(ids.len(), 10);
}

#[test]
fn a_cursor_for_another_sort_is_rejected() {
    let conn = conn();
    seed(&conn, 10);
    let by_name = LeadQuery {
        sort: LeadSort::Name,
        ..LeadQuery::default()
    };
    let page = get_leads_page_internal(&conn, &by_name, None, 3).expect("page");
    let cursor = page.next_cursor.expect("more rows");
    let by_value = LeadQuery {
        sort: LeadSort::Value,
        ..LeadQuery::default()
    };
    assert!(matches!(
        get_leads_page_internal(&conn, &by_value, Some(&cursor), 3),
        Err(PageError::InvalidCursor(_))
    ));
    let reversed = LeadQuery {
        descending: true,
        ..by_name
    };
    assert!(matches!(
        get_leads_page_internal(&conn, &reversed, Some(&cursor), 3),
        Err(PageError::InvalidCursor(_))
    ));
    assert!(matches!(
        get_leads_page_internal(&conn, &LeadQuery::default(), Some("garbage"), 3),
        Err(PageError::InvalidCursor(_))
    ));
}

#[test]
fn page_size_is_clamped_and_total_is_the_filtered_count() {
    let conn = conn();
    seed(&conn, 7);
    let page = get_leads_page_internal(&conn, &LeadQuery::default(), None, 0).expect("page");
    assert_eq!(page.items.len(), 7);
    assert_eq!(page.total, 7);
    let one = get_leads_page_internal(&conn, &LeadQuery::default(), None, 1).expect("page");
    assert_eq!(one.items.len(), 1);
    assert_eq!(one.total, 7);
    assert!(one.next_cursor.is_some());
}

#[test]
fn empty_table_gives_an_empty_last_page() {
    let conn = conn();
    let page = get_leads_page_internal(&conn, &LeadQuery::default(), None, 10).expect("page");
    assert!(page.items.is_empty());
    assert_eq!(page.total, 0);
    assert!(page.next_cursor.is_none());
}

fn insert_filtered(
    conn: &Connection,
    name: &str,
    owner: &str,
    value: f64,
    currency: &str,
    has_zalo: Option<i32>,
    zalo_reached: i32,
) -> i64 {
    conn.execute(
        "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone,
                            currency, has_zalo, zalo_reached)
         VALUES (?1, 'Acme', 'New', ?2, '2026-01-01', ?3, ?4, '0900000000', ?5, ?6, ?7)",
        rusqlite::params![
            name,
            value,
            owner,
            format!("{name}@example.test"),
            currency,
            has_zalo,
            zalo_reached
        ],
    )
    .expect("insert lead");
    conn.last_insert_rowid()
}

#[test]
fn owner_filter_matches_the_listed_owner_and_counts_only_its_rows() {
    let conn = conn();
    insert_filtered(&conn, "A", "An", 1.0, "VND", None, 0);
    insert_filtered(&conn, "B", "an", 2.0, "VND", None, 0);
    insert_filtered(&conn, "C", "Binh", 3.0, "VND", None, 0);

    let by = |owner: &str| {
        let query = LeadQuery {
            owner: Some(owner.to_string()),
            ..LeadQuery::default()
        };
        get_leads_page_internal(&conn, &query, None, 50).expect("page")
    };

    let upper = by("An");
    assert_eq!(upper.total, 1);
    assert_eq!(upper.items[0].owner, "An");
    let lower = by("an");
    assert_eq!(lower.total, 1);
    assert_eq!(lower.items[0].owner, "an");
    assert_eq!(by("  an  ").total, 1, "surrounding spaces are ignored");

    let blank = LeadQuery {
        owner: Some("   ".to_string()),
        ..LeadQuery::default()
    };
    assert_eq!(
        walk(&conn, &blank, 2).len(),
        3,
        "blank owner means no filter"
    );
}

#[test]
fn owner_filter_finds_accented_names_as_the_owner_list_shows_them() {
    let conn = conn();
    insert_filtered(&conn, "A", "Đặng Văn B", 1.0, "VND", None, 0);
    insert_filtered(&conn, "B", "Ưng Văn C", 2.0, "VND", None, 0);
    insert_filtered(&conn, "C", "Nguyễn Thị Ánh", 3.0, "VND", None, 0);
    insert_filtered(&conn, "D", "An", 4.0, "VND", None, 0);

    let owners = get_lead_owners(&conn).expect("owners");
    for owner in ["Đặng Văn B", "Ưng Văn C", "Nguyễn Thị Ánh"] {
        assert!(
            owners.iter().any(|listed| listed == owner),
            "{owner} is in the owner list"
        );
        let query = LeadQuery {
            owner: Some(owner.to_string()),
            ..LeadQuery::default()
        };
        let page = get_leads_page_internal(&conn, &query, None, 50).expect("page");
        assert_eq!(page.total, 1, "the listed {owner} selects its lead");
        assert_eq!(page.items[0].owner, owner);
    }
}

#[test]
fn zalo_states_split_verified_no_zalo_and_unchecked() {
    let conn = conn();
    insert_filtered(&conn, "A", "one", 1.0, "VND", Some(1), 0);
    insert_filtered(&conn, "B", "one", 1.0, "VND", Some(0), 0);
    insert_filtered(&conn, "C", "one", 1.0, "VND", None, 0);
    let by = |zalo| {
        let query = LeadQuery {
            zalo: Some(zalo),
            ..LeadQuery::default()
        };
        get_leads_page_internal(&conn, &query, None, 50)
            .expect("page")
            .total
    };
    assert_eq!(by(ZaloFilter::Verified), 1);
    assert_eq!(by(ZaloFilter::NoZalo), 1);
    assert_eq!(by(ZaloFilter::Unchecked), 1);
}

#[test]
fn outreach_filter_counts_unreached_rows_without_a_flag() {
    let conn = conn();
    insert_filtered(&conn, "A", "one", 1.0, "VND", None, 1);
    insert_filtered(&conn, "B", "one", 1.0, "VND", None, 0);
    let reached = LeadQuery {
        zalo_reached: Some(true),
        ..LeadQuery::default()
    };
    let unreached = LeadQuery {
        zalo_reached: Some(false),
        ..LeadQuery::default()
    };
    assert_eq!(
        get_leads_page_internal(&conn, &reached, None, 50)
            .expect("page")
            .total,
        1
    );
    assert_eq!(
        get_leads_page_internal(&conn, &unreached, None, 50)
            .expect("page")
            .total,
        1
    );
}

#[test]
fn value_range_converts_usd_leads_with_the_stored_rate() {
    let conn = conn();
    conn.execute(
        "INSERT INTO app_metadata (key, value) VALUES ('usd_vnd_exchange_rate', '25000')
         ON CONFLICT(key) DO UPDATE SET value = '25000'",
        [],
    )
    .expect("rate");
    insert_filtered(&conn, "vnd", "one", 20_000_000.0, "VND", None, 0);
    insert_filtered(&conn, "usd small", "one", 100.0, "USD", None, 0);
    insert_filtered(&conn, "usd big", "one", 1_000.0, "USD", None, 0);
    let by = |min_value, max_value| {
        let query = LeadQuery {
            min_value,
            max_value,
            ..LeadQuery::default()
        };
        get_leads_page_internal(&conn, &query, None, 50).expect("page")
    };
    let middle = by(Some(10_000_000.0), Some(30_000_000.0));
    assert_eq!(middle.total, 2, "20m VND and 1000 USD at 25k");
    let small = by(None, Some(9_999_999.0));
    assert_eq!(small.total, 1);
    assert_eq!(small.items[0].name, "usd small");
    assert_eq!(by(Some(40_000_000.0), None).total, 0);
}

#[test]
fn owners_are_distinct_sorted_and_skip_blank_names() {
    let conn = conn();
    insert_filtered(&conn, "A", "Binh", 1.0, "VND", None, 0);
    insert_filtered(&conn, "B", "an", 1.0, "VND", None, 0);
    insert_filtered(&conn, "C", "an", 1.0, "VND", None, 0);
    insert_filtered(&conn, "D", "", 1.0, "VND", None, 0);
    assert_eq!(get_lead_owners(&conn).expect("owners"), vec!["an", "Binh"]);
}
