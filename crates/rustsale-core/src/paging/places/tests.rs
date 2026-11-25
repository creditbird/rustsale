//! Synthetic places only.

use super::*;
use crate::db::init_db;
use std::collections::HashSet;

fn conn() -> Connection {
    let conn = init_db(":memory:").expect("db");
    conn.execute_batch("PRAGMA foreign_keys = OFF;")
        .expect("pragma");
    conn
}

fn insert(
    conn: &Connection,
    job: &str,
    id: &str,
    name: &str,
    phone: Option<&str>,
    seen: &str,
    import: &str,
) {
    conn.execute(
        "INSERT INTO google_maps_scrape_results
         (id, job_id, source_unit_id, dedup_key, name, phone, address, detail_status, enrichment_status, import_status, first_seen_at, last_seen_at)
         VALUES (?1, ?2, 'unit', ?3, ?4, ?5, '1 Test Street', 'done', 'pending', ?6, ?7, ?7)",
        rusqlite::params![id, job, format!("dedup:{id}"), name, phone, import, seen],
    )
    .expect("insert place");
}

fn seed(conn: &Connection, n: usize) {
    for i in 0..n {
        insert(
            conn,
            "job-1",
            &format!("r{i:03}"),
            &format!("Cafe {}", i % 5),
            if i % 3 == 0 { None } else { Some("0900000000") },
            // Many identical timestamps on purpose.
            &format!("2026-01-0{}", 1 + i % 3),
            if i % 4 == 0 { "imported" } else { "pending" },
        );
    }
    insert(
        conn,
        "job-2",
        "other",
        "Other job",
        None,
        "2026-01-01",
        "pending",
    );
}

fn walk(conn: &Connection, query: &PlaceQuery, limit: u32) -> Vec<String> {
    let mut ids = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let page = list_places_page_internal(conn, "job-1", query, cursor.as_deref(), limit)
            .expect("page");
        ids.extend(page.items.iter().map(|p| p.id.clone()));
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => return ids,
        }
    }
}

#[test]
fn every_sort_walks_each_place_once_in_both_directions() {
    let conn = conn();
    seed(&conn, 37);
    for sort in [PlaceSort::FirstSeen, PlaceSort::LastSeen, PlaceSort::Name] {
        for descending in [false, true] {
            let query = PlaceQuery {
                sort,
                descending,
                ..PlaceQuery::default()
            };
            let whole =
                list_places_page_internal(&conn, "job-1", &query, None, 200).expect("whole");
            let expected: Vec<_> = whole.items.iter().map(|p| p.id.clone()).collect();
            let paged = walk(&conn, &query, 6);
            assert_eq!(paged, expected, "{sort:?} desc={descending}");
            assert_eq!(paged.iter().collect::<HashSet<_>>().len(), 37);
        }
    }
}

#[test]
fn places_of_another_job_never_appear() {
    let conn = conn();
    seed(&conn, 5);
    let ids = walk(&conn, &PlaceQuery::default(), 2);
    assert_eq!(ids.len(), 5);
    assert!(!ids.contains(&"other".to_string()));
    let none = list_places_page_internal(&conn, "missing", &PlaceQuery::default(), None, 10)
        .expect("page");
    assert_eq!(none.total, 0);
}

#[test]
fn filters_apply_before_paging() {
    let conn = conn();
    seed(&conn, 20);
    let imported = PlaceQuery {
        import_status: Some("imported".into()),
        ..PlaceQuery::default()
    };
    assert_eq!(walk(&conn, &imported, 2).len(), 5);

    let phoneless = PlaceQuery {
        has_phone: Some(false),
        ..PlaceQuery::default()
    };
    let page = list_places_page_internal(&conn, "job-1", &phoneless, None, 3).expect("page");
    assert_eq!(page.total, 7);
    assert!(page.items.iter().all(|p| p.phone.is_none()));

    let with_phone = PlaceQuery {
        has_phone: Some(true),
        ..PlaceQuery::default()
    };
    assert_eq!(
        list_places_page_internal(&conn, "job-1", &with_phone, None, 3)
            .expect("page")
            .total,
        13
    );

    let search = PlaceQuery {
        search: Some("CAFE 2".into()),
        ..PlaceQuery::default()
    };
    let found = list_places_page_internal(&conn, "job-1", &search, None, 50).expect("page");
    assert_eq!(found.total, 4);
    assert!(found.items.iter().all(|p| p.name == "Cafe 2"));
}

#[test]
fn search_treats_wildcards_literally() {
    let conn = conn();
    insert(
        &conn,
        "job-1",
        "a",
        "50% off",
        None,
        "2026-01-01",
        "pending",
    );
    insert(
        &conn,
        "job-1",
        "b",
        "500 off",
        None,
        "2026-01-01",
        "pending",
    );
    let query = PlaceQuery {
        search: Some("50%".into()),
        ..PlaceQuery::default()
    };
    let page = list_places_page_internal(&conn, "job-1", &query, None, 10).expect("page");
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].id, "a");
}

#[test]
fn a_place_found_mid_scroll_does_not_shift_the_next_page() {
    let conn = conn();
    seed(&conn, 12);
    let query = PlaceQuery::default();
    let expected = walk(&conn, &query, 200);
    let first = list_places_page_internal(&conn, "job-1", &query, None, 4).expect("first");
    insert(
        &conn,
        "job-1",
        "r000a",
        "Late",
        Some("1"),
        "2026-01-01",
        "pending",
    );
    let second = list_places_page_internal(&conn, "job-1", &query, first.next_cursor.as_deref(), 4)
        .expect("second");
    let second_ids: Vec<_> = second.items.iter().map(|p| p.id.clone()).collect();
    // r000a sorts inside page 1 (same timestamp as r000), so page 2 must not move.
    assert_eq!(second_ids, expected[4..8]);
}

#[test]
fn cursor_for_another_sort_is_rejected() {
    let conn = conn();
    seed(&conn, 10);
    let page =
        list_places_page_internal(&conn, "job-1", &PlaceQuery::default(), None, 3).expect("page");
    let cursor = page.next_cursor.expect("more");
    let by_name = PlaceQuery {
        sort: PlaceSort::Name,
        ..PlaceQuery::default()
    };
    assert!(matches!(
        list_places_page_internal(&conn, "job-1", &by_name, Some(&cursor), 3),
        Err(PageError::InvalidCursor(_))
    ));
}
