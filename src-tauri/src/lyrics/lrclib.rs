//! Minimal LRCLIB (https://lrclib.net) client.
//!
//! LRCLIB rate-limits aggressively (HTTP 429 with `retry-after`), so requests
//! are serialized with a minimum spacing and 429s are retried a few times.

use std::time::{Duration, Instant};

use reqwest::StatusCode;
use serde::Deserialize;
use tokio::sync::Mutex;

const BASE: &str = "https://lrclib.net/api";
/// LRCLIB answers 429 to more than about one request per second.
const MIN_SPACING: Duration = Duration::from_millis(1100);
const MAX_RETRIES: u32 = 3;
const MAX_RETRY_WAIT: Duration = Duration::from_secs(10);
/// No real lyrics response comes close; anything bigger is junk or hostile.
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
/// LRCLIB's own matching tolerance for search results.
pub const DURATION_TOLERANCE_S: f64 = 3.0;

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    #[serde(default)]
    pub track_name: String,
    #[serde(default)]
    pub artist_name: String,
    pub duration: Option<f64>,
    #[serde(default)]
    pub instrumental: bool,
    pub plain_lyrics: Option<String>,
    pub synced_lyrics: Option<String>,
}

/// LRCLIB is crowd-sourced and has junk entries (e.g. a single `[00:00.00]probe`
/// line under a famous song's name). Real lyrics have more than a few lines.
const MIN_LYRIC_LINES: usize = 4;

fn plausible_synced(s: &str) -> bool {
    let sung = s
        .lines()
        .filter(|l| {
            // Text after the leading [..] tags.
            let text = l.trim().rsplit(']').next().unwrap_or("");
            !text.trim().is_empty()
        })
        .count();
    sung >= MIN_LYRIC_LINES
}

fn plausible_plain(s: &str) -> bool {
    s.lines().filter(|l| !l.trim().is_empty()).count() >= MIN_LYRIC_LINES
}

impl Record {
    pub fn has_synced(&self) -> bool {
        self.synced_lyrics.as_deref().is_some_and(plausible_synced)
    }

    pub fn has_plain(&self) -> bool {
        self.plain_lyrics.as_deref().is_some_and(plausible_plain)
    }

    /// Anything worth showing: lyrics or an explicit instrumental flag.
    pub fn is_useful(&self) -> bool {
        self.has_synced() || self.has_plain() || self.instrumental
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    /// Could not reach the server at all: treat as offline.
    #[error("network unavailable: {0}")]
    Offline(reqwest::Error),
    /// Rate limited (429) or server trouble (5xx) beyond our retries; transient.
    #[error("LRCLIB is busy or unavailable")]
    Busy,
    #[error("LRCLIB error: {0}")]
    Http(StatusCode),
    #[error("unexpected LRCLIB response: {0}")]
    Decode(reqwest::Error),
    /// Any other transport failure (TLS, protocol, redirect...).
    #[error("LRCLIB request failed: {0}")]
    Request(reqwest::Error),
    #[error("LRCLIB response is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("LRCLIB response exceeds {MAX_BODY_BYTES} bytes")]
    TooLarge,
}

/// Request pacing; shortened in tests.
#[derive(Debug, Clone, Copy)]
struct Timing {
    spacing: Duration,
    /// First retry wait; doubles per attempt.
    backoff: Duration,
    /// Wait before retrying a failed connect once.
    reconnect: Duration,
    timeout: Duration,
}

const TIMING: Timing = Timing {
    spacing: MIN_SPACING,
    backoff: Duration::from_secs(1),
    reconnect: Duration::from_millis(1500),
    timeout: Duration::from_secs(10),
};

pub struct Client {
    http: reqwest::Client,
    base: String,
    timing: Timing,
    last_request: Mutex<Option<Instant>>,
}

impl Client {
    pub fn new() -> Result<Self, reqwest::Error> {
        Self::with(BASE, TIMING)
    }

    fn with(base: &str, timing: Timing) -> Result<Self, reqwest::Error> {
        let http = reqwest::Client::builder()
            .user_agent(crate::net::user_agent())
            .timeout(timing.timeout)
            .connect_timeout(Duration::from_secs(5))
            .build()?;
        Ok(Self {
            http,
            base: base.trim_end_matches('/').to_owned(),
            timing,
            last_request: Mutex::new(None),
        })
    }

    /// Exact lookup. `Ok(None)` means LRCLIB has no such track.
    pub async fn get(
        &self,
        artist: &str,
        title: &str,
        album: &str,
        duration_s: u64,
    ) -> Result<Option<Record>, FetchError> {
        self.lookup("get", artist, title, album, duration_s).await
    }

