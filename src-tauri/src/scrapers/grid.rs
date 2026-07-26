use crate::google_maps_jobs::validation::{validate_and_estimate_grid, ScrapeError};
use crate::scrapers::google_maps::{BusinessInfo, GoogleMapsScraper};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox {
    pub lat_min: f64,
    pub lat_max: f64,
    pub lng_min: f64,
    pub lng_max: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GridCell {
    pub bbox: BoundingBox,
    pub center_lat: f64,
    pub center_lng: f64,
    pub step_lat_deg: f64,
    pub step_lng_deg: f64,
    pub radius_m: f64,
    pub depth: u8,
    pub parent_unit_id: Option<String>,
    pub consecutive_low_yield_levels: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GridLimits {
    pub max_depth: u8,
    pub min_cell_size_m: f64,
    pub max_initial_cells: usize,
    pub max_total_cells: usize,
}

impl Default for GridLimits {
    fn default() -> Self {
        Self {
            max_depth: 6,
            min_cell_size_m: 20.0,
            max_initial_cells: 10_000,
            max_total_cells: 25_000,
        }
    }
}

pub fn radius_to_zoom(radius_m: f64) -> u8 {
    if radius_m >= 10000.0 {
        12
    } else if radius_m >= 5000.0 {
        13
    } else if radius_m >= 2500.0 {
        14
    } else if radius_m >= 1200.0 {
        15
    } else if radius_m >= 600.0 {
        16
    } else if radius_m >= 300.0 {
        17
    } else {
        18
    }
}

pub fn generate_grid(bbox: BoundingBox, step_km: f64) -> Vec<GridCell> {
    generate_initial_cells(bbox, step_km, GridLimits::default()).unwrap_or_default()
}

pub fn generate_initial_cells(
    bbox: BoundingBox,
    step_km: f64,
    limits: GridLimits,
) -> std::result::Result<Vec<GridCell>, ScrapeError> {
    let estimate = validate_and_estimate_grid(bbox, step_km, limits)?;
    if estimate.initial_cells > limits.max_initial_cells {
        let recommended = estimate.recommended_min_step_km.unwrap_or(step_km);
        return Err(ScrapeError {
            kind: crate::google_maps_jobs::types::ScrapeErrorKind::InvalidInput,
            message: format!(
                "grid requires {} initial cells, exceeding cap {}; use step_km >= {:.3}",
                estimate.initial_cells, limits.max_initial_cells, recommended
            ),
            retryable: false,
        });
    }

    let lat_step_deg = step_km / 111.0;
    let rows = ((bbox.lat_max - bbox.lat_min) / lat_step_deg)
        .ceil()
        .max(1.0) as usize;
    let mut cells = Vec::with_capacity(estimate.initial_cells);

    for row in 0..rows {
        let lat_min = bbox.lat_min + row as f64 * lat_step_deg;
        if lat_min >= bbox.lat_max {
            break;
        }
        let lat_max = (lat_min + lat_step_deg).min(bbox.lat_max);
        let center_lat = (lat_min + lat_max) / 2.0;
        let cos_lat = center_lat.to_radians().cos().abs();
        let lng_step_deg = if cos_lat > 1e-9 {
            step_km / (111.0 * cos_lat)
        } else {
            bbox.lng_max - bbox.lng_min
        };
        let columns = ((bbox.lng_max - bbox.lng_min) / lng_step_deg)
            .ceil()
            .max(1.0) as usize;

        for column in 0..columns {
            if cells.len() >= limits.max_initial_cells {
                return Err(ScrapeError {
                    kind: crate::google_maps_jobs::types::ScrapeErrorKind::InvalidInput,
                    message: "generated grid exceeded the validated initial cell cap".to_string(),
                    retryable: false,
                });
            }
            let lng_min = bbox.lng_min + column as f64 * lng_step_deg;
            if lng_min >= bbox.lng_max {
                break;
            }
            let lng_max = (lng_min + lng_step_deg).min(bbox.lng_max);
            cells.push(grid_cell_from_bbox(
                BoundingBox {
                    lat_min,
                    lat_max,
                    lng_min,
                    lng_max,
                },
                0,
                None,
                0,
            ));
        }
    }

    Ok(cells)
}

pub fn subdivide_cell(cell: &GridCell, new_unique_ratio: f64) -> Vec<GridCell> {
    let lat_mid = (cell.bbox.lat_min + cell.bbox.lat_max) / 2.0;
    let lng_mid = (cell.bbox.lng_min + cell.bbox.lng_max) / 2.0;
    let low_yield_levels = if new_unique_ratio.is_finite() && new_unique_ratio < 0.05 {
        cell.consecutive_low_yield_levels.saturating_add(1)
    } else {
        0
    };
    let depth = cell.depth.saturating_add(1);

    [
        BoundingBox {
            lat_min: lat_mid,
            lat_max: cell.bbox.lat_max,
            lng_min: cell.bbox.lng_min,
            lng_max: lng_mid,
        },
        BoundingBox {
            lat_min: lat_mid,
            lat_max: cell.bbox.lat_max,
            lng_min: lng_mid,
            lng_max: cell.bbox.lng_max,
        },
        BoundingBox {
            lat_min: cell.bbox.lat_min,
            lat_max: lat_mid,
            lng_min: cell.bbox.lng_min,
            lng_max: lng_mid,
        },
        BoundingBox {
            lat_min: cell.bbox.lat_min,
            lat_max: lat_mid,
            lng_min: lng_mid,
            lng_max: cell.bbox.lng_max,
        },
    ]
    .into_iter()
    .map(|bbox| grid_cell_from_bbox(bbox, depth, cell.parent_unit_id.clone(), low_yield_levels))
    .collect()
}

pub fn should_subdivide(
    cell: &GridCell,
    raw_listing_count: usize,
    saturation_threshold: usize,
    reached_end: bool,
    new_unique_ratio: f64,
    total_cells: usize,
    limits: GridLimits,
) -> bool {
    if reached_end && raw_listing_count < saturation_threshold {
        return false;
    }
    if cell.depth >= limits.max_depth || total_cells.saturating_add(4) > limits.max_total_cells {
        return false;
    }
    if cell_min_dimension_m(cell) <= limits.min_cell_size_m {
        return false;
    }
    let low_yield_levels = if new_unique_ratio.is_finite() && new_unique_ratio < 0.05 {
        cell.consecutive_low_yield_levels.saturating_add(1)
    } else {
        0
    };
    if low_yield_levels >= 2 {
        return false;
    }
    let children = subdivide_cell(cell, new_unique_ratio);
    children
        .first()
        .is_some_and(|child| radius_to_zoom(child.radius_m) > radius_to_zoom(cell.radius_m))
}

fn grid_cell_from_bbox(
    bbox: BoundingBox,
    depth: u8,
    parent_unit_id: Option<String>,
    consecutive_low_yield_levels: u8,
) -> GridCell {
    let center_lat = (bbox.lat_min + bbox.lat_max) / 2.0;
    let center_lng = (bbox.lng_min + bbox.lng_max) / 2.0;
    let step_lat_deg = bbox.lat_max - bbox.lat_min;
    let step_lng_deg = bbox.lng_max - bbox.lng_min;
    let height_m = step_lat_deg * 111_000.0;
    let width_m = step_lng_deg * 111_000.0 * center_lat.to_radians().cos().abs();
    let radius_m = (height_m / 2.0).hypot(width_m / 2.0);
    GridCell {
        bbox,
        center_lat,
        center_lng,
        step_lat_deg,
        step_lng_deg,
        radius_m,
        depth,
        parent_unit_id,
        consecutive_low_yield_levels,
    }
}

fn cell_min_dimension_m(cell: &GridCell) -> f64 {
    let height_m = cell.step_lat_deg * 111_000.0;
    let width_m = cell.step_lng_deg * 111_000.0 * cell.center_lat.to_radians().cos().abs();
    height_m.min(width_m)
}

impl GridCell {
    pub fn subdivide(&self) -> Vec<GridCell> {
        subdivide_cell(self, 1.0)
    }
}

pub struct GridScraper {
    pub scraper: GoogleMapsScraper,
    pub saturation_threshold: usize,
}

fn normalize_string(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

impl GridScraper {
    pub fn new(scraper: GoogleMapsScraper) -> Self {
        Self {
            scraper,
            saturation_threshold: 55,
        }
    }

    pub async fn search_grid<F>(
        &self,
        keyword: &str,
        initial_cells: Vec<GridCell>,
        mut on_progress: F,
    ) -> Result<Vec<BusinessInfo>>
    where
        F: FnMut(&GridCell, usize, usize, &str) + Send,
    {
        let mut stack = initial_cells;
        let mut unique_results = std::collections::HashMap::new();

        let mut processed_count = 0;
        let mut total_count = stack.len();

        while let Some(cell) = stack.pop() {
            processed_count += 1;
            let zoom = radius_to_zoom(cell.radius_m);

            let msg = format!(
                "Scraping cell at lat: {:.4}, lng: {:.4}, zoom: {}, depth: {}",
                cell.center_lat, cell.center_lng, zoom, cell.depth
            );
            on_progress(&cell, processed_count, total_count, &msg);

            match self
                .scraper
                .search_region(keyword, cell.center_lat, cell.center_lng, zoom)
                .await
            {
                Ok(results) => {
                    let count = results.len();

                    for item in results {
                        let norm_name = normalize_string(&item.name);
                        let norm_addr = normalize_string(item.address.as_deref().unwrap_or(""));
                        let key = format!("{}_{}", norm_name, norm_addr);
                        unique_results.insert(key, item);
                    }

                    if should_subdivide(
                        &cell,
                        count,
                        self.saturation_threshold,
                        count < self.saturation_threshold,
                        1.0,
                        total_count,
                        GridLimits::default(),
                    ) {
                        let children = cell.subdivide();
                        total_count += children.len();
                        stack.extend(children);

                        let subdivide_msg =
                            format!("Cell saturated ({} results). Subdivided.", count);
                        on_progress(&cell, processed_count, total_count, &subdivide_msg);
                    } else if count >= self.saturation_threshold {
                        let stop_msg = format!(
                            "Cell saturated ({} results) but reached a grid safety limit. Stopped subdivision.",
                            count
                        );
                        on_progress(&cell, processed_count, total_count, &stop_msg);
                    }
                }
                Err(e) => {
                    let err_msg = format!("Failed to scrape cell: {}", e);
                    on_progress(&cell, processed_count, total_count, &err_msg);
                    let err_s = e.to_string().to_lowercase();
                    if err_s.contains("connection lost")
                        || err_s.contains("disconnected")
                        || err_s.contains("closed")
                        || err_s.contains("timed out")
                    {
                        return Err(e);
                    }
                }
            }
        }

        Ok(unique_results.into_values().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scrapers::browser::BrowserManager;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    async fn spawn_mock_server() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = [0; 4096];
                    if let Ok(n) = socket.read(&mut buf).await {
                        let request = String::from_utf8_lossy(&buf[..n]);

                        let response = if request.contains("/search") {
                            // If it is the initial cell (center 10.1, 106.1, zoom 13), return 5 links to trigger subdivision
                            let is_initial = request.contains("@10.1,106.1,13z");
                            let count = if is_initial { 5 } else { 2 };
                            let mut offset = 40;
                            if is_initial {
                                offset = 0;
                            } else {
                                if let Some(at_idx) = request.find('@') {
                                    let parts: Vec<&str> =
                                        request[at_idx + 1..].split(',').collect();
                                    if parts.len() >= 2 {
                                        if let (Ok(lat), Ok(lng)) =
                                            (parts[0].parse::<f64>(), parts[1].parse::<f64>())
                                        {
                                            let lat_rounded = (lat * 1000.0).round() / 1000.0;
                                            let lng_rounded = (lng * 1000.0).round() / 1000.0;
                                            if (lat_rounded - 10.125).abs() < 0.001
                                                && (lng_rounded - 106.075).abs() < 0.001
                                            {
                                                offset = 10;
                                            } else if (lat_rounded - 10.125).abs() < 0.001
                                                && (lng_rounded - 106.125).abs() < 0.001
                                            {
                                                offset = 20;
                                            } else if (lat_rounded - 10.075).abs() < 0.001
                                                && (lng_rounded - 106.075).abs() < 0.001
                                            {
                                                offset = 30;
                                            }
                                        }
                                    }
                                }
                            }
                            let mut links = String::new();
                            for i in 1..=count {
                                let cafe_id = offset + i;
                                links.push_str(&format!(
                                    r#"<a href="http://127.0.0.1:{}/maps/place/Cafe+{}">Cafe {}</a>"#,
                                    port, cafe_id, cafe_id
                                ));
                            }
                            let body = format!(
                                r#"<html><body><div role="feed">{}</div></body></html>"#,
                                links
                            );
                            format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
                                body.len(), body
                            )
                        } else if let Some(pos) = request.find("/maps/place/Cafe+") {
                            let end_pos = request[pos..].find(' ').unwrap_or(request[pos..].len());
                            let place_name = &request[pos..pos + end_pos];
                            let num_str: String =
                                place_name.chars().filter(|c| c.is_ascii_digit()).collect();
                            let phone = if num_str.is_empty() {
                                "+84987654320".to_string()
                            } else {
                                format!("+8498765432{}", num_str)
                            };

                            let body = format!(
                                r#"<html>
                                <body>
                                  <h1>{}</h1>
                                  <button data-item-id="phone:tel:{}">Phone Button</button>
                                </body>
                                </html>"#,
                                place_name, phone
                            );
                            format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
                                body.len(), body
                            )
                        } else {
                            let body = "Not Found";
                            format!(
                                "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                                body.len(), body
                            )
                        };
                        let _ = socket.write_all(response.as_bytes()).await;
                    }
                });
            }
        });

        format!("http://127.0.0.1:{}", port)
    }

    #[test]
    fn test_grid_generation_and_subdivision() {
        let bbox = BoundingBox {
            lat_min: 10.0,
            lat_max: 10.2,
            lng_min: 106.0,
            lng_max: 106.2,
        };
        let cells = generate_grid(bbox, 10.0);
        assert!(!cells.is_empty(), "Should generate at least one cell");

        let cell = &cells[0];
        let sub_cells = cell.subdivide();
        assert_eq!(sub_cells.len(), 4, "Subdivision should yield 4 quadrants");

        for child in sub_cells {
            assert_eq!(child.depth, cell.depth + 1);
            assert!((child.step_lat_deg - cell.step_lat_deg / 2.0).abs() < 1e-12);
            assert!((child.step_lng_deg - cell.step_lng_deg / 2.0).abs() < 1e-12);
            assert!(child.radius_m < cell.radius_m);
        }
    }

    #[test]
    fn clips_last_cell_to_bbox_and_keeps_center_inside() {
        let bbox = BoundingBox {
            lat_min: 10.0,
            lat_max: 10.025,
            lng_min: 106.0,
            lng_max: 106.025,
        };
        let cells = generate_initial_cells(
            bbox,
            2.0,
            GridLimits {
                max_depth: 6,
                min_cell_size_m: 20.0,
                max_initial_cells: 100,
                max_total_cells: 1_000,
            },
        )
        .unwrap();

        assert!(cells.iter().any(|cell| cell.bbox.lat_max == bbox.lat_max));
        assert!(cells.iter().any(|cell| cell.bbox.lng_max == bbox.lng_max));
        for cell in cells {
            assert!(cell.bbox.lat_min >= bbox.lat_min);
            assert!(cell.bbox.lat_max <= bbox.lat_max);
            assert!(cell.bbox.lng_min >= bbox.lng_min);
            assert!(cell.bbox.lng_max <= bbox.lng_max);
            assert!((cell.bbox.lat_min..=cell.bbox.lat_max).contains(&cell.center_lat));
            assert!((cell.bbox.lng_min..=cell.bbox.lng_max).contains(&cell.center_lng));
        }
    }

    #[test]
    fn subdivision_stops_when_zoom_does_not_increase() {
        let cell = generate_initial_cells(
            BoundingBox {
                lat_min: 10.0,
                lat_max: 10.0001,
                lng_min: 106.0,
                lng_max: 106.0001,
            },
            1.0,
            GridLimits {
                max_depth: 6,
                min_cell_size_m: 1.0,
                max_initial_cells: 10,
                max_total_cells: 100,
            },
        )
        .unwrap()
        .remove(0);

        assert!(!should_subdivide(
            &cell,
            100,
            55,
            false,
            1.0,
            1,
            GridLimits {
                max_depth: 6,
                min_cell_size_m: 1.0,
                max_initial_cells: 10,
                max_total_cells: 100,
            },
        ));
    }

    #[test]
    fn subdivision_stops_after_two_low_yield_levels() {
        let mut cell = generate_initial_cells(
            BoundingBox {
                lat_min: 10.0,
                lat_max: 10.2,
                lng_min: 106.0,
                lng_max: 106.2,
            },
            10.0,
            GridLimits {
                max_depth: 6,
                min_cell_size_m: 20.0,
                max_initial_cells: 100,
                max_total_cells: 1_000,
            },
        )
        .unwrap()
        .remove(0);
        cell.consecutive_low_yield_levels = 1;

        assert!(!should_subdivide(
            &cell,
            100,
            55,
            false,
            0.01,
            1,
            GridLimits {
                max_depth: 6,
                min_cell_size_m: 20.0,
                max_initial_cells: 100,
                max_total_cells: 1_000,
            },
        ));
    }

    #[tokio::test]
    async fn test_grid_scraper_adaptive_subdivision() {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let manager = BrowserManager::new(1).with_port(port);
        manager.start(true).await.expect("Failed to start browser");

        let session = manager.acquire().await.expect("Failed to acquire session");
        let mock_base = spawn_mock_server().await;

        let scraper = GoogleMapsScraper::with_base_url(session, mock_base);
        let mut grid_scraper = GridScraper::new(scraper);
        grid_scraper.saturation_threshold = 5;

        // Define a single cell centered at 10.1, 106.1 which will return 55 results and trigger subdivision
        let initial_cells = vec![GridCell {
            bbox: BoundingBox {
                lat_min: 10.05,
                lat_max: 10.15,
                lng_min: 106.05,
                lng_max: 106.15,
            },
            center_lat: 10.1,
            center_lng: 106.1,
            step_lat_deg: 0.1,
            step_lng_deg: 0.1,
            radius_m: 5000.0,
            depth: 0,
            parent_unit_id: None,
            consecutive_low_yield_levels: 0,
        }];

        let mut progress_calls = Vec::new();
        let results = grid_scraper
            .search_grid("cafe", initial_cells, |cell, processed, total, msg| {
                progress_calls.push((cell.depth, processed, total, msg.to_string()));
            })
            .await;

        assert!(results.is_ok());
        let results = results.unwrap();

        // Check that subdivision was triggered:
        // Initial cell was processed. Saturated (55 results) -> subdivided.
        // Pushes 4 child cells to the stack. Total total_count = 1 + 4 = 5.
        // The child cells return 2 results each and are not saturated, so no more subdivision.
        // Unique results should contain multiple cafes.
        assert!(results.len() > 10, "Should have retrieved multiple cafes");

        // Let's verify progress callback registered subdivision
        let has_subdivision_msg = progress_calls
            .iter()
            .any(|(_, _, _, msg)| msg.contains("Subdivided"));
        assert!(
            has_subdivision_msg,
            "Progress calls should record subdivision event"
        );

        let _ = manager.close().await;
    }
}
