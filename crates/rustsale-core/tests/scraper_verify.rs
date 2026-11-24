//! RS#53 verification harness (not product code). All tests are #[ignore]; run one at a time:
//!   VERIFY_OUT=/tmp/x RUSTSALE_CLOAK_RUNTIME_PATH=... cargo test -p rustsale-core --test scraper_verify <name> -- --ignored --nocapture
use async_trait::async_trait;
use rustsale_core::google_maps_jobs::commands::GoogleMapsJobService;
use rustsale_core::google_maps_jobs::events::ScrapeEventPublisher;
use rustsale_core::google_maps_jobs::processor::GoogleMapsUnitProcessor;
use rustsale_core::google_maps_jobs::repository::ScrapeJobRepository;
use rustsale_core::google_maps_jobs::runner::{
    FinalizeOutcome, RetryPolicy, ScrapeJobRunner, ScrapeUnitProcessor, UnitExecution,
};
use rustsale_core::google_maps_jobs::types::*;
use rustsale_core::scrapers::browser::{BrowserManager, GoogleMapsPageHealth, GoogleMapsPageState};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn out_dir() -> PathBuf {
    let p = PathBuf::from(std::env::var("VERIFY_OUT").expect("VERIFY_OUT"));
    std::fs::create_dir_all(&p).unwrap();
    p
}
fn env_or(k: &str, d: &str) -> String {
    std::env::var(k).unwrap_or_else(|_| d.to_string())
}

/// Wraps the real processor: caps place units per job and sleeps between units (politeness).
struct Capped {
    inner: GoogleMapsUnitProcessor,
    budget: AtomicUsize,
    pause_ms: u64,
    t0: Instant,
}
#[async_trait]
impl ScrapeUnitProcessor for Capped {
    async fn execute(
        &self,
        job: &ScrapeJob,
        unit: &ScrapeUnit,
    ) -> Result<UnitExecution, ScrapeJobError> {
        tokio::time::sleep(Duration::from_millis(self.pause_ms)).await;
        let started = Instant::now();
        let r = self.inner.execute(job, unit).await;
        println!(
            "[t+{:>4}s] unit {} type={} took={}ms ok={}",
            self.t0.elapsed().as_secs(),
            unit.unit_key,
            unit.unit_type,
            started.elapsed().as_millis(),
            r.is_ok()
        );
        let mut exec = r?;
        let before = exec.child_units.len();
        let mut kept = Vec::new();
        for c in exec.child_units.drain(..) {
            if c.unit_type == "place" {
                if self.budget.load(Ordering::SeqCst) == 0 {
                    continue;
                }
                self.budget.fetch_sub(1, Ordering::SeqCst);
            }
            kept.push(c);
        }
        println!("  children offered={} kept={}", before, kept.len());
        exec.child_units = kept;
        Ok(exec)
    }
    async fn action_health(&self, job: &ScrapeJob) -> Result<GoogleMapsPageHealth, ScrapeJobError> {
        self.inner.action_health(job).await
    }
    async fn relaunch_for_action(&self, job: &ScrapeJob) -> Result<(), ScrapeJobError> {
        self.inner.relaunch_for_action(job).await
    }
    async fn shutdown(&self) -> Result<(), ScrapeJobError> {
        self.inner.shutdown().await
    }
    async fn shutdown_for(&self, id: &str) -> Result<(), ScrapeJobError> {
        self.inner.shutdown_for(id).await
    }
    async fn finalize(
        &self,
        job: &ScrapeJob,
        c: &tokio_util::sync::CancellationToken,
    ) -> Result<FinalizeOutcome, ScrapeJobError> {
        let t = Instant::now();
        let r = self.inner.finalize(job, c).await;
        println!(
            "[finalize] enrichment+import took {}s ok={}",
            t.elapsed().as_secs(),
            r.is_ok()
        );
        r
    }
}

