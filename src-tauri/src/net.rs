//! Shared HTTP hardening for the artwork and lyrics clients.
//!
//! Both talk to the internet on behalf of data we do not control: artwork
//! URLs come from any MPRIS client on the session bus, lyrics from a
//! crowd-sourced database. So bodies are size-capped, artwork may only be
//! fetched over HTTPS from public addresses, and redirects are limited.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use reqwest::redirect;
use reqwest::Url;

/// Sent with every request; LRCLIB asks clients to identify themselves.
pub fn user_agent() -> String {
    format!(
        "Sahne/{} ({})",
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_REPOSITORY")
    )
}

#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("response body exceeds {0} bytes")]
    TooLarge(usize),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
}

/// Reads a response body, aborting as soon as it grows past `max` bytes.
/// A declared `Content-Length` over the limit is rejected up front; without
/// one, the body is streamed and never buffered beyond `max`.
pub async fn read_capped(mut resp: reqwest::Response, max: usize) -> Result<Vec<u8>, ReadError> {
    if resp
        .content_length()
        .is_some_and(|n| usize::try_from(n).map_or(true, |n| n > max))
    {
        return Err(ReadError::TooLarge(max));
    }
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await? {
        if body.len() + chunk.len() > max {
            return Err(ReadError::TooLarge(max));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// Addresses that must never be contacted on an untrusted URL's behalf:
/// loopback, private, link-local, CGNAT, multicast, unspecified and the like.
pub fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_public_v4(v4);
            }
            let seg0 = v6.segments()[0];
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (seg0 & 0xfe00) == 0xfc00 // unique local fc00::/7
                || (seg0 & 0xffc0) == 0xfe80 // link-local fe80::/10
                || v6 == Ipv6Addr::new(0x64, 0xff9b, 0, 0, 0, 0, 0, 0))
        }
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    !(ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_multicast()
        || ip.is_documentation()
        || a == 0
        || (a == 100 && (64..128).contains(&b)) // CGNAT 100.64.0.0/10
        || (a == 198 && (b == 18 || b == 19)) // benchmarking 198.18.0.0/15
        || a >= 240)
}

#[derive(Debug, thiserror::Error)]
pub enum UrlError {
    #[error("only https:// artwork is allowed")]
    NotHttps,
    #[error("artwork host is a non-public address")]
    PrivateHost,
    #[error("artwork URL has no host")]
    NoHost,
}

/// Checks an artwork URL before fetching: https only, and a literal IP host
/// must be public (names are checked by [`PublicResolver`] when resolved).
pub fn check_remote_url(url: &Url) -> Result<(), UrlError> {
    if url.scheme() != "https" {
        return Err(UrlError::NotHttps);
    }
    match url.host() {
        None => Err(UrlError::NoHost),
        Some(url::Host::Ipv4(ip)) if !is_public_v4(ip) => Err(UrlError::PrivateHost),
        Some(url::Host::Ipv6(ip)) if !is_public_ip(IpAddr::V6(ip)) => Err(UrlError::PrivateHost),
        Some(_) => Ok(()),
    }
}

pub const MAX_REDIRECTS: usize = 3;

/// Redirects: at most [`MAX_REDIRECTS`] hops, each passing [`check_remote_url`].
pub fn redirect_policy() -> redirect::Policy {
    redirect::Policy::custom(|attempt| {
        if attempt.previous().len() > MAX_REDIRECTS {
            return attempt.error("too many redirects");
        }
        match check_remote_url(attempt.url()) {
            Ok(()) => attempt.follow(),
            Err(e) => attempt.error(e),
        }
    })
}

/// DNS resolver that drops non-public addresses, so neither a hostname nor a
/// redirect target can steer requests into the local network.
#[derive(Debug, Default)]
pub struct PublicResolver;

impl Resolve for PublicResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_owned();
        Box::pin(async move {
            let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 0))
                .await?
                .filter(|a| is_public_ip(a.ip()))
                .collect();
            if addrs.is_empty() {
                return Err(format!("{host} resolves to no public address").into());
            }
            let addrs: Addrs = Box::new(addrs.into_iter());
            Ok(addrs)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn classifies_addresses() {
        for private in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "255.255.255.255",
            "::1",
            "fd00::1",
            "fe80::1",
            "::ffff:192.168.0.1",
        ] {
            assert!(!is_public_ip(private.parse().unwrap()), "{private}");
        }
        for public in ["1.1.1.1", "151.101.1.1", "2606:4700::1111"] {
            assert!(is_public_ip(public.parse().unwrap()), "{public}");
        }
    }

    #[test]
    fn remote_urls_must_be_https_and_public() {
        let ok = |s: &str| check_remote_url(&Url::parse(s).unwrap());
        assert!(ok("https://i.scdn.co/image/abc").is_ok());
        assert!(matches!(
            ok("http://i.scdn.co/image/abc"),
            Err(UrlError::NotHttps)
        ));
        assert!(matches!(
            ok("https://192.168.1.10/x.jpg"),
            Err(UrlError::PrivateHost)
        ));
        assert!(matches!(
            ok("https://[::1]/x.jpg"),
            Err(UrlError::PrivateHost)
        ));
        assert!(matches!(ok("ftp://example.com/x"), Err(UrlError::NotHttps)));
    }

    #[tokio::test]
    async fn resolver_drops_private_addresses() {
        let name: Name = "localhost".parse().unwrap();
        assert!(PublicResolver.resolve(name).await.is_err());
    }

    /// Serves an endless chunked body (no Content-Length) on a local port.
    async fn endless_server() -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = sock.read(&mut buf).await;
            let _ = sock
                .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n")
                .await;
            let chunk = vec![b'x'; 64 * 1024];
            let head = format!("{:x}\r\n", chunk.len());
            loop {
                if sock.write_all(head.as_bytes()).await.is_err()
                    || sock.write_all(&chunk).await.is_err()
                    || sock.write_all(b"\r\n").await.is_err()
                {
                    return;
                }
            }
        });
        format!("http://{addr}/")
    }

    #[tokio::test]
    async fn stream_without_length_is_capped() {
        let url = endless_server().await;
        let resp = reqwest::Client::new().get(url).send().await.unwrap();
        let max = 2 * 1024 * 1024;
        assert!(matches!(read_capped(resp, max).await, Err(ReadError::TooLarge(m)) if m == max));
    }
}
