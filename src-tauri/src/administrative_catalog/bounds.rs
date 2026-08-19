use std::time::Duration;
use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::Deserialize;

use super::repository::AdministrativeCatalogRepository;
use super::types::{AdministrativeArea, AdministrativeBounds};

#[derive(Debug, Clone)]
pub struct GeoBoundsResolver {
    repository: AdministrativeCatalogRepository,
    http_client: Client,
    base_url: String,
}

#[derive(Debug, Deserialize)]
struct NominatimSearchResult {
    boundingbox: Vec<String>,
}

impl GeoBoundsResolver {
    pub fn new(repository: AdministrativeCatalogRepository) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(6))
            .user_agent("RustSale-CRM-v2-Client (support@rustsale.trantrongtri.com)")
            .build()
            .unwrap_or_default();
        let base_url = std::env::var("RUSTSALE_NOMINATIM_BASE_URL")
            .unwrap_or_else(|_| "https://nominatim.openstreetmap.org".to_string());
        Self {
            repository,
            http_client,
            base_url,
        }
    }

    pub fn with_client(repository: AdministrativeCatalogRepository, http_client: Client) -> Self {
        let base_url = std::env::var("RUSTSALE_NOMINATIM_BASE_URL")
            .unwrap_or_else(|_| "https://nominatim.openstreetmap.org".to_string());
        Self {
            repository,
            http_client,
            base_url,
        }
    }

    pub fn with_base_url(
        repository: AdministrativeCatalogRepository,
        http_client: Client,
        base_url: String,
    ) -> Self {
        Self {
            repository,
            http_client,
            base_url,
        }
    }

    pub async fn resolve_bounds(&self, geoname_id: i64) -> Result<AdministrativeBounds> {
        // 1. Check cache first (only authoritative entries are cached)
        if let Some(cached) = self.repository.get_cached_bounds(geoname_id).await? {
            return Ok(cached);
        }

        // 2. Fetch area details
        let area = self
            .repository
            .get_area_by_id(geoname_id)
            .await?
            .ok_or_else(|| anyhow!("administrative area not found for geoname_id {geoname_id}"))?;

        // 3. Try structured geocoding via OpenStreetMap Nominatim
        let geocoded = self.try_structured_geocoding(&area).await;
        match geocoded {
            Ok(Some(bbox)) => {
                let bounds = AdministrativeBounds {
                    geoname_id,
                    country_code: area.country_code.clone(),
                    min_lat: bbox[0],
                    max_lat: bbox[1],
                    min_lng: bbox[2],
                    max_lng: bbox[3],
                    source: "nominatim".to_string(),
                    updated_at: chrono::Utc::now().to_rfc3339(),
                };
                // Persist only authoritative Nominatim results to SQLite cache
                if let Err(err) = self.repository.save_cached_bounds(&bounds).await {
                    eprintln!("Warning: failed saving administrative bounds to cache: {err}");
                }
                Ok(bounds)
            }
            _ => {
                // 4. Fallback to centroid radius in-memory without permanently poisoning the cache
                let bbox = compute_centroid_bounds(
                    area.latitude,
                    area.longitude,
                    area.administrative_level,
                );
                Ok(AdministrativeBounds {
                    geoname_id,
                    country_code: area.country_code.clone(),
                    min_lat: bbox[0],
                    max_lat: bbox[1],
                    min_lng: bbox[2],
                    max_lng: bbox[3],
                    source: "centroid_radius".to_string(),
                    updated_at: chrono::Utc::now().to_rfc3339(),
                })
            }
        }
    }

    async fn try_structured_geocoding(&self, area: &AdministrativeArea) -> Result<Option<[f64; 4]>> {
        let chain = self.repository.get_area_hierarchy(area.geoname_id).await?;
        let country_code = area.country_code.to_lowercase();
        
        // Build fully disambiguated query from hierarchy (e.g. "Phường Bến Nghé, Quận 1, Thành phố Hồ Chí Minh")
        let query_parts: Vec<&str> = chain
            .iter()
            .map(|a| a.name.as_str())
            .filter(|name| !name.is_empty())
            .collect();

        let query_str = if query_parts.is_empty() {
            area.name.clone()
        } else {
            query_parts.join(", ")
        };

        let url = format!(
            "{}/search?q={}&countrycodes={}&format=json&limit=1",
            self.base_url.trim_end_matches('/'),
            urlencoding::encode(&query_str),
            country_code
        );

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .context("failed sending geocoding request")?;

        if !response.status().is_success() {
            return Err(anyhow!("nominatim returned error status: {}", response.status()));
        }

        let results: Vec<NominatimSearchResult> = response.json().await.context("failed parsing nominatim json")?;
        if let Some(first) = results.into_iter().next() {
            if first.boundingbox.len() >= 4 {
                let min_lat = first.boundingbox[0].parse::<f64>()?;
                let max_lat = first.boundingbox[1].parse::<f64>()?;
                let min_lng = first.boundingbox[2].parse::<f64>()?;
                let max_lng = first.boundingbox[3].parse::<f64>()?;
                
                if min_lat < max_lat && min_lng < max_lng {
                    // Validate that catalog centroid is within or reasonably close to returned bbox
                    let lat_margin = 0.5;
                    let lng_margin = 0.5;
                    if area.latitude >= min_lat - lat_margin
                        && area.latitude <= max_lat + lat_margin
                        && area.longitude >= min_lng - lng_margin
                        && area.longitude <= max_lng + lng_margin
                    {
                        return Ok(Some([min_lat, max_lat, min_lng, max_lng]));
                    }
                }
            }
        }

        Ok(None)
    }

    pub async fn geocode_query(&self, query: &str, country_code: Option<&str>) -> Result<Option<[f64; 4]>> {
        let mut url = format!(
            "{}/search?q={}&format=json&limit=1",
            self.base_url.trim_end_matches('/'),
            urlencoding::encode(query)
        );
        if let Some(cc) = country_code {
            if !cc.trim().is_empty() {
                url.push_str(&format!("&countrycodes={}", urlencoding::encode(&cc.trim().to_lowercase())));
            }
        }

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .context("failed sending geocoding request")?;

        if !response.status().is_success() {
            return Err(anyhow!("geocoding service returned status: {}", response.status()));
        }

        let results: Vec<NominatimSearchResult> = response.json().await.context("failed parsing geocoding json")?;
        if let Some(first) = results.into_iter().next() {
            if first.boundingbox.len() >= 4 {
                let min_lat = first.boundingbox[0].parse::<f64>()?;
                let max_lat = first.boundingbox[1].parse::<f64>()?;
                let min_lng = first.boundingbox[2].parse::<f64>()?;
                let max_lng = first.boundingbox[3].parse::<f64>()?;
                if min_lat < max_lat && min_lng < max_lng {
                    return Ok(Some([min_lat, max_lat, min_lng, max_lng]));
                }
            }
        }

        Ok(None)
    }
}

