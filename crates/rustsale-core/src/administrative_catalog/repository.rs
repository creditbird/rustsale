use std::{io, path::Path, time::Duration};

use anyhow::{anyhow, Context, Result};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use super::types::{
    AdministrativeArea, AdministrativeAreaKind, AdministrativeAreaSummary, AdministrativeBounds,
    AdministrativeCatalogState, AdministrativeCountryStatus, ParsedCountryData,
};

const MAX_QUERY_RESULTS: usize = 500;
const MAX_SEARCH_QUERY_CHARS: usize = 200;

#[derive(Debug, Clone)]
pub struct AdministrativeCatalogRepository {
    db_path: String,
}

impl AdministrativeCatalogRepository {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed creating administrative catalog directory {}",
                    parent.display()
                )
            })?;
        }
        let repository = Self {
            db_path: path.to_string_lossy().into_owned(),
        };
        let connection = repository.open_connection()?;
        initialize_schema(&connection)?;
        Ok(repository)
    }

    pub async fn replace_country(
        &self,
        country: ParsedCountryData,
        fetched_at: String,
        source_url: String,
    ) -> Result<AdministrativeCountryStatus> {
        let country_code = normalize_country_code(&country.country_code)?;
        if country
            .areas
            .iter()
            .any(|area| area.country_code != country_code)
        {
            return Err(anyhow!(
                "parsed administrative area belongs to a different country"
            ));
        }
        let record_count =
            u64::try_from(country.areas.len()).context("administrative area count exceeds u64")?;
        let repository = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = repository.open_connection()?;
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let bbox = country.bbox;
            transaction.execute(
                "INSERT INTO administrative_catalog_countries (
                    country_code, state, fetched_at, source_updated_at, record_count,
                    source_url, last_error, min_lat, max_lat, min_lng, max_lng, updated_at
                 ) VALUES (?1, 'ready', ?2, ?3, ?4, ?5, NULL, ?6, ?7, ?8, ?9, ?2)
                 ON CONFLICT(country_code) DO UPDATE SET
                    state='ready', fetched_at=excluded.fetched_at,
                    source_updated_at=excluded.source_updated_at,
                    record_count=excluded.record_count, source_url=excluded.source_url,
                    last_error=NULL, min_lat=excluded.min_lat, max_lat=excluded.max_lat,
                    min_lng=excluded.min_lng, max_lng=excluded.max_lng,
                    updated_at=excluded.updated_at",
                params![
                    country_code,
                    fetched_at,
                    country.source_updated_at,
                    i64::try_from(record_count).context("record count exceeds SQLite integer")?,
                    source_url,
                    bbox.map(|value| value[0]),
                    bbox.map(|value| value[1]),
                    bbox.map(|value| value[2]),
                    bbox.map(|value| value[3]),
                ],
            )?;
            transaction.execute(
                "DELETE FROM administrative_catalog_areas WHERE country_code=?1",
                params![country_code],
            )?;
            transaction.execute(
                "DELETE FROM administrative_bounds_cache WHERE country_code=?1",
                params![country_code],
            )?;
            {
                let mut statement = transaction.prepare(
                    "INSERT INTO administrative_catalog_areas (
                        geoname_id, country_code, parent_id, name, ascii_name,
                        alternate_names, feature_code, administrative_level, kind,
                        latitude, longitude, population, modification_date,
                        canonical_path, search_text
                     ) VALUES (
                        ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
                        ?10, ?11, ?12, ?13, ?14, ?15
                     )",
                )?;
                for area in &country.areas {
                    statement.execute(params![
                        area.geoname_id,
                        area.country_code,
                        area.parent_id,
                        area.name,
                        area.ascii_name,
                        area.alternate_names,
                        area.feature_code,
                        area.administrative_level.map(i64::from),
                        area_kind_to_db(area.kind),
                        area.latitude,
                        area.longitude,
                        i64::try_from(area.population)
                            .context("administrative area population exceeds SQLite integer")?,
                        area.modification_date,
                        area.canonical_path,
                        search_text(
                            &area.name,
                            &area.ascii_name,
                            &area.alternate_names,
                            &area.canonical_path,
                        ),
                    ])?;
                }
            }
            let status = load_status(&transaction, &country_code)?.ok_or_else(|| {
                anyhow!("administrative catalog country status was not persisted")
            })?;
            transaction.commit()?;
            Ok(status)
        })
        .await
        .context("administrative catalog replacement task failed")?
    }

    pub async fn record_failure(
        &self,
        country_code: &str,
        message: &str,
    ) -> Result<AdministrativeCountryStatus> {
        let country_code = normalize_country_code(country_code)?;
        let message = message.trim().to_string();
        if message.is_empty() {
            return Err(anyhow!("administrative catalog failure message is empty"));
        }
        let repository = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = repository.open_connection()?;
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let now = chrono::Utc::now().to_rfc3339();
            let existing = load_status(&transaction, &country_code)?;
            if existing
                .as_ref()
                .is_some_and(|status| status.state == AdministrativeCatalogState::Ready)
            {
                transaction.execute(
                    "UPDATE administrative_catalog_countries
                     SET last_error=?1, updated_at=?2
                     WHERE country_code=?3",
                    params![message, now, country_code],
                )?;
            } else {
                transaction.execute(
                    "INSERT INTO administrative_catalog_countries (
                        country_code, state, fetched_at, source_updated_at, record_count,
                        source_url, last_error, min_lat, max_lat, min_lng, max_lng, updated_at
                     ) VALUES (?1, 'failed', NULL, NULL, 0, NULL, ?2, NULL, NULL, NULL, NULL, ?3)
                     ON CONFLICT(country_code) DO UPDATE SET
                        state='failed', record_count=0, last_error=excluded.last_error,
                        updated_at=excluded.updated_at",
                    params![country_code, message, now],
                )?;
            }
            let status = load_status(&transaction, &country_code)?.ok_or_else(|| {
                anyhow!("administrative catalog failure status was not persisted")
            })?;
            transaction.commit()?;
            Ok(status)
        })
        .await
        .context("administrative catalog failure task failed")?
    }

    pub async fn status(&self, country_code: &str) -> Result<AdministrativeCountryStatus> {
        let country_code = normalize_country_code(country_code)?;
        let repository = self.clone();
        tokio::task::spawn_blocking(move || {
            let connection = repository.open_connection()?;
            Ok(load_status(&connection, &country_code)?
                .unwrap_or_else(|| not_loaded(&country_code)))
        })
        .await
        .context("administrative catalog status task failed")?
    }

    pub async fn list_children(
        &self,
        country_code: &str,
        parent_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<AdministrativeAreaSummary>> {
        let country_code = normalize_country_code(country_code)?;
        validate_limit(limit)?;
        let repository = self.clone();
        tokio::task::spawn_blocking(move || {
            let connection = repository.open_connection()?;
            let mut statement = connection.prepare(&area_select_sql(
                "((?2 IS NULL AND area.parent_id IS NULL) OR area.parent_id=?2)",
            ))?;
            let rows = statement.query_map(
                params![country_code, parent_id, i64::try_from(limit)?],
                map_area_summary,
            )?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(Into::into)
        })
        .await
        .context("administrative catalog child-list task failed")?
    }

    pub async fn search(
        &self,
        country_code: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<AdministrativeAreaSummary>> {
        let country_code = normalize_country_code(country_code)?;
        validate_limit(limit)?;
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        if query.chars().count() > MAX_SEARCH_QUERY_CHARS {
            return Err(anyhow!(
                "administrative catalog query exceeds {MAX_SEARCH_QUERY_CHARS} characters"
            ));
        }
        let normalized_query = query.to_lowercase();
        let pattern = format!("%{}%", escape_like(&normalized_query));
        let repository = self.clone();
        tokio::task::spawn_blocking(move || {
            let connection = repository.open_connection()?;
            let mut statement =
                connection.prepare(&area_select_sql("area.search_text LIKE ?2 ESCAPE '\\'"))?;
            let rows = statement.query_map(
                params![
                    country_code,
                    pattern,
                    normalized_query,
                    i64::try_from(limit)?
                ],
                map_area_summary,
            )?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(Into::into)
        })
        .await
        .context("administrative catalog search task failed")?
    }

    pub async fn get_cached_bounds(&self, geoname_id: i64) -> Result<Option<AdministrativeBounds>> {
        let repository = self.clone();
        tokio::task::spawn_blocking(move || {
            let connection = repository.open_connection()?;
            connection
                .query_row(
                    "SELECT geoname_id, country_code, min_lat, max_lat, min_lng, max_lng, source, updated_at
                     FROM administrative_bounds_cache WHERE geoname_id=?1",
                    params![geoname_id],
                    |row| {
                        Ok(AdministrativeBounds {
                            geoname_id: row.get(0)?,
                            country_code: row.get(1)?,
                            min_lat: row.get(2)?,
                            max_lat: row.get(3)?,
                            min_lng: row.get(4)?,
                            max_lng: row.get(5)?,
                            source: row.get(6)?,
                            updated_at: row.get(7)?,
                        })
                    },
                )
                .optional()
                .map_err(Into::into)
        })
        .await
        .context("administrative bounds cache query task failed")?
    }

    pub async fn save_cached_bounds(&self, bounds: &AdministrativeBounds) -> Result<()> {
        let repository = self.clone();
        let bounds = bounds.clone();
        tokio::task::spawn_blocking(move || {
            let connection = repository.open_connection()?;
            connection.execute(
                "INSERT INTO administrative_bounds_cache (
                    geoname_id, country_code, min_lat, max_lat, min_lng, max_lng, source, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(geoname_id) DO UPDATE SET
                    country_code=excluded.country_code,
                    min_lat=excluded.min_lat,
                    max_lat=excluded.max_lat,
                    min_lng=excluded.min_lng,
                    max_lng=excluded.max_lng,
                    source=excluded.source,
                    updated_at=excluded.updated_at",
                params![
                    bounds.geoname_id,
                    bounds.country_code,
                    bounds.min_lat,
                    bounds.max_lat,
                    bounds.min_lng,
                    bounds.max_lng,
                    bounds.source,
                    bounds.updated_at,
                ],
            )?;
            Ok(())
        })
        .await
        .context("administrative bounds cache save task failed")?
    }

    pub async fn get_area_by_id(&self, geoname_id: i64) -> Result<Option<AdministrativeArea>> {
        let repository = self.clone();
        tokio::task::spawn_blocking(move || {
            let connection = repository.open_connection()?;
            connection
                .query_row(
                    "SELECT geoname_id, country_code, parent_id, name, ascii_name,
                            alternate_names, feature_code, administrative_level, kind,
                            latitude, longitude, population, modification_date, canonical_path
                     FROM administrative_catalog_areas WHERE geoname_id=?1",
                    params![geoname_id],
                    |row| {
                        let admin_level: Option<i64> = row.get(7)?;
                        let kind_str: String = row.get(8)?;
                        let pop_int: i64 = row.get(11)?;
                        Ok(AdministrativeArea {
                            geoname_id: row.get(0)?,
                            country_code: row.get(1)?,
                            parent_id: row.get(2)?,
                            name: row.get(3)?,
                            ascii_name: row.get(4)?,
                            alternate_names: row.get(5)?,
                            feature_code: row.get(6)?,
                            administrative_level: admin_level.map(|l| l as u8),
                            kind: area_kind_from_db(&kind_str)?,
                            latitude: row.get(9)?,
                            longitude: row.get(10)?,
                            population: pop_int.max(0) as u64,
                            modification_date: row.get(12)?,
                            canonical_path: row.get(13)?,
                        })
                    },
                )
                .optional()
                .map_err(Into::into)
        })
        .await
        .context("administrative area get task failed")?
    }

    pub async fn get_area_hierarchy(&self, geoname_id: i64) -> Result<Vec<AdministrativeArea>> {
        let mut current_id = Some(geoname_id);
        let mut chain = Vec::new();
        while let Some(id) = current_id {
            if let Some(area) = self.get_area_by_id(id).await? {
                current_id = area.parent_id;
                chain.push(area);
                if chain.len() > 10 {
                    break;
                }
            } else {
                break;
            }
        }
        chain.reverse();
        Ok(chain)
    }

    pub fn db_path(&self) -> &str {
        &self.db_path
    }

    fn open_connection(&self) -> Result<Connection> {
        let connection = Connection::open(&self.db_path).with_context(|| {
            format!("failed opening administrative catalog DB {}", self.db_path)
        })?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        Ok(connection)
    }
}

fn initialize_schema(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS administrative_catalog_countries (
            country_code TEXT PRIMARY KEY,
            state TEXT NOT NULL CHECK(state IN ('ready', 'failed')),
            fetched_at TEXT,
            source_updated_at TEXT,
            record_count INTEGER NOT NULL DEFAULT 0 CHECK(record_count >= 0),
            source_url TEXT,
            last_error TEXT,
            min_lat REAL,
            max_lat REAL,
            min_lng REAL,
            max_lng REAL,
            updated_at TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS administrative_catalog_areas (
            geoname_id INTEGER PRIMARY KEY,
            country_code TEXT NOT NULL,
            parent_id INTEGER,
            name TEXT NOT NULL,
            ascii_name TEXT NOT NULL,
            alternate_names TEXT NOT NULL,
            feature_code TEXT NOT NULL,
            administrative_level INTEGER CHECK(administrative_level BETWEEN 1 AND 5),
            kind TEXT NOT NULL CHECK(kind IN ('administrative', 'populated_place')),
            latitude REAL NOT NULL,
            longitude REAL NOT NULL,
            population INTEGER NOT NULL CHECK(population >= 0),
            modification_date TEXT NOT NULL,
            canonical_path TEXT NOT NULL,
            search_text TEXT NOT NULL,
            FOREIGN KEY(country_code) REFERENCES administrative_catalog_countries(country_code)
                ON DELETE CASCADE
         );
         CREATE TABLE IF NOT EXISTS administrative_bounds_cache (
            geoname_id INTEGER PRIMARY KEY,
            country_code TEXT NOT NULL,
            min_lat REAL NOT NULL,
            max_lat REAL NOT NULL,
            min_lng REAL NOT NULL,
            max_lng REAL NOT NULL,
            source TEXT NOT NULL,
            updated_at TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_administrative_catalog_parent
            ON administrative_catalog_areas(country_code, parent_id, kind, canonical_path);
         CREATE INDEX IF NOT EXISTS idx_administrative_catalog_country_path
            ON administrative_catalog_areas(country_code, canonical_path);
         CREATE INDEX IF NOT EXISTS idx_administrative_bounds_cache_country
            ON administrative_bounds_cache(country_code);",
    )?;
    Ok(())
}

fn normalize_country_code(country_code: &str) -> Result<String> {
    let country_code = country_code.trim().to_ascii_uppercase();
    if country_code.len() != 2 || !country_code.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return Err(anyhow!("country code must be a two-letter ISO code"));
    }
    Ok(country_code)
}

fn validate_limit(limit: usize) -> Result<()> {
    if !(1..=MAX_QUERY_RESULTS).contains(&limit) {
        return Err(anyhow!(
            "administrative catalog result limit must be between 1 and {MAX_QUERY_RESULTS}"
        ));
    }
    Ok(())
}

fn area_kind_to_db(kind: AdministrativeAreaKind) -> &'static str {
    match kind {
        AdministrativeAreaKind::Administrative => "administrative",
        AdministrativeAreaKind::PopulatedPlace => "populated_place",
    }
}

