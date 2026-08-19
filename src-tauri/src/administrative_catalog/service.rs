use std::{
    io::{self, BufReader, Cursor, Read},
    sync::Arc,
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::header::{ACCEPT, CONTENT_LENGTH, USER_AGENT};
use tokio::sync::Mutex;
use zip::ZipArchive;

use super::{
    bounds::GeoBoundsResolver,
    countries::bundled_countries,
    parser::{parse_admin5_codes, parse_country_dump},
    repository::AdministrativeCatalogRepository,
    types::{
        AdministrativeAreaSummary, AdministrativeBounds, AdministrativeCatalogState,
        AdministrativeCountry, AdministrativeCountryStatus, ParsedCountryData,
    },
};

const COUNTRY_DUMP_BASE_URL: &str = "https://download.geonames.org/export/dump";
const ADMIN5_ARCHIVE_URL: &str = "https://download.geonames.org/export/dump/adminCode5.zip";
const DOWNLOAD_USER_AGENT: &str = "RustSale/0.1 administrative-catalog";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Debug, Clone, Copy)]
struct AdministrativeCatalogLimits {
    max_compressed_bytes: usize,
    max_extracted_bytes: u64,
}

impl Default for AdministrativeCatalogLimits {
    fn default() -> Self {
        Self {
            max_compressed_bytes: 128 * 1024 * 1024,
            max_extracted_bytes: 1024 * 1024 * 1024,
        }
    }
}

#[async_trait]
pub trait AdministrativeCatalogDownloader: Send + Sync {
    async fn download(&self, url: &str, max_bytes: usize) -> Result<Vec<u8>>;
}

struct ReqwestAdministrativeCatalogDownloader {
    client: reqwest::Client,
}

impl ReqwestAdministrativeCatalogDownloader {
    fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .context("failed building the GeoNames download client")?;
        Ok(Self { client })
    }
}