    /// Like [`Client::get`] but only consults LRCLIB's own database: ~0.3 s
    /// instead of several seconds when `get` falls back to external sources.
    pub async fn get_cached(
        &self,
        artist: &str,
        title: &str,
        album: &str,
        duration_s: u64,
    ) -> Result<Option<Record>, FetchError> {
        self.lookup("get-cached", artist, title, album, duration_s)
            .await
    }

    async fn lookup(
        &self,
        endpoint: &str,
        artist: &str,
        title: &str,
        album: &str,
        duration_s: u64,
    ) -> Result<Option<Record>, FetchError> {
        let duration = duration_s.to_string();
        let mut query = vec![
            ("artist_name", artist),
            ("track_name", title),
            ("duration", duration.as_str()),
        ];
        if !album.is_empty() {
            query.push(("album_name", album));
        }
        match self.request(endpoint, &query).await? {
            Some(resp) => read_json(resp).await.map(Some),
            None => Ok(None),
        }
    }

    pub async fn search(&self, artist: &str, title: &str) -> Result<Vec<Record>, FetchError> {
        let query = [("track_name", title), ("artist_name", artist)];
        match self.request("search", &query).await? {
            Some(resp) => read_json(resp).await,
            None => Ok(Vec::new()),
        }
    }

    /// Sends a GET with spacing and 429 retries. `Ok(None)` on 404.
    async fn request(
        &self,
        endpoint: &str,
        query: &[(&str, &str)],
    ) -> Result<Option<reqwest::Response>, FetchError> {
        let url = format!("{}/{endpoint}", self.base);
        for attempt in 0..=MAX_RETRIES {
            self.wait_turn().await;
            let resp = match self.http.get(&url).query(query).send().await {
                Ok(resp) => resp,
                // A single failed connect is often a blip; only a repeat means offline.
                Err(e) if e.is_connect() && attempt == 0 => {
                    tracing::debug!("LRCLIB connect failed ({e}), retrying once");
                    tokio::time::sleep(self.timing.reconnect).await;
                    continue;
                }
                Err(e) if e.is_connect() => return Err(FetchError::Offline(e)),
                // Reachable but slow (LRCLIB's `get` can take seconds): retry, don't claim offline.
                Err(e) if e.is_timeout() => return Err(FetchError::Busy),
                Err(e) => return Err(FetchError::Request(e)),
            };
            match resp.status() {
                s if s.is_success() => return Ok(Some(resp)),
                StatusCode::NOT_FOUND => return Ok(None),
                s if (s == StatusCode::TOO_MANY_REQUESTS || s.is_server_error())
                    && attempt < MAX_RETRIES =>
                {
                    // Retry-After is often 1 s, which just earns another 429; back off harder.
                    let backoff = self.timing.backoff * (1 << attempt);
                    let wait = retry_after(&resp).map_or(backoff, |r| r.max(backoff));
                    tracing::debug!("LRCLIB {s}, retrying in {wait:?}");
                    tokio::time::sleep(wait.min(MAX_RETRY_WAIT)).await;
                }
                s if s == StatusCode::TOO_MANY_REQUESTS || s.is_server_error() => {
                    return Err(FetchError::Busy)
                }
                s => return Err(FetchError::Http(s)),
            }
        }
        Err(FetchError::Busy)
    }

