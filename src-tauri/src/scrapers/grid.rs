use crate::scrapers::google_maps::{BusinessInfo, GoogleMapsScraper};
use anyhow::Result;

#[derive(Debug, Clone, Copy)]
pub struct BoundingBox {
    pub lat_min: f64,
    pub lat_max: f64,
    pub lng_min: f64,
    pub lng_max: f64,
}

#[derive(Debug, Clone)]
pub struct GridCell {
    pub center_lat: f64,
    pub center_lng: f64,
    pub step_lat_deg: f64,
    pub step_lng_deg: f64,
    pub radius_m: f64,
    pub depth: usize,
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
    let mut cells = Vec::new();
    let lat_step_deg = step_km / 111.0;
    
    let mut lat_curr = bbox.lat_min;
    while lat_curr < bbox.lat_max {
        let center_lat = lat_curr + lat_step_deg / 2.0;
        
        let cos_lat = center_lat.to_radians().cos().abs();
        let lng_step_deg = if cos_lat > 1e-6 {
            step_km / (111.0 * cos_lat)
        } else {
            step_km / 111.0
        };
        
        let mut lng_curr = bbox.lng_min;
        while lng_curr < bbox.lng_max {
            let center_lng = lng_curr + lng_step_deg / 2.0;
            let radius_m = step_km * 1000.0 * 0.70710678;

            cells.push(GridCell {
                center_lat,
                center_lng,
                step_lat_deg: lat_step_deg,
                step_lng_deg: lng_step_deg,
                radius_m,
                depth: 0,
            });
            
            lng_curr += lng_step_deg;
        }
        
        lat_curr += lat_step_deg;
    }
    
    if cells.is_empty() {
        let center_lat = (bbox.lat_min + bbox.lat_max) / 2.0;
        let center_lng = (bbox.lng_min + bbox.lng_max) / 2.0;
        let lat_step_deg = (bbox.lat_max - bbox.lat_min).max(step_km / 111.0);
        let cos_lat = center_lat.to_radians().cos().abs();
        let lng_step_deg = (bbox.lng_max - bbox.lng_min).max(if cos_lat > 1e-6 { step_km / (111.0 * cos_lat) } else { step_km / 111.0 });
        let radius_m = step_km * 1000.0 * 0.70710678;
        cells.push(GridCell {
            center_lat,
            center_lng,
            step_lat_deg: lat_step_deg,
            step_lng_deg: lng_step_deg,
            radius_m,
            depth: 0,
        });
    }
    
    cells
}

impl GridCell {
    pub fn subdivide(&self) -> Vec<GridCell> {
        let child_step_lat = self.step_lat_deg / 2.0;
        let child_step_lng = self.step_lng_deg / 2.0;
        let child_radius = self.radius_m / 2.0;
        let child_depth = self.depth + 1;
        
        vec![
            GridCell {
                center_lat: self.center_lat + child_step_lat / 2.0,
                center_lng: self.center_lng - child_step_lng / 2.0,
                step_lat_deg: child_step_lat,
                step_lng_deg: child_step_lng,
                radius_m: child_radius,
                depth: child_depth,
            },
            GridCell {
                center_lat: self.center_lat + child_step_lat / 2.0,
                center_lng: self.center_lng + child_step_lng / 2.0,
                step_lat_deg: child_step_lat,
                step_lng_deg: child_step_lng,
                radius_m: child_radius,
                depth: child_depth,
            },
            GridCell {
                center_lat: self.center_lat - child_step_lat / 2.0,
                center_lng: self.center_lng - child_step_lng / 2.0,
                step_lat_deg: child_step_lat,
                step_lng_deg: child_step_lng,
                radius_m: child_radius,
                depth: child_depth,
            },
            GridCell {
                center_lat: self.center_lat - child_step_lat / 2.0,
                center_lng: self.center_lng + child_step_lng / 2.0,
                step_lat_deg: child_step_lat,
                step_lng_deg: child_step_lng,
                radius_m: child_radius,
                depth: child_depth,
            },
        ]
    }
}

