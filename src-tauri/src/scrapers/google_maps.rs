use crate::scrapers::browser::SessionGuard;
use anyhow::Result;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct BusinessInfo {
    pub name: String,
    pub phone: String,
    pub address: Option<String>,
    pub website: Option<String>,
    pub facebook: Option<String>,
    pub zalo: Option<String>,
}

pub struct GoogleMapsScraper {
    pub session: SessionGuard,
    pub base_url: String,
    pub min_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl GoogleMapsScraper {
    pub fn new(session: SessionGuard) -> Self {
        Self {
            session,
            base_url: "https://www.google.com/maps".to_string(),
            min_delay_ms: 8000,
            max_delay_ms: 15000,
        }
    }

    #[allow(dead_code)]
    pub fn with_base_url(session: SessionGuard, base_url: String) -> Self {
        Self {
            session,
            base_url,
            min_delay_ms: 10,
            max_delay_ms: 50,
        }
    }

    async fn delay(&self) {
        if self.min_delay_ms >= self.max_delay_ms {
            tokio::time::sleep(Duration::from_millis(self.min_delay_ms)).await;
        } else {
            let delay = rand::thread_rng().gen_range(self.min_delay_ms..=self.max_delay_ms);
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
    }

    pub async fn search(&self, keyword: &str, location: Option<&str>) -> Result<Vec<BusinessInfo>> {
        let query = if let Some(loc) = location {
            format!("{} {}", keyword, loc)
        } else {
            keyword.to_string()
        };
        let encoded = urlencoding::encode(&query);
        let url = format!("{}/search/{}?hl=en", self.base_url, encoded);
        self.scrape_url(&url).await
    }

    pub async fn search_region(
        &self,
        keyword: &str,
        lat: f64,
        lng: f64,
        zoom: u8,
    ) -> Result<Vec<BusinessInfo>> {
        let encoded_keyword = urlencoding::encode(keyword);
        let url = format!(
            "{}/search/{}/@{},{},{}z?hl=en",
            self.base_url, encoded_keyword, lat, lng, zoom
        );
        self.scrape_url(&url).await
    }

    pub async fn scrape_url(&self, url: &str) -> Result<Vec<BusinessInfo>> {
        self.session.navigate(url).await?;

        // Wait for maps page to load (feed, place title, or place links)
        for _ in 0..20 {
            let is_loaded = self
                .session
                .evaluate(
                    r#"
                (() => {
                    return !!(
                        document.querySelector('div[role="feed"]') || 
                        document.querySelector('h1') || 
                        document.querySelector('a[href*="/maps/place/"]')
                    );
                })()
            "#,
                )
                .await
                .unwrap_or(serde_json::Value::Bool(false));
            if is_loaded.as_bool().unwrap_or(false) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        let current_url = self
            .session
            .evaluate("window.location.href")
            .await?
            .as_str()
            .unwrap_or("")
            .to_string();

        if current_url.contains("/maps/place/") {
            let _ = self.session.evaluate(r#"
                (() => {
                    const btn = document.querySelector('[data-item-id^="phone:tel:"]') || document.querySelector('button[aria-label*="Phone"]');
                    if (btn) btn.click();
                })()
            "#).await;
            tokio::time::sleep(Duration::from_millis(500)).await;

            if let Some(biz) = self.extract_current_place().await? {
                return Ok(vec![biz]);
            }
            return Ok(vec![]);
        }

        // Scroll feed to load all results
        for _ in 0..10 {
            let _ = self
                .session
                .evaluate(
                    r#"
                (() => {
                    const feed = document.querySelector('div[role="feed"]');
                    if (feed) {
                        feed.scrollTop += 1500;
                    } else {
                        window.scrollBy(0, 1500);
                    }
                })()
            "#,
                )
                .await?;
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        // Collect all place URLs
        let urls_val = self
            .session
            .evaluate(
                r#"
            (() => {
                const links = Array.from(document.querySelectorAll('a'));
                return links
                    .map(a => {
                        try { return a.href; } catch(e) { return ""; }
                    })
                    .filter(href => href && href.includes('/maps/place/'));
            })()
        "#,
            )
            .await?;

        let mut urls: Vec<String> = serde_json::from_value(urls_val).unwrap_or_default();
        urls.sort();
        urls.dedup();

        let mut results = Vec::new();
        for place_url in urls {
            if let Err(e) = self.session.navigate(&place_url).await {
                eprintln!("Failed to navigate to {}: {}", place_url, e);
                continue;
            }

            self.delay().await;

            let _ = self.session.evaluate(r#"
                (() => {
                    const btn = document.querySelector('[data-item-id^="phone:tel:"]') || document.querySelector('button[aria-label*="Phone"]');
                    if (btn) btn.click();
                })()
            "#).await;
            tokio::time::sleep(Duration::from_millis(500)).await;

            match self.extract_current_place().await {
                Ok(Some(biz)) => {
                    results.push(biz);
                }
                Ok(None) => {}
                Err(e) => {
                    eprintln!("Error extracting place details: {}", e);
                }
            }
        }

        Ok(results)
    }

    async fn extract_current_place(&self) -> Result<Option<BusinessInfo>> {
        let val = self.session.evaluate(r#"
            (() => {
                let name = '';
                const nameEl = document.querySelector('h1');
                if (nameEl) {
                    name = nameEl.textContent.trim();
                }

                let phone = '';
                const phoneEl = document.querySelector('[data-item-id^="phone:tel:"]');
                if (phoneEl) {
                    const itemId = phoneEl.getAttribute('data-item-id');
                    phone = itemId.replace('phone:tel:', '').trim();
                }
                if (!phone) {
                    const telLink = document.querySelector('a[href^="tel:"]');
                    if (telLink) {
                        phone = telLink.getAttribute('href').replace('tel:', '').trim();
                    }
                }
                if (!phone) {
                    const elements = Array.from(document.querySelectorAll('button, a, div, span'));
                    for (const el of elements) {
                        const txt = el.textContent ? el.textContent.trim() : '';
                        const cleaned = txt.replace(/[\s.-]/g, '');
                        if (/^(?:\+84|0)(?:\d){9,10}$/.test(cleaned)) {
                            phone = txt;
                            break;
                        }
                    }
                }

                let address = '';
                const addressEl = document.querySelector('[data-item-id="address"]');
                if (addressEl) {
                    address = addressEl.textContent.trim();
                } else {
                    const rogEl = document.querySelector('.rogA2c');
                    if (rogEl) {
                        address = rogEl.textContent.trim();
                    }
                }

                let website = '';
                const websiteEl = document.querySelector('[data-item-id="authority"]');
                if (websiteEl) {
                    website = websiteEl.getAttribute('href') || websiteEl.textContent.trim();
                } else {
                    const links = Array.from(document.querySelectorAll('a'));
                    for (const link of links) {
                        const href = link.getAttribute('href') || '';
                        if (href.startsWith('http') && !href.includes('google.com') && !href.includes('gstatic.com') && !href.includes('facebook.com') && !href.includes('zalo.me')) {
                            website = href;
                            break;
                        }
                    }
                }

                let facebook = '';
                let zalo = '';
                const links = Array.from(document.querySelectorAll('a'));
                for (const link of links) {
                    const href = link.getAttribute('href') || '';
                    if (href.includes('facebook.com')) {
                        facebook = href;
                    } else if (href.includes('zalo.me')) {
                        zalo = href;
                    }
                }

                return {
                    name,
                    phone,
                    address: address || null,
                    website: website || null,
                    facebook: facebook || null,
                    zalo: zalo || null
                };
            })()
        "#).await?;

        #[derive(Deserialize)]
        struct RawInfo {
            name: String,
            phone: String,
            address: Option<String>,
            website: Option<String>,
            facebook: Option<String>,
            zalo: Option<String>,
        }

        if let Ok(raw) = serde_json::from_value::<RawInfo>(val) {
            if raw.phone.is_empty() || raw.name.is_empty() {
                return Ok(None);
            }
            Ok(Some(BusinessInfo {
                name: raw.name,
                phone: raw.phone,
                address: raw.address,
                website: raw.website,
                facebook: raw.facebook,
                zalo: raw.zalo,
            }))
        } else {
            Ok(None)
        }
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
                            let body = format!(
                                r#"<html>
                                <body>
                                  <div role="feed">
                                    <a href="http://127.0.0.1:{}/maps/place/Cafe+One">Cafe One</a>
                                    <a href="http://127.0.0.1:{}/maps/place/Cafe+Two">Cafe Two</a>
                                  </div>
                                </body>
                                </html>"#,
                                port, port
                            );
                            format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
                                body.len(), body
                            )
                        } else if request.contains("/maps/place/Cafe+One") {
                            let body = r#"<html>
                            <body>
                              <h1>Cafe One</h1>
                              <button data-item-id="phone:tel:+84987654321">Phone Button</button>
                              <button data-item-id="address">123 Le Loi St, Dist 1, HCMC</button>
                              <a data-item-id="authority" href="https://cafeone.com">Website</a>
                              <a href="https://facebook.com/cafeone">Facebook</a>
                              <a href="https://zalo.me/cafeone">Zalo</a>
                            </body>
                            </html>"#;
                            format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
                                body.len(), body
                            )
                        } else if request.contains("/maps/place/Cafe+Two") {
                            let body = r#"<html>
                            <body>
                              <h1>Cafe Two</h1>
                              <button data-item-id="address">456 Nguyen Hue St, Dist 1, HCMC</button>
                            </body>
                            </html>"#;
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

    #[tokio::test]
    async fn test_google_maps_scraper_search() {
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
        let results = scraper.search("cafe", Some("District 1")).await;

        assert!(results.is_ok(), "search failed: {:?}", results.err());
        let results = results.unwrap();

        assert_eq!(results.len(), 1);
        let first = &results[0];
        assert_eq!(first.name, "Cafe One");
        assert_eq!(first.phone, "+84987654321");
        assert_eq!(
            first.address,
            Some("123 Le Loi St, Dist 1, HCMC".to_string())
        );
        assert_eq!(first.website, Some("https://cafeone.com".to_string()));
        assert_eq!(
            first.facebook,
            Some("https://facebook.com/cafeone".to_string())
        );
        assert_eq!(first.zalo, Some("https://zalo.me/cafeone".to_string()));

        let _ = manager.close().await;
    }
}