fn area_kind_from_db(value: &str) -> rusqlite::Result<AdministrativeAreaKind> {
    match value {
        "administrative" => Ok(AdministrativeAreaKind::Administrative),
        "populated_place" => Ok(AdministrativeAreaKind::PopulatedPlace),
        other => Err(rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unknown administrative area kind {other}"),
            )),
        )),
    }
}

fn catalog_state_from_db(value: &str) -> rusqlite::Result<AdministrativeCatalogState> {
    match value {
        "ready" => Ok(AdministrativeCatalogState::Ready),
        "failed" => Ok(AdministrativeCatalogState::Failed),
        other => Err(rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unknown administrative catalog state {other}"),
            )),
        )),
    }
}

fn load_status(
    connection: &Connection,
    country_code: &str,
) -> Result<Option<AdministrativeCountryStatus>> {
    connection
        .query_row(
            "SELECT country_code, state, fetched_at, source_updated_at, record_count,
                    source_url, last_error, min_lat, max_lat, min_lng, max_lng
             FROM administrative_catalog_countries WHERE country_code=?1",
            params![country_code],
            |row| {
                let min_lat: Option<f64> = row.get(7)?;
                let max_lat: Option<f64> = row.get(8)?;
                let min_lng: Option<f64> = row.get(9)?;
                let max_lng: Option<f64> = row.get(10)?;
                let bbox = match (min_lat, max_lat, min_lng, max_lng) {
                    (Some(min_lat), Some(max_lat), Some(min_lng), Some(max_lng)) => {
                        Some([min_lat, max_lat, min_lng, max_lng])
                    }
                    (None, None, None, None) => None,
                    _ => {
                        return Err(rusqlite::Error::FromSqlConversionFailure(
                            7,
                            rusqlite::types::Type::Real,
                            Box::new(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "partial administrative catalog bounding box",
                            )),
                        ))
                    }
                };
                let state: String = row.get(1)?;
                let record_count: i64 = row.get(4)?;
                Ok(AdministrativeCountryStatus {
                    country_code: row.get(0)?,
                    state: catalog_state_from_db(&state)?,
                    fetched_at: row.get(2)?,
                    source_updated_at: row.get(3)?,
                    record_count: u64::try_from(record_count).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            4,
                            rusqlite::types::Type::Integer,
                            Box::new(error),
                        )
                    })?,
                    source_url: row.get(5)?,
                    last_error: row.get(6)?,
                    bbox,
                })
            },
        )
        .optional()
        .map_err(Into::into)
}