/// Live keyword job, capped at VERIFY_MAX_PLACES places. Dumps job/units/results as JSON into VERIFY_OUT.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore]
async fn live_keyword_job() {
    let out = out_dir();
    let db = out.join("v3dev.db");
    let repo = Arc::new(ScrapeJobRepository::new(db.to_string_lossy().to_string()).unwrap());
    let events = Arc::new(ScrapeEventPublisher::new(256));
    let max: usize = env_or("VERIFY_MAX_PLACES", "20").parse().unwrap();
    let capped = Arc::new(Capped {
        inner: GoogleMapsUnitProcessor::new(repo.clone(), events.clone()),
        budget: AtomicUsize::new(max),
        pause_ms: env_or("VERIFY_PAUSE_MS", "4000").parse().unwrap(),
        t0: Instant::now(),
    });
    let runner = Arc::new(ScrapeJobRunner::new(
        repo.clone(),
        capped,
        events.clone(),
        RetryPolicy::default(),
    ));
    let service = GoogleMapsJobService::new(repo.clone(), runner.clone(), out.join("profiles"));
    let mut rx = events.subscribe();
    let log_task = tokio::spawn(async move {
        while let Ok(e) = rx.recv().await {
            println!("EVENT seq={} {:?}/{:?} units {}/{} failed={} disc={} qual={} leads={} msg={} err={:?}",
                e.sequence, e.status, e.stage, e.processed_units, e.total_units, e.failed_units,
                e.businesses_discovered, e.businesses_qualified, e.leads_created, e.message, e.error.as_ref().map(|x| (x.kind, x.retryable)));
        }
    });
    let job = service
        .create_job(CreateScrapeJobRequest {
            mode: ScrapeMode::Keyword,
            keyword: env_or("VERIFY_KEYWORD", "quán cà phê"),
            location: Some(env_or("VERIFY_LOCATION", "Quận 1, Thành phố Hồ Chí Minh")),
            bbox: None,
            step_km: None,
            districts: vec![],
            headless: true,
        })
        .await
        .unwrap();
    println!("created job ...{}", &job.id[job.id.len() - 6..]);
    runner.start(&job.id).await.unwrap();
    let deadline =
        Instant::now() + Duration::from_secs(env_or("VERIFY_TIMEOUT_S", "1500").parse().unwrap());
    let last = loop {
        let j = service.get_job(&job.id).await.unwrap();
        if j.status.is_terminal()
            || matches!(
                j.status,
                ScrapeJobStatus::PausedRequiresAction | ScrapeJobStatus::PausedLimitReached
            )
        {
            break j;
        }
        if Instant::now() > deadline {
            println!("DEADLINE reached; cancelling");
            let _ = service.cancel_job(&job.id).await;
            break service.get_job(&job.id).await.unwrap();
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    };
    println!("FINAL status={:?} stage={:?} processed={}/{} failed={} disc={} qual={} leads={} err={:?}/{:?}",
        last.status, last.stage, last.processed_units, last.total_units, last.failed_units,
        last.businesses_discovered, last.businesses_qualified, last.leads_created, last.last_error_kind, last.last_error_message);
    let units = repo.list_units(&job.id).await.unwrap();
    let results = repo
        .list_results_page(
            &job.id,
            rustsale_core::paging::places::PlaceQuery::default(),
            None,
            200,
        )
        .await
        .map(|p| serde_json::to_value(p.items).unwrap())
        .unwrap_or_default();
    std::fs::write(
        out.join("job.json"),
        serde_json::to_vec_pretty(&last).unwrap(),
    )
    .unwrap();
    std::fs::write(
        out.join("units.json"),
        serde_json::to_vec_pretty(&units).unwrap(),
    )
    .unwrap();
    std::fs::write(
        out.join("results.json"),
        serde_json::to_vec_pretty(&results).unwrap(),
    )
    .unwrap();
    let _ = runner.wait_for_idle(Duration::from_secs(60)).await;
    log_task.abort();
}

/// DOM probes on live Google Maps: C6/B-U1 (PUA), C10, C18, C19, B-U2. Writes probe.json.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore]
async fn live_dom_probe() {
    let out = out_dir();
    let mgr = BrowserManager::new(1).with_profile_dir(out.join("probe-profile"));
    mgr.start(true).await.expect("start browser");
    let guard = mgr.acquire().await.expect("acquire");
    let sess = &*guard;
    let q = urlencoding::encode(&env_or("VERIFY_KEYWORD", "quán cà phê")).to_string();
    let url = format!("https://www.google.com/maps/search/{q}+Quận+1+Thành+phố+Hồ+Chí+Minh?hl=en");
    let mut probe = serde_json::Map::new();
    sess.navigate(&url).await.expect("navigate listing");
    tokio::time::sleep(Duration::from_secs(8)).await;
    let listing = sess.evaluate(r#"(() => {
      const isVis = e => { const r = e.getBoundingClientRect(); const s = getComputedStyle(e); return r.width>0&&r.height>0&&s.visibility!=='hidden'&&s.display!=='none'; };
      const h1s = [...document.querySelectorAll('h1')].map(h => ({text: (h.textContent||'').trim().slice(0,40), visible: isVis(h), cls: h.className.slice(0,30)}));
      const consent = [...document.querySelectorAll('[aria-label*="consent" i],[aria-label*="Đồng ý" i],[aria-label*="Chấp nhận" i]')].map(e => ({tag:e.tagName, label:(e.getAttribute('aria-label')||'').slice(0,60), visible:isVis(e)}));
      const body = (document.body.innerText||'').slice(0,20000).toLowerCase();
      return {
        url: location.href, lang: document.documentElement.lang, nav_lang: navigator.language, nav_langs: navigator.languages,
        inner: [innerWidth, innerHeight], outer: [outerWidth, outerHeight], dpr: devicePixelRatio,
        has_feed: !!document.querySelector('div[role="feed"]'),
        feed_cards: document.querySelectorAll('div[role="feed"] a[href*="/maps/place/"]').length,
        h1_count: h1s.length, h1s, consent_matches: consent,
        body_has_recaptcha: body.includes('recaptcha'), body_has_unusual: body.includes('unusual traffic'),
        main_h1: !!document.querySelector('[role="main"] h1'),
      };
    })()"#).await.expect("eval listing");
    probe.insert("listing".into(), listing.clone());
    // first few place links
    let links = sess.evaluate(r#"[...document.querySelectorAll('div[role="feed"] a[href*="/maps/place/"]')].slice(0,4).map(a=>a.href)"#).await.expect("links");
    let mut places = Vec::new();
    for (i, l) in links
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .enumerate()
    {
        let href = l.as_str().unwrap_or_default();
        // canonical URL exactly as the scraper opens it: query stripped (C18)
        let canon = href.split('?').next().unwrap_or(href).to_string();
        tokio::time::sleep(Duration::from_secs(6)).await;
        sess.navigate(&canon).await.expect("navigate place");
        tokio::time::sleep(Duration::from_secs(6)).await;
        let v = sess.evaluate(r#"(() => {
          const pua = s => (s||'').split('').filter(c => { const k=c.charCodeAt(0); return k>=0xE000&&k<=0xF8FF; }).map(c=>'U+'+c.charCodeAt(0).toString(16).toUpperCase());
          const addrBtn = document.querySelector('[data-item-id="address"]');
          const rog = document.querySelector('.rogA2c');
          const webBtn = document.querySelector('[data-item-id="authority"]');
          const phoneBtn = document.querySelector('[data-item-id^="phone:tel:"]');
          const h1s = [...document.querySelectorAll('h1')].map(h => (h.textContent||'').trim().slice(0,30));
          const inRowText = e => e ? e.textContent : null;
          return {
            url_has_hl: /[?&]hl=/.test(location.href), url_tail: location.href.split('?')[1]||null,
            lang: document.documentElement.lang,
            h1_count: h1s.length, h1_nonempty: h1s.filter(Boolean).length,
            addr_btn: !!addrBtn, addr_text_pua: pua(addrBtn && addrBtn.textContent), addr_text_len: addrBtn ? addrBtn.textContent.length : 0,
            addr_first_char_code: addrBtn && addrBtn.textContent ? addrBtn.textContent.charCodeAt(0).toString(16) : null,
            addr_aria_prefix: addrBtn ? (addrBtn.getAttribute('aria-label')||'').slice(0,12) : null,
            addr_text_in_lang_probe: addrBtn ? addrBtn.getAttribute('aria-label') && /^(Address|Địa chỉ)/.test(addrBtn.getAttribute('aria-label')) : null,
            rogA2c_present: !!rog, rogA2c_text_pua: pua(rog && rog.textContent),
            web_btn_pua: pua(inRowText(webBtn)), phone_btn_pua: pua(inRowText(phoneBtn)),
            addr_label_lang: addrBtn ? (/^Address/.test(addrBtn.getAttribute('aria-label')||'') ? 'en' : (/^Địa chỉ/.test(addrBtn.getAttribute('aria-label')||'') ? 'vi' : 'other')) : null,
          };
        })()"#).await.expect("eval place");
        println!("place[{i}] -> {}", v);
        places.push(v);
    }
    probe.insert("places".into(), serde_json::Value::Array(places));
    std::fs::write(
        out.join("probe.json"),
        serde_json::to_vec_pretty(&probe).unwrap(),
    )
    .unwrap();
    println!("LISTING {}", listing);
    drop(guard);
    let _ = mgr.close().await;
}