    async fn wait_turn(&self) {
        let mut last = self.last_request.lock().await;
        if let Some(t) = *last {
            let since = t.elapsed();
            if since < self.timing.spacing {
                tokio::time::sleep(self.timing.spacing - since).await;
            }
        }
        *last = Some(Instant::now());
    }
}

/// Parses a JSON body read under [`MAX_BODY_BYTES`].
async fn read_json<T: serde::de::DeserializeOwned>(
    resp: reqwest::Response,
) -> Result<T, FetchError> {
    read_json_capped(resp, MAX_BODY_BYTES).await
}

async fn read_json_capped<T: serde::de::DeserializeOwned>(
    resp: reqwest::Response,
    max: usize,
) -> Result<T, FetchError> {
    let body = crate::net::read_capped(resp, max)
        .await
        .map_err(|e| match e {
            crate::net::ReadError::TooLarge(_) => FetchError::TooLarge,
            crate::net::ReadError::Http(e) => FetchError::Decode(e),
        })?;
    Ok(serde_json::from_slice(&body)?)
}

fn retry_after(resp: &reqwest::Response) -> Option<Duration> {
    let secs: u64 = resp
        .headers()
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()?;
    Some(Duration::from_secs(secs))
}

/// Picks the best search result: duration within tolerance, synced lyrics
/// preferred over plain ones, closest duration first.
pub fn pick_best(records: &[Record], duration_s: f64) -> Option<&Record> {
    let close = |r: &&Record| {
        r.duration
            .is_some_and(|d| (d - duration_s).abs() <= DURATION_TOLERANCE_S)
    };
    let distance = |r: &Record| r.duration.map_or(f64::MAX, |d| (d - duration_s).abs());
    let best_by = |pred: &dyn Fn(&Record) -> bool| {
        records
            .iter()
            .filter(close)
            .filter(|r| pred(r))
            .min_by(|a, b| distance(a).total_cmp(&distance(b)))
    };
    best_by(&|r| r.has_synced())
        .or_else(|| best_by(&|r| r.has_plain()))
        .or_else(|| best_by(&|r| r.instrumental))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(duration: f64, synced: bool, plain: bool) -> Record {
        Record {
            track_name: "t".into(),
            artist_name: "a".into(),
            duration: Some(duration),
            instrumental: !synced && !plain,
            plain_lyrics: plain.then(|| "a\nb\nc\nd".into()),
            synced_lyrics: synced
                .then(|| "[00:01.00]a\n[00:02.00]b\n[00:03.00]c\n[00:04.00]d".into()),
        }
    }

    #[test]
    fn rejects_junk_entries() {
        let mut r = rec(213.0, true, true);
        r.synced_lyrics = Some("[00:00.00]probe".into());
        r.plain_lyrics = Some("probe".into());
        assert!(!r.has_synced() && !r.has_plain() && !r.is_useful());
        // Blank stamped lines don't count as lyrics.
        r.synced_lyrics = Some("[00:01.00]a\n[00:02.00]\n[00:03.00] \n[00:04.00]b".into());
        assert!(!r.has_synced());
    }

    #[test]
    fn decodes_lrclib_json() {
        let json = r#"{"id":1,"name":"x","trackName":"Işıklı Günler","artistName":"Örnek Sanatçı",
            "albumName":"Deneme Albümü","duration":213.0,"instrumental":false,"hasWordSync":false,
            "plainLyrics":"Çiçekler sabahı\nbekliyor\ngölgede üşüyen\nİstanbul",
            "syncedLyrics":"[00:18.68] Çiçekler sabahı\n[00:20.00]bekliyor\n[00:22.84]gölgede üşüyen\n[00:24.00]İstanbul"}"#;
        let r: Record = serde_json::from_str(json).unwrap();
        assert_eq!(r.duration, Some(213.0));
        assert!(r.has_synced() && r.has_plain());
        let r: Record = serde_json::from_str(r#"{"trackName":"x","artistName":"y","duration":null,"instrumental":true,"plainLyrics":null,"syncedLyrics":null}"#).unwrap();
        assert!(r.instrumental && !r.has_synced() && !r.has_plain());
    }

    #[test]
    fn prefers_synced_within_tolerance() {
        let records = vec![
            rec(213.0, false, true),
            rec(260.0, true, true),
            rec(215.5, true, true),
            rec(212.0, true, false),
        ];
        let best = pick_best(&records, 213.0).unwrap();
        assert_eq!(best.duration, Some(212.0)); // synced and closest
    }

    #[test]
    fn falls_back_to_plain_then_instrumental() {
        let records = vec![rec(100.0, true, true), rec(213.0, false, true)];
        assert_eq!(pick_best(&records, 214.0).unwrap().duration, Some(213.0));
        let records = vec![rec(213.0, false, false)];
        assert!(pick_best(&records, 213.0).unwrap().instrumental);
        assert!(pick_best(&[rec(300.0, true, true)], 213.0).is_none());
    }

    // S9: an oversized body is refused instead of buffered and parsed.
    #[tokio::test]
    async fn oversized_body_is_refused() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = sock.read(&mut buf).await;
            let body = format!("[{}]", vec!["{}"; 4096].join(","));
            let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len());
            let _ = sock.write_all(head.as_bytes()).await;
            let _ = sock.write_all(body.as_bytes()).await;
        });
        let resp = reqwest::get(format!("http://{addr}/")).await.unwrap();
        let got: Result<Vec<Record>, _> = read_json_capped(resp, 1024).await;
        assert!(matches!(got, Err(FetchError::TooLarge)));
    }

    // T3: the client against a mock LRCLIB.
    mod http {
        use super::super::*;
        use wiremock::matchers::{method, path, query_param};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        const FAST: Timing = Timing {
            spacing: Duration::from_millis(5),
            backoff: Duration::from_millis(20),
            reconnect: Duration::from_millis(20),
            timeout: Duration::from_millis(500),
        };

        fn record_json() -> serde_json::Value {
            serde_json::json!({
                "trackName": "Işıklı Günler", "artistName": "Örnek Sanatçı", "duration": 213.0,
                "instrumental": false, "plainLyrics": "bir\niki\nüç\ndört",
                "syncedLyrics": "[00:01.00]bir\n[00:02.00]iki\n[00:03.00]üç\n[00:04.00]dört"
            })
        }

        async fn client(server: &MockServer) -> Client {
            Client::with(&format!("{}/api", server.uri()), FAST).unwrap()
        }

        #[tokio::test]
        async fn ok_returns_the_record() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/api/get"))
                .and(query_param("track_name", "Işıklı Günler"))
                .respond_with(ResponseTemplate::new(200).set_body_json(record_json()))
                .expect(1)
                .mount(&server)
                .await;
            let got = client(&server)
                .await
                .get("Örnek Sanatçı", "Işıklı Günler", "", 213)
                .await
                .unwrap()
                .unwrap();
            assert!(got.has_synced() && got.has_plain());
        }

        #[tokio::test]
        async fn not_found_is_none() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .respond_with(ResponseTemplate::new(404))
                .mount(&server)
                .await;
            let got = client(&server).await.get("a", "b", "", 1).await.unwrap();
            assert!(got.is_none());
        }

        #[tokio::test]
        async fn too_many_requests_honours_retry_after() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "1"))
                .up_to_n_times(1)
                .mount(&server)
                .await;
            Mock::given(method("GET"))
                .respond_with(ResponseTemplate::new(200).set_body_json(record_json()))
                .mount(&server)
                .await;
            let started = Instant::now();
            let got = client(&server).await.get("a", "b", "", 213).await.unwrap();
            assert!(got.is_some());
            assert!(
                started.elapsed() >= Duration::from_secs(1),
                "waited {:?}",
                started.elapsed()
            );
            assert_eq!(server.received_requests().await.unwrap().len(), 2);
        }

        #[tokio::test]
        async fn persistent_server_errors_are_busy() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .respond_with(ResponseTemplate::new(503))
                .mount(&server)
                .await;
            let got = client(&server).await.get("a", "b", "", 1).await;
            assert!(matches!(got, Err(FetchError::Busy)), "{got:?}");
            assert_eq!(
                server.received_requests().await.unwrap().len(),
                MAX_RETRIES as usize + 1
            );
        }

        #[tokio::test]
        async fn other_statuses_are_http_errors() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .respond_with(ResponseTemplate::new(400))
                .mount(&server)
                .await;
            let got = client(&server).await.get("a", "b", "", 1).await;
            assert!(
                matches!(got, Err(FetchError::Http(StatusCode::BAD_REQUEST))),
                "{got:?}"
            );
        }

        #[tokio::test]
        async fn slow_server_is_busy_not_offline() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(record_json())
                        .set_delay(Duration::from_secs(2)),
                )
                .mount(&server)
                .await;
            let got = client(&server).await.get("a", "b", "", 1).await;
            assert!(matches!(got, Err(FetchError::Busy)), "{got:?}");
        }

        #[tokio::test]
        async fn unreachable_server_is_offline() {
            // Bind and drop: nothing listens on this port any more.
            let port = std::net::TcpListener::bind("127.0.0.1:0")
                .unwrap()
                .local_addr()
                .unwrap()
                .port();
            // Windows retries a refused connection for about two seconds, so
            // the request needs more than FAST's timeout to see the refusal.
            let timing = Timing {
                timeout: Duration::from_secs(10),
                ..FAST
            };
            let client = Client::with(&format!("http://127.0.0.1:{port}/api"), timing).unwrap();
            let got = client.get("a", "b", "", 1).await;
            assert!(matches!(got, Err(FetchError::Offline(_))), "{got:?}");
        }

        #[tokio::test]
        async fn oversized_response_is_refused() {
            let server = MockServer::start().await;
            let huge = format!("[{}]", vec!["{}"; MAX_BODY_BYTES / 2].join(","));
            Mock::given(method("GET"))
                .and(path("/api/search"))
                .respond_with(ResponseTemplate::new(200).set_body_string(huge))
                .mount(&server)
                .await;
            let got = client(&server).await.search("a", "b").await;
            assert!(matches!(got, Err(FetchError::TooLarge)), "{got:?}");
        }

        // A track change aborts the lookup task (`sync::Inflight`); the
        // request in flight must go with it instead of finishing later.
        #[tokio::test]
        async fn aborting_the_task_drops_the_request() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(record_json())
                        .set_delay(Duration::from_millis(400)),
                )
                .mount(&server)
                .await;
            let client = std::sync::Arc::new(client(&server).await);
            let task = {
                let client = std::sync::Arc::clone(&client);
                tokio::spawn(async move { client.get("a", "b", "", 1).await })
            };
            tokio::time::sleep(Duration::from_millis(100)).await;
            task.abort();
            let joined = task.await;
            assert!(joined.unwrap_err().is_cancelled());
            assert_eq!(server.received_requests().await.unwrap().len(), 1);
        }
    }
}