fn not_loaded(country_code: &str) -> AdministrativeCountryStatus {
    AdministrativeCountryStatus {
        country_code: country_code.to_string(),
        state: AdministrativeCatalogState::NotLoaded,
        fetched_at: None,
        source_updated_at: None,
        record_count: 0,
        source_url: None,
        last_error: None,
        bbox: None,
    }
}

fn search_text(name: &str, ascii_name: &str, alternate_names: &str, path: &str) -> String {
    format!("{name}\n{ascii_name}\n{alternate_names}\n{path}").to_lowercase()
}

fn escape_like(query: &str) -> String {
    query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn area_select_sql(predicate: &str) -> String {
    let order = if predicate.contains("search_text") {
        "CASE WHEN lower(area.name)=?3 THEN 0 ELSE 1 END,
         CASE area.kind WHEN 'administrative' THEN 0 ELSE 1 END,
         area.population DESC, area.canonical_path, area.geoname_id
         LIMIT ?4"
    } else {
        "CASE area.kind WHEN 'administrative' THEN 0 ELSE 1 END,
         area.administrative_level, area.population DESC,
         area.canonical_path, area.geoname_id
         LIMIT ?3"
    };
    format!(
        "SELECT area.geoname_id, area.country_code, area.parent_id, area.name,
                area.ascii_name, area.feature_code, area.administrative_level,
                area.kind, area.latitude, area.longitude, area.population,
                area.canonical_path,
                EXISTS(
                    SELECT 1 FROM administrative_catalog_areas child
                    WHERE child.country_code=area.country_code
                      AND child.parent_id=area.geoname_id
                ) AS has_children
         FROM administrative_catalog_areas area
         WHERE area.country_code=?1 AND {predicate}
         ORDER BY {order}"
    )
}

fn map_area_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<AdministrativeAreaSummary> {
    let kind: String = row.get(7)?;
    let population: i64 = row.get(10)?;
    Ok(AdministrativeAreaSummary {
        geoname_id: row.get(0)?,
        country_code: row.get(1)?,
        parent_id: row.get(2)?,
        name: row.get(3)?,
        ascii_name: row.get(4)?,
        feature_code: row.get(5)?,
        administrative_level: row
            .get::<_, Option<i64>>(6)?
            .map(u8::try_from)
            .transpose()
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    6,
                    rusqlite::types::Type::Integer,
                    Box::new(error),
                )
            })?,
        kind: area_kind_from_db(&kind)?,
        latitude: row.get(8)?,
        longitude: row.get(9)?,
        population: u64::try_from(population).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                10,
                rusqlite::types::Type::Integer,
                Box::new(error),
            )
        })?,
        canonical_path: row.get(11)?,
        has_children: row.get(12)?,
    })
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::AdministrativeCatalogRepository;
    use crate::administrative_catalog::types::{
        AdministrativeArea, AdministrativeAreaKind, AdministrativeCatalogState, ParsedCountryData,
    };

    fn fixture() -> ParsedCountryData {
        ParsedCountryData {
            country_code: "VN".to_string(),
            source_updated_at: Some("2026-08-11".to_string()),
            bbox: Some([10.0, 12.0, 20.0, 22.0]),
            areas: vec![
                area(
                    100,
                    None,
                    "Alpha Province",
                    "Province Alpha",
                    "ADM1",
                    Some(1),
                    AdministrativeAreaKind::Administrative,
                    "Alpha Province, Vietnam",
                ),
                area(
                    200,
                    Some(100),
                    "Beta District",
                    "District Beta",
                    "ADM2",
                    Some(2),
                    AdministrativeAreaKind::Administrative,
                    "Beta District, Alpha Province, Vietnam",
                ),
                area(
                    300,
                    Some(200),
                    "Zeta City",
                    "City Zeta,Zeta",
                    "PPLA2",
                    None,
                    AdministrativeAreaKind::PopulatedPlace,
                    "Zeta City, Beta District, Alpha Province, Vietnam",
                ),
            ],
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn area(
        geoname_id: i64,
        parent_id: Option<i64>,
        name: &str,
        alternate_names: &str,
        feature_code: &str,
        administrative_level: Option<u8>,
        kind: AdministrativeAreaKind,
        canonical_path: &str,
    ) -> AdministrativeArea {
        AdministrativeArea {
            geoname_id,
            country_code: "VN".to_string(),
            parent_id,
            name: name.to_string(),
            ascii_name: name.to_string(),
            alternate_names: alternate_names.to_string(),
            feature_code: feature_code.to_string(),
            administrative_level,
            kind,
            latitude: 10.0,
            longitude: 20.0,
            population: if geoname_id == 300 { 5_000 } else { 0 },
            modification_date: "2026-08-11".to_string(),
            canonical_path: canonical_path.to_string(),
        }
    }

    fn repository() -> (TempDir, AdministrativeCatalogRepository) {
        let temp = tempfile::tempdir().expect("temp dir should exist");
        let repository = AdministrativeCatalogRepository::new(temp.path().join("catalog.db"))
            .expect("repository should initialize");
        (temp, repository)
    }

    #[tokio::test]
    async fn replaces_country_transactionally_and_exposes_hierarchy() {
        let (_temp, repository) = repository();

        let status = repository
            .replace_country(
                fixture(),
                "2026-08-12T06:00:00Z".to_string(),
                "https://download.geonames.org/export/dump/VN.zip".to_string(),
            )
            .await
            .expect("country replacement should succeed");

        assert_eq!(status.state, AdministrativeCatalogState::Ready);
        assert_eq!(status.record_count, 3);
        assert_eq!(status.bbox, Some([10.0, 12.0, 20.0, 22.0]));
        let roots = repository
            .list_children("VN", None, 500)
            .await
            .expect("root areas should load");
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].geoname_id, 100);
        assert!(roots[0].has_children);
        let children = repository
            .list_children("VN", Some(100), 500)
            .await
            .expect("children should load");
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].geoname_id, 200);
    }

    #[tokio::test]
    async fn searches_names_aliases_and_canonical_paths() {
        let (_temp, repository) = repository();
        repository
            .replace_country(
                fixture(),
                "2026-08-12T06:00:00Z".to_string(),
                "source".to_string(),
            )
            .await
            .expect("country replacement should succeed");

        for query in ["Zeta", "City Zeta", "Alpha Province"] {
            let results = repository
                .search("VN", query, 50)
                .await
                .expect("search should succeed");
            assert!(
                results.iter().any(|area| area.geoname_id == 300),
                "query {query} did not find the city"
            );
        }
    }

    #[tokio::test]
    async fn failed_refresh_preserves_the_last_ready_snapshot() {
        let (_temp, repository) = repository();
        repository
            .replace_country(
                fixture(),
                "2026-08-12T06:00:00Z".to_string(),
                "source".to_string(),
            )
            .await
            .expect("country replacement should succeed");

        let status = repository
            .record_failure("VN", "network unavailable")
            .await
            .expect("failure should be recorded");

        assert_eq!(status.state, AdministrativeCatalogState::Ready);
        assert_eq!(status.record_count, 3);
        assert_eq!(status.last_error.as_deref(), Some("network unavailable"));
        assert_eq!(
            repository
                .list_children("VN", None, 500)
                .await
                .expect("old snapshot should remain")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn initial_failure_is_visible_as_failed() {
        let (_temp, repository) = repository();

        let status = repository
            .record_failure("KR", "network unavailable")
            .await
            .expect("failure should be recorded");

        assert_eq!(status.state, AdministrativeCatalogState::Failed);
        assert_eq!(status.record_count, 0);
    }
}
