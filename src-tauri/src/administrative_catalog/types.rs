use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdministrativeCountry {
    pub code: String,
    pub iso3: String,
    pub name: String,
    pub capital: String,
    pub continent_code: String,
    pub languages: Vec<String>,
    pub geoname_id: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdministrativeAreaKind {
    Administrative,
    PopulatedPlace,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdministrativeArea {
    pub geoname_id: i64,
    pub country_code: String,
    pub parent_id: Option<i64>,
    pub name: String,
    pub ascii_name: String,
    pub alternate_names: String,
    pub feature_code: String,
    pub administrative_level: Option<u8>,
    pub kind: AdministrativeAreaKind,
    pub latitude: f64,
    pub longitude: f64,
    pub population: u64,
    pub modification_date: String,
    pub canonical_path: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedCountryData {
    pub country_code: String,
    pub areas: Vec<AdministrativeArea>,
    pub source_updated_at: Option<String>,
    pub bbox: Option<[f64; 4]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdministrativeCatalogState {
    NotLoaded,
    Ready,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdministrativeCountryStatus {
    pub country_code: String,
    pub state: AdministrativeCatalogState,
    pub fetched_at: Option<String>,
    pub source_updated_at: Option<String>,
    pub record_count: u64,
    pub source_url: Option<String>,
    pub last_error: Option<String>,
    pub bbox: Option<[f64; 4]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdministrativeAreaSummary {
    pub geoname_id: i64,
    pub country_code: String,
    pub parent_id: Option<i64>,
    pub name: String,
    pub ascii_name: String,
    pub feature_code: String,
    pub administrative_level: Option<u8>,
    pub kind: AdministrativeAreaKind,
    pub latitude: f64,
    pub longitude: f64,
    pub population: u64,
    pub canonical_path: String,
    pub has_children: bool,
}