/// State classification (C10/C19) on empty and bogus pages, plus hl effect on place pages (C18).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore]
async fn live_state_probe() {
    let out = out_dir();
    let mgr = BrowserManager::new(1).with_profile_dir(out.join("probe2-profile"));
    mgr.start(true).await.expect("start browser");
    let guard = mgr.acquire().await.expect("acquire");
    let sess = &*guard;
    let h1js = r#"(() => ({ h1: [...document.querySelectorAll('h1')].map(h=>[(h.textContent||'').trim().slice(0,30), h.getBoundingClientRect().height>0]),
        feed: !!document.querySelector('div[role="feed"]'), url: location.href.slice(0,90),
        body: (document.body.innerText||'').replace(/\s+/g,' ').slice(0,160) }))()"#;
    for (name, url) in [
        (
            "empty_search",
            "https://www.google.com/maps/search/zzqxjkwv9283746xx?hl=en",
        ),
        (
            "bogus_place",
            "https://www.google.com/maps/place/zzqxjkwv9283746qq?hl=en",
        ),
        (
            "blank_maps",
            "https://www.google.com/maps/@10.77,106.69,12z?hl=en",
        ),
        (
            "listing",
            "https://www.google.com/maps/search/cafe+District+1+Ho+Chi+Minh?hl=en",
        ),
    ] {
        tokio::time::sleep(Duration::from_secs(6)).await;
        sess.navigate(url).await.expect("navigate");
        tokio::time::sleep(Duration::from_secs(7)).await;
        let st = sess.health_snapshot().await;
        let dom = sess.evaluate(h1js).await.unwrap_or_default();
        println!(
            "STATE {name}: {:?} dom={}",
            st.as_ref()
                .map(|h| (format!("{:?}", h.state), h.title.clone()))
                .map_err(|e| e.message.clone()),
            dom
        );
    }
    // C18: same place URL with and without hl
    let links = sess.evaluate(r#"[...document.querySelectorAll('div[role="feed"] a[href*="/maps/place/"]')].slice(0,1).map(a=>a.href.split('?')[0])"#).await.unwrap();
    let place = links[0].as_str().unwrap_or_default().to_string();
    let probe = r#"(() => { const a=document.querySelector('[data-item-id="address"]'); const cat=document.querySelector('button[jsaction*="category"]'); return {lang:document.documentElement.lang, addr_label:(a&&a.getAttribute('aria-label')||'').slice(0,14), category:(cat&&cat.textContent||'').slice(0,30), url_hl:/[?&]hl=/.test(location.href)}; })()"#;
    for (name, u) in [
        ("no_hl", place.clone()),
        ("hl_vi", format!("{place}?hl=vi")),
        ("hl_en", format!("{place}?hl=en")),
    ] {
        tokio::time::sleep(Duration::from_secs(6)).await;
        sess.navigate(&u).await.expect("navigate place");
        tokio::time::sleep(Duration::from_secs(6)).await;
        println!(
            "HL {name}: {}",
            sess.evaluate(probe).await.unwrap_or_default()
        );
    }
    drop(guard);
    let _ = mgr.close().await;
}