#[async_trait]
impl AdministrativeCatalogDownloader for ReqwestAdministrativeCatalogDownloader {
    async fn download(&self, url: &str, max_bytes: usize) -> Result<Vec<u8>> {
        let max_bytes_u64 = u64::try_from(max_bytes).context("download limit exceeds u64")?;
        let response = self
            .client
            .get(url)
            .header(USER_AGENT, DOWNLOAD_USER_AGENT)
            .header(ACCEPT, "application/zip")
            .send()
            .await
            .with_context(|| format!("failed downloading {url}"))?;
        if !response.status().is_success() {
            return Err(anyhow!(
                "GeoNames download {url} returned HTTP {}",
                response.status()
            ));
        }
        if let Some(content_length) = response
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
        {
            if content_length > max_bytes_u64 {
                return Err(anyhow!(
                    "GeoNames compressed download exceeds the {max_bytes} byte limit"
                ));
            }
        }

        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.with_context(|| format!("failed reading {url}"))?;
            if bytes.len().saturating_add(chunk.len()) > max_bytes {
                return Err(anyhow!(
                    "GeoNames compressed download exceeds the {max_bytes} byte limit"
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
}

pub struct AdministrativeCatalogService {
    repository: AdministrativeCatalogRepository,
    downloader: Arc<dyn AdministrativeCatalogDownloader>,
    countries: Arc<Vec<AdministrativeCountry>>,
    limits: AdministrativeCatalogLimits,
    load_guard: Mutex<()>,
}

impl AdministrativeCatalogService {
    pub fn new(repository: AdministrativeCatalogRepository) -> Result<Self> {
        Self::build(
            repository,
            Arc::new(ReqwestAdministrativeCatalogDownloader::new()?),
            AdministrativeCatalogLimits::default(),
        )
    }

    #[cfg(test)]
    fn with_downloader(
        repository: AdministrativeCatalogRepository,
        downloader: Arc<dyn AdministrativeCatalogDownloader>,
        limits: AdministrativeCatalogLimits,
    ) -> Result<Self> {
        Self::build(repository, downloader, limits)
    }

    fn build(
        repository: AdministrativeCatalogRepository,
        downloader: Arc<dyn AdministrativeCatalogDownloader>,
        limits: AdministrativeCatalogLimits,
    ) -> Result<Self> {
        if limits.max_compressed_bytes == 0 || limits.max_extracted_bytes == 0 {
            return Err(anyhow!("administrative catalog limits must be positive"));
        }
        Ok(Self {
            repository,
            downloader,
            countries: Arc::new(bundled_countries()?),
            limits,
            load_guard: Mutex::new(()),
        })
    }

    pub fn countries(&self) -> Vec<AdministrativeCountry> {
        self.countries.as_ref().clone()
    }

    pub async fn load_country(
        &self,
        country_code: &str,
        force_refresh: bool,
    ) -> Result<AdministrativeCountryStatus> {
        let country = self.country(country_code)?.clone();
        let _guard = self.load_guard.lock().await;
        let existing = self.repository.status(&country.code).await?;
        if !force_refresh && existing.state == AdministrativeCatalogState::Ready {
            return Ok(existing);
        }

        let source_url = country_archive_url(&country.code);
        match self.download_country(&country, &source_url).await {
            Ok(parsed) => {
                self.repository
                    .replace_country(parsed, chrono::Utc::now().to_rfc3339(), source_url)
                    .await
            }
            Err(error) => self
                .repository
                .record_failure(&country.code, &error.to_string())
                .await
                .with_context(|| {
                    format!(
                        "failed recording the {} administrative catalog refresh error",
                        country.code
                    )
                }),
        }
    }

    pub async fn status(&self, country_code: &str) -> Result<AdministrativeCountryStatus> {
        let country = self.country(country_code)?;
        self.repository.status(&country.code).await
    }

    pub async fn list_children(
        &self,
        country_code: &str,
        parent_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<AdministrativeAreaSummary>> {
        let country = self.country(country_code)?;
        self.repository
            .list_children(&country.code, parent_id, limit)
            .await
    }

    pub async fn search(
        &self,
        country_code: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<AdministrativeAreaSummary>> {
        let country = self.country(country_code)?;
        self.repository.search(&country.code, query, limit).await
    }

    pub async fn resolve_bounds(&self, geoname_id: i64) -> Result<AdministrativeBounds> {
        let resolver = GeoBoundsResolver::new(self.repository.clone());
        resolver.resolve_bounds(geoname_id).await
    }

    fn country(&self, country_code: &str) -> Result<&AdministrativeCountry> {
        let normalized = country_code.trim().to_ascii_uppercase();
        self.countries
            .iter()
            .find(|country| country.code == normalized)
            .ok_or_else(|| anyhow!("unknown country code {normalized}"))
    }

    async fn download_country(
        &self,
        country: &AdministrativeCountry,
        source_url: &str,
    ) -> Result<ParsedCountryData> {
        let country_archive = self
            .downloader
            .download(source_url, self.limits.max_compressed_bytes)
            .await?;
        validate_compressed_size(
            &country_archive,
            self.limits.max_compressed_bytes,
            "country archive",
        )?;
        let admin5_archive = self
            .downloader
            .download(ADMIN5_ARCHIVE_URL, self.limits.max_compressed_bytes)
            .await?;
        validate_compressed_size(
            &admin5_archive,
            self.limits.max_compressed_bytes,
            "ADM5 archive",
        )?;

        let country = country.clone();
        let max_extracted_bytes = self.limits.max_extracted_bytes;
        tokio::task::spawn_blocking(move || {
            parse_archives(
                &country,
                country_archive,
                admin5_archive,
                max_extracted_bytes,
            )
        })
        .await
        .context("administrative catalog parsing task failed")?
    }
}

fn country_archive_url(country_code: &str) -> String {
    format!("{COUNTRY_DUMP_BASE_URL}/{country_code}.zip")
}

fn validate_compressed_size(bytes: &[u8], limit: usize, label: &str) -> Result<()> {
    if bytes.len() > limit {
        return Err(anyhow!(
            "GeoNames {label} compressed size exceeds the {limit} byte limit"
        ));
    }
    Ok(())
}

fn parse_archives(
    country: &AdministrativeCountry,
    country_archive: Vec<u8>,
    admin5_archive: Vec<u8>,
    max_extracted_bytes: u64,
) -> Result<ParsedCountryData> {
    let admin5_codes = {
        let mut archive = ZipArchive::new(Cursor::new(admin5_archive))
            .context("invalid GeoNames ADM5 ZIP archive")?;
        let entry = archive
            .by_name("adminCode5.txt")
            .context("GeoNames ADM5 ZIP does not contain adminCode5.txt")?;
        validate_extracted_size(entry.size(), max_extracted_bytes, "ADM5")?;
        parse_admin5_codes(BufReader::new(BoundedReader::new(
            entry,
            max_extracted_bytes,
            "ADM5",
        )))?
    };

    let expected_entry = format!("{}.txt", country.code);
    let mut archive = ZipArchive::new(Cursor::new(country_archive))
        .with_context(|| format!("invalid GeoNames {} ZIP archive", country.code))?;
    let entry = archive.by_name(&expected_entry).with_context(|| {
        format!(
            "GeoNames {} ZIP does not contain {expected_entry}",
            country.code
        )
    })?;
    validate_extracted_size(entry.size(), max_extracted_bytes, "country")?;
    parse_country_dump(
        country,
        BufReader::new(BoundedReader::new(entry, max_extracted_bytes, "country")),
        &admin5_codes,
    )
}

fn validate_extracted_size(size: u64, limit: u64, label: &str) -> Result<()> {
    if size > limit {
        return Err(anyhow!(
            "GeoNames {label} extracted entry exceeds the {limit} byte limit"
        ));
    }
    Ok(())
}

struct BoundedReader<R> {
    inner: R,
    remaining: u64,
    label: &'static str,
}

impl<R> BoundedReader<R> {
    fn new(inner: R, limit: u64, label: &'static str) -> Self {
        Self {
            inner,
            remaining: limit,
            label,
        }
    }
}

impl<R: Read> Read for BoundedReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            let mut probe = [0_u8; 1];
            return match self.inner.read(&mut probe)? {
                0 => Ok(0),
                _ => Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "GeoNames {} extracted entry exceeds the configured limit",
                        self.label
                    ),
                )),
            };
        }

        let allowed_u64 = self
            .remaining
            .min(u64::try_from(buffer.len()).map_err(io::Error::other)?);
        let allowed = usize::try_from(allowed_u64).map_err(io::Error::other)?;
        let read = self.inner.read(&mut buffer[..allowed])?;
        self.remaining = self
            .remaining
            .saturating_sub(u64::try_from(read).map_err(io::Error::other)?);
        Ok(read)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, VecDeque},
        io::{Cursor, Write},
        sync::{Arc, Mutex},
    };

    use anyhow::{anyhow, Result};
    use async_trait::async_trait;
    use tempfile::TempDir;
    use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

    use super::{
        AdministrativeCatalogDownloader, AdministrativeCatalogLimits, AdministrativeCatalogService,
        ADMIN5_ARCHIVE_URL, COUNTRY_DUMP_BASE_URL,
    };
    use crate::administrative_catalog::{
        repository::AdministrativeCatalogRepository, types::AdministrativeCatalogState,
    };

    const COUNTRY_DUMP: &str = concat!(
        "100\tAlpha Province\tAlpha Province\tProvince Alpha\t10\t20\tA\tADM1\tVN\t\t01\t\t\t\t0\t\t0\tAsia/Ho_Chi_Minh\t2026-08-01\n",
        "200\tBeta District\tBeta District\tDistrict Beta\t11\t21\tA\tADM2\tVN\t\t01\t02\t\t\t0\t\t0\tAsia/Ho_Chi_Minh\t2026-08-02\n",
    );
    const UPDATED_COUNTRY_DUMP: &str = concat!(
        "100\tUpdated Province\tUpdated Province\tProvince Updated\t10\t20\tA\tADM1\tVN\t\t01\t\t\t\t0\t\t0\tAsia/Ho_Chi_Minh\t2026-08-11\n",
        "200\tBeta District\tBeta District\tDistrict Beta\t11\t21\tA\tADM2\tVN\t\t01\t02\t\t\t0\t\t0\tAsia/Ho_Chi_Minh\t2026-08-11\n",
    );

    enum FakeResponse {
        Bytes(Vec<u8>),
        Error(String),
    }

    #[derive(Default)]
    struct FakeDownloader {
        responses: Mutex<HashMap<String, VecDeque<FakeResponse>>>,
        calls: Mutex<Vec<String>>,
    }

    impl FakeDownloader {
        fn respond(&self, url: &str, response: FakeResponse) {
            self.responses
                .lock()
                .expect("responses lock should not be poisoned")
                .entry(url.to_string())
                .or_default()
                .push_back(response);
        }

        fn calls(&self) -> Vec<String> {
            self.calls
                .lock()
                .expect("calls lock should not be poisoned")
                .clone()
        }
    }

    #[async_trait]
    impl AdministrativeCatalogDownloader for FakeDownloader {
        async fn download(&self, url: &str, _max_bytes: usize) -> Result<Vec<u8>> {
            self.calls
                .lock()
                .expect("calls lock should not be poisoned")
                .push(url.to_string());
            match self
                .responses
                .lock()
                .expect("responses lock should not be poisoned")
                .get_mut(url)
                .and_then(VecDeque::pop_front)
            {
                Some(FakeResponse::Bytes(bytes)) => Ok(bytes),
                Some(FakeResponse::Error(message)) => Err(anyhow!(message)),
                None => Err(anyhow!("no fake response for {url}")),
            }
        }
    }

    fn zip_entry(name: &str, contents: &[u8]) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(cursor);
        writer
            .start_file(
                name,
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
            )
            .expect("zip entry should start");
        writer
            .write_all(contents)
            .expect("zip entry should be written");
        writer.finish().expect("zip should finish").into_inner()
    }

    fn service(
        downloader: Arc<FakeDownloader>,
        limits: AdministrativeCatalogLimits,
    ) -> (TempDir, AdministrativeCatalogService) {
        let temp = tempfile::tempdir().expect("temp dir should exist");
        let repository = AdministrativeCatalogRepository::new(temp.path().join("catalog.db"))
            .expect("repository should initialize");
        let service = AdministrativeCatalogService::with_downloader(repository, downloader, limits)
            .expect("service should initialize");
        (temp, service)
    }

    fn country_url(country_code: &str) -> String {
        format!("{COUNTRY_DUMP_BASE_URL}/{country_code}.zip")
    }

    fn queue_success(downloader: &FakeDownloader, dump: &str) {
        downloader.respond(
            &country_url("VN"),
            FakeResponse::Bytes(zip_entry("VN.txt", dump.as_bytes())),
        );
        downloader.respond(
            ADMIN5_ARCHIVE_URL,
            FakeResponse::Bytes(zip_entry("adminCode5.txt", b"")),
        );
    }

    #[tokio::test]
    async fn reuses_ready_cache_and_only_downloads_again_when_forced() {
        let downloader = Arc::new(FakeDownloader::default());
        queue_success(&downloader, COUNTRY_DUMP);
        queue_success(&downloader, UPDATED_COUNTRY_DUMP);
        let (_temp, service) = service(downloader.clone(), AdministrativeCatalogLimits::default());

        let first = service
            .load_country("VN", false)
            .await
            .expect("initial load should succeed");
        let cached = service
            .load_country("vn", false)
            .await
            .expect("cached load should succeed");
        let refreshed = service
            .load_country("VN", true)
            .await
            .expect("forced refresh should succeed");

        assert_eq!(first.state, AdministrativeCatalogState::Ready);
        assert_eq!(cached.fetched_at, first.fetched_at);
        assert_eq!(refreshed.source_updated_at.as_deref(), Some("2026-08-11"));
        assert_eq!(downloader.calls().len(), 4);
    }

    #[tokio::test]
    async fn rejects_unknown_country_before_downloading() {
        let downloader = Arc::new(FakeDownloader::default());
        let (_temp, service) = service(downloader.clone(), AdministrativeCatalogLimits::default());

        let error = service
            .load_country("ZZ", false)
            .await
            .expect_err("unknown country should fail");

        assert!(error.to_string().contains("unknown country"));
        assert!(downloader.calls().is_empty());
    }

    #[tokio::test]
    async fn records_failure_when_compressed_archive_exceeds_the_limit() {
        let downloader = Arc::new(FakeDownloader::default());
        downloader.respond(&country_url("VN"), FakeResponse::Bytes(vec![0; 33]));
        let (_temp, service) = service(
            downloader,
            AdministrativeCatalogLimits {
                max_compressed_bytes: 32,
                max_extracted_bytes: 1024,
            },
        );

        let status = service
            .load_country("VN", false)
            .await
            .expect("bounded failure should be represented as status");

        assert_eq!(status.state, AdministrativeCatalogState::Failed);
        assert!(status
            .last_error
            .as_deref()
            .is_some_and(|message| message.contains("compressed")));
    }

    #[tokio::test]
    async fn records_failure_when_extracted_entry_exceeds_the_limit() {
        let downloader = Arc::new(FakeDownloader::default());
        queue_success(&downloader, COUNTRY_DUMP);
        let (_temp, service) = service(
            downloader,
            AdministrativeCatalogLimits {
                max_compressed_bytes: 1024 * 1024,
                max_extracted_bytes: 32,
            },
        );

        let status = service
            .load_country("VN", false)
            .await
            .expect("bounded failure should be represented as status");

        assert_eq!(status.state, AdministrativeCatalogState::Failed);
        assert!(status
            .last_error
            .as_deref()
            .is_some_and(|message| message.contains("extracted")));
    }

    #[tokio::test]
    async fn failed_forced_refresh_preserves_the_ready_snapshot() {
        let downloader = Arc::new(FakeDownloader::default());
        queue_success(&downloader, COUNTRY_DUMP);
        downloader.respond(
            &country_url("VN"),
            FakeResponse::Error("network unavailable".to_string()),
        );
        let (_temp, service) = service(downloader, AdministrativeCatalogLimits::default());

        service
            .load_country("VN", false)
            .await
            .expect("initial load should succeed");
        let status = service
            .load_country("VN", true)
            .await
            .expect("failed refresh should return the preserved status");
        let roots = service
            .repository
            .list_children("VN", None, 500)
            .await
            .expect("ready snapshot should remain queryable");

        assert_eq!(status.state, AdministrativeCatalogState::Ready);
        assert_eq!(status.record_count, 2);
        assert_eq!(status.last_error.as_deref(), Some("network unavailable"));
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].name, "Alpha Province");
    }
}