pub fn compute_centroid_bounds(
    lat: f64,
    lng: f64,
    admin_level: Option<u8>,
) -> [f64; 4] {
    // Radius deltas in degrees based on administrative level
    let delta_lat = match admin_level {
        Some(1) => 0.35,  // ~40 km (Province/State)
        Some(2) => 0.10,  // ~11 km (District/County)
        Some(3) => 0.04,  // ~4.5 km (Ward/Commune)
        Some(4) | Some(5) => 0.025, // ~2.8 km (Village/Neighborhood)
        _ => 0.05,       // ~5.5 km default
    };

    let cos_lat = (lat * std::f64::consts::PI / 180.0).cos().abs().max(0.1);
    let delta_lng = delta_lat / cos_lat;

    let min_lat = (lat - delta_lat).max(-90.0);
    let max_lat = (lat + delta_lat).min(90.0);
    let min_lng = (lng - delta_lng).max(-180.0);
    let max_lng = (lng + delta_lng).min(180.0);

    [min_lat, max_lat, min_lng, max_lng]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::administrative_catalog::types::{AdministrativeAreaKind, ParsedCountryData};

    #[tokio::test]
    async fn test_centroid_bounds_computation() {
        let bbox = compute_centroid_bounds(10.7769, 106.7009, Some(2));
        assert!(bbox[0] < bbox[1]);
        assert!(bbox[2] < bbox[3]);
        assert!(bbox[0] <= 10.7769 && bbox[1] >= 10.7769);
        assert!(bbox[2] <= 106.7009 && bbox[3] >= 106.7009);
    }

    #[tokio::test]
    async fn test_bounds_resolver_returns_cached_authoritative_bounds() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("admin_test.db");
        let repo = AdministrativeCatalogRepository::new(&db_path).unwrap();

        let parsed = ParsedCountryData {
            country_code: "VN".to_string(),
            areas: vec![AdministrativeArea {
                geoname_id: 1566083,
                country_code: "VN".to_string(),
                parent_id: None,
                name: "Thành phố Hồ Chí Minh".to_string(),
                ascii_name: "Ho Chi Minh City".to_string(),
                alternate_names: "".to_string(),
                feature_code: "ADM1".to_string(),
                administrative_level: Some(1),
                kind: AdministrativeAreaKind::Administrative,
                latitude: 10.7769,
                longitude: 106.7009,
                population: 9000000,
                modification_date: "2026-01-01".to_string(),
                canonical_path: "Vietnam / Ho Chi Minh City".to_string(),
            }],
            source_updated_at: None,
            bbox: Some([8.5, 23.4, 102.1, 109.5]),
        };

        repo.replace_country(parsed, "2026-08-19T00:00:00Z".to_string(), "local".to_string())
            .await
            .unwrap();

        // Pre-cache an authoritative bounding box
        let sample_bounds = AdministrativeBounds {
            geoname_id: 1566083,
            country_code: "VN".to_string(),
            min_lat: 10.35,
            max_lat: 11.15,
            min_lng: 106.35,
            max_lng: 107.05,
            source: "nominatim".to_string(),
            updated_at: "2026-08-19T00:00:00Z".to_string(),
        };
        repo.save_cached_bounds(&sample_bounds).await.unwrap();

        let resolver = GeoBoundsResolver::new(repo.clone());
        let bounds = resolver.resolve_bounds(1566083).await.unwrap();

        assert_eq!(bounds.geoname_id, 1566083);
        assert_eq!(bounds.source, "nominatim");
        assert_eq!(bounds.min_lat, 10.35);
        assert_eq!(bounds.max_lat, 11.15);
    }

    #[tokio::test]
    async fn test_bounds_resolver_falls_back_to_centroid_without_poisoning_cache() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("admin_test.db");
        let repo = AdministrativeCatalogRepository::new(&db_path).unwrap();

        let parsed = ParsedCountryData {
            country_code: "VN".to_string(),
            areas: vec![AdministrativeArea {
                geoname_id: 999999,
                country_code: "VN".to_string(),
                parent_id: None,
                name: "Unresolvable Area".to_string(),
                ascii_name: "Unresolvable Area".to_string(),
                alternate_names: "".to_string(),
                feature_code: "ADM2".to_string(),
                administrative_level: Some(2),
                kind: AdministrativeAreaKind::Administrative,
                latitude: 10.5,
                longitude: 106.5,
                population: 1000,
                modification_date: "2026-01-01".to_string(),
                canonical_path: "Vietnam / Unresolvable Area".to_string(),
            }],
            source_updated_at: None,
            bbox: Some([8.5, 23.4, 102.1, 109.5]),
        };

        repo.replace_country(parsed.clone(), "2026-08-19T00:00:00Z".to_string(), "local".to_string())
            .await
            .unwrap();

        // Pass invalid localhost URL so network request fails immediately offline
        let resolver = GeoBoundsResolver::with_base_url(
            repo.clone(),
            Client::new(),
            "http://127.0.0.1:9".to_string(),
        );

        let bounds = resolver.resolve_bounds(999999).await.unwrap();
        assert_eq!(bounds.source, "centroid_radius");
        assert!(bounds.min_lat < bounds.max_lat);

        // Verify fallback was NOT saved to cache to avoid poisoning
        let cached = repo.get_cached_bounds(999999).await.unwrap();
        assert!(cached.is_none());
    }

    #[tokio::test]
    async fn test_bounds_resolver_invalidates_cache_on_country_replace() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("admin_test.db");
        let repo = AdministrativeCatalogRepository::new(&db_path).unwrap();

        let parsed = ParsedCountryData {
            country_code: "VN".to_string(),
            areas: vec![AdministrativeArea {
                geoname_id: 1566083,
                country_code: "VN".to_string(),
                parent_id: None,
                name: "Thành phố Hồ Chí Minh".to_string(),
                ascii_name: "Ho Chi Minh City".to_string(),
                alternate_names: "".to_string(),
                feature_code: "ADM1".to_string(),
                administrative_level: Some(1),
                kind: AdministrativeAreaKind::Administrative,
                latitude: 10.7769,
                longitude: 106.7009,
                population: 9000000,
                modification_date: "2026-01-01".to_string(),
                canonical_path: "Vietnam / Ho Chi Minh City".to_string(),
            }],
            source_updated_at: None,
            bbox: Some([8.5, 23.4, 102.1, 109.5]),
        };

        repo.replace_country(parsed.clone(), "2026-08-19T00:00:00Z".to_string(), "local".to_string())
            .await
            .unwrap();

        repo.save_cached_bounds(&AdministrativeBounds {
            geoname_id: 1566083,
            country_code: "VN".to_string(),
            min_lat: 10.35,
            max_lat: 11.15,
            min_lng: 106.35,
            max_lng: 107.05,
            source: "nominatim".to_string(),
            updated_at: "2026-08-19T00:00:00Z".to_string(),
        })
        .await
        .unwrap();

        assert!(repo.get_cached_bounds(1566083).await.unwrap().is_some());

        // Re-replacing the country must invalidate the cached bounds
        repo.replace_country(parsed, "2026-08-20T00:00:00Z".to_string(), "local".to_string())
            .await
            .unwrap();

        assert!(repo.get_cached_bounds(1566083).await.unwrap().is_none());
    }
}