// ---------- hermetic tests (no network, no browser): A-U1, A-U2, A-U4 ----------
use tokio::sync::Notify;

struct Fake {
    gate: Option<Arc<Notify>>,
    panic_in_execute: bool,
    entered: Arc<Notify>,
}
#[async_trait]
impl ScrapeUnitProcessor for Fake {
    async fn execute(
        &self,
        _j: &ScrapeJob,
        _u: &ScrapeUnit,
    ) -> Result<UnitExecution, ScrapeJobError> {
        self.entered.notify_waiters();
        if self.panic_in_execute {
            panic!("simulated panic inside processor");
        }
        if let Some(g) = &self.gate {
            g.notified().await;
        }
        Ok(UnitExecution::default())
    }
    async fn action_health(&self, _j: &ScrapeJob) -> Result<GoogleMapsPageHealth, ScrapeJobError> {
        Ok(GoogleMapsPageHealth {
            state: GoogleMapsPageState::Ready,
            url: String::new(),
            title: String::new(),
        })
    }
    async fn relaunch_for_action(&self, _j: &ScrapeJob) -> Result<(), ScrapeJobError> {
        Ok(())
    }
    async fn shutdown(&self) -> Result<(), ScrapeJobError> {
        Ok(())
    }
}

fn fake_stack(
    db: &std::path::Path,
    proc_: Fake,
) -> (
    Arc<ScrapeJobRepository>,
    Arc<ScrapeJobRunner>,
    GoogleMapsJobService,
) {
    let repo = Arc::new(ScrapeJobRepository::new(db.to_string_lossy().to_string()).unwrap());
    let events = Arc::new(ScrapeEventPublisher::new(64));
    let runner = Arc::new(ScrapeJobRunner::new(
        repo.clone(),
        Arc::new(proc_),
        events,
        RetryPolicy::default(),
    ));
    let svc = GoogleMapsJobService::new(
        repo.clone(),
        runner.clone(),
        db.parent().unwrap().join("profiles"),
    );
    (repo, runner, svc)
}
fn kw() -> CreateScrapeJobRequest {
    CreateScrapeJobRequest {
        mode: ScrapeMode::Keyword,
        keyword: "x".into(),
        location: Some("y".into()),
        bbox: None,
        step_km: None,
        districts: vec![],
        headless: true,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore]
async fn au1_panic_in_processor() {
    let out = out_dir();
    let (repo, runner, svc) = fake_stack(
        &out.join("au1.db"),
        Fake {
            gate: None,
            panic_in_execute: true,
            entered: Arc::new(Notify::new()),
        },
    );
    let job = svc.create_job(kw()).await.unwrap();
    runner.start(&job.id).await.unwrap();
    tokio::time::sleep(Duration::from_secs(5)).await;
    let j = repo.get_job(&job.id).await.unwrap().unwrap();
    println!(
        "AU1 after panic: db status={:?} stage={:?} err={:?}",
        j.status, j.stage, j.last_error_kind
    );
    println!(
        "AU1 runner.active_job_id={:?}",
        runner.active_job_id().await
    );
    let again = runner.pause(&job.id).await;
    println!(
        "AU1 pause after panic -> {:?}",
        again.as_ref().map(|j| j.status).map_err(|e| e.to_string())
    );
    let j2 = repo.get_job(&job.id).await.unwrap().unwrap();
    println!("AU1 final db status={:?}", j2.status);
}

async fn recover_race(two_instances: bool) {
    let out = out_dir();
    let name = if two_instances { "au2.db" } else { "au4.db" };
    let gate = Arc::new(Notify::new());
    let entered = Arc::new(Notify::new());
    let (repo, runner, svc) = fake_stack(
        &out.join(name),
        Fake {
            gate: Some(gate.clone()),
            panic_in_execute: false,
            entered: entered.clone(),
        },
    );
    let job = svc.create_job(kw()).await.unwrap();
    let waiter = entered.notified();
    tokio::pin!(waiter);
    waiter.as_mut().enable();
    runner.start(&job.id).await.unwrap();
    waiter.await; // unit is executing
    let label = if two_instances {
        "AU2(second instance on same DB)"
    } else {
        "AU4(recover_startup racing a started job)"
    };
    let before = repo.get_job(&job.id).await.unwrap().unwrap();
    println!(
        "{label} before: status={:?} version={}",
        before.status, before.version
    );
    let recovered = if two_instances {
        let (_r2, runner2, _s2) = fake_stack(
            &out.join(name),
            Fake {
                gate: None,
                panic_in_execute: false,
                entered: Arc::new(Notify::new()),
            },
        );
        runner2.recover_startup().await.unwrap()
    } else {
        runner.recover_startup().await.unwrap()
    };
    let mid = repo.get_job(&job.id).await.unwrap().unwrap();
    println!(
        "{label} recovered_jobs={recovered} -> status={:?}",
        mid.status
    );
    gate.notify_waiters();
    tokio::time::sleep(Duration::from_secs(4)).await;
    gate.notify_waiters();
    tokio::time::sleep(Duration::from_secs(4)).await;
    let last = repo.get_job(&job.id).await.unwrap().unwrap();
    println!(
        "{label} final: status={:?} stage={:?} processed={}/{} err={:?}/{:?}",
        last.status,
        last.stage,
        last.processed_units,
        last.total_units,
        last.last_error_kind,
        last.last_error_message
    );
    let units = repo.list_units(&job.id).await.unwrap();
    println!(
        "{label} units: {:?}",
        units
            .iter()
            .map(|u| (u.status, u.attempt_count))
            .collect::<Vec<_>>()
    );
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore]
async fn au2_two_instances() {
    recover_race(true).await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore]
async fn au4_recover_race() {
    recover_race(false).await
}

/// A-U3: start the real browser, print pids, then die the way the app does on Quit (no shutdown call).
/// MODE=exit -> std::process::exit(0); MODE=kill -> SIGKILL self; MODE=drop -> orderly close() (control).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore]
async fn au3_orphans_on_exit() {
    let out = out_dir();
    let mgr = BrowserManager::new(1).with_profile_dir(out.join("au3-profile"));
    mgr.start(true).await.expect("start");
    let guard = mgr.acquire().await.expect("acquire");
    guard
        .navigate("https://www.google.com/maps?hl=en")
        .await
        .expect("nav");
    tokio::time::sleep(Duration::from_secs(5)).await;
    println!(
        "AU3 sidecar_pid={:?} self_pid={}",
        mgr.process_id().await,
        std::process::id()
    );
    match env_or("MODE", "exit").as_str() {
        "kill" => unsafe { libc_kill(std::process::id() as i32, 9) },
        "drop" => {
            drop(guard);
            mgr.close().await.expect("close");
            println!("AU3 closed orderly");
        }
        _ => std::process::exit(0),
    }
}
unsafe fn libc_kill(pid: i32, sig: i32) {
    extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    kill(pid, sig);
}