pub struct GridScraper {
    pub scraper: GoogleMapsScraper,
    pub saturation_threshold: usize,
}

impl GridScraper {
    pub fn new(scraper: GoogleMapsScraper) -> Self {
        Self {
            scraper,
            saturation_threshold: 55,
        }
    }

    pub async fn search_grid<F>(&self, keyword: &str, initial_cells: Vec<GridCell>, mut on_progress: F) -> Result<Vec<BusinessInfo>>
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
            
            let msg = format!("Scraping cell at lat: {:.4}, lng: {:.4}, zoom: {}, depth: {}", cell.center_lat, cell.center_lng, zoom, cell.depth);
            on_progress(&cell, processed_count, total_count, &msg);
            
            match self.scraper.search_region(keyword, cell.center_lat, cell.center_lng, zoom).await {
                Ok(results) => {
                    let count = results.len();
                    
                    for item in results {
                        unique_results.insert(item.phone.clone(), item);
                    }
                    
                    if count >= self.saturation_threshold && cell.depth < 4 {
                        let children = cell.subdivide();
                        total_count += children.len();
                        stack.extend(children);
                        
                        let subdivide_msg = format!("Cell saturated ({} results). Subdivided.", count);
                        on_progress(&cell, processed_count, total_count, &subdivide_msg);
                    }
                }
                Err(e) => {
                    let err_msg = format!("Failed to scrape cell: {}", e);
                    on_progress(&cell, processed_count, total_count, &err_msg);
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
    use tokio::net::TcpListener;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
                                    let parts: Vec<&str> = request[at_idx + 1..].split(',').collect();
                                    if parts.len() >= 2 {
                                        if let (Ok(lat), Ok(lng)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) {
                                            let lat_rounded = (lat * 1000.0).round() / 1000.0;
                                            let lng_rounded = (lng * 1000.0).round() / 1000.0;
                                            if (lat_rounded - 10.125).abs() < 0.001 && (lng_rounded - 106.075).abs() < 0.001 {
                                                offset = 10;
                                            } else if (lat_rounded - 10.125).abs() < 0.001 && (lng_rounded - 106.125).abs() < 0.001 {
                                                offset = 20;
                                            } else if (lat_rounded - 10.075).abs() < 0.001 && (lng_rounded - 106.075).abs() < 0.001 {
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
                            let place_name = &request[pos..pos+end_pos];
                            let num_str: String = place_name.chars().filter(|c| c.is_digit(10)).collect();
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
            assert_eq!(child.step_lat_deg, cell.step_lat_deg / 2.0);
            assert_eq!(child.step_lng_deg, cell.step_lng_deg / 2.0);
            assert!(child.radius_m < cell.radius_m);
        }
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
            center_lat: 10.1,
            center_lng: 106.1,
            step_lat_deg: 0.1,
            step_lng_deg: 0.1,
            radius_m: 5000.0,
            depth: 0,
        }];

        let mut progress_calls = Vec::new();
        let results = grid_scraper.search_grid("cafe", initial_cells, |cell, processed, total, msg| {
            progress_calls.push((cell.depth, processed, total, msg.to_string()));
        }).await;

        assert!(results.is_ok());
        let results = results.unwrap();

        // Check that subdivision was triggered:
        // Initial cell was processed. Saturated (55 results) -> subdivided.
        // Pushes 4 child cells to the stack. Total total_count = 1 + 4 = 5.
        // The child cells return 2 results each and are not saturated, so no more subdivision.
        // Unique results should contain multiple cafes.
        assert!(results.len() > 10, "Should have retrieved multiple cafes");
        
        // Let's verify progress callback registered subdivision
        let has_subdivision_msg = progress_calls.iter().any(|(_, _, _, msg)| msg.contains("Subdivided"));
        assert!(has_subdivision_msg, "Progress calls should record subdivision event");

        let _ = manager.close().await;
    }
}
