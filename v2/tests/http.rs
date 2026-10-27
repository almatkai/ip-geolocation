use std::{sync::Arc, time::Duration};
use tezbar_ip_geolocation::services::{decode_public_response, https_body, public_urls};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
#[derive(Debug)]
struct FixedTime;
impl rustls::time_provider::TimeProvider for FixedTime {
    fn current_time(&self) -> Option<rustls::pki_types::UnixTime> {
        Some(rustls::pki_types::UnixTime::since_unix_epoch(
            Duration::from_secs(1798761600),
        ))
    }
}
async fn fixture(response: Vec<u8>, delay: bool, trusted: bool) -> Result<String, String> {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let cert =
        rustls::pki_types::CertificateDer::from(include_bytes!("../qa/tls/cert.der").to_vec());
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
        include_bytes!("../qa/tls/key.der").to_vec(),
    ));
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert.clone()], key)
        .unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
    let server = async move {
        let (socket, _) = listener.accept().await.unwrap();
        if let Ok(mut stream) = acceptor.accept(socket).await {
            let mut buffer = [0; 4096];
            let _ = stream.read(&mut buffer).await;
            if delay {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            let _ = stream.write_all(&response).await;
            let _ = stream.shutdown().await;
        }
    };
    let client = async move {
        let mut roots = rustls::RootCertStore::empty();
        if trusted {
            roots.add(cert.clone()).unwrap();
        }
        let mut config = rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        config.time_provider = Arc::new(FixedTime);
        let builder = reqwest::Client::builder()
            .use_preconfigured_tls(config)
            .no_proxy()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .resolve("fixture.test", address)
            .timeout(Duration::from_millis(60));
        let client = builder.build().unwrap();
        let response = client
            .get(format!("https://fixture.test:{}/", address.port()))
            .send()
            .await
            .map_err(|_| "Synthetic TLS request failed".to_string())?;
        decode_public_response(response).await
    };
    let (_, result) = tokio::join!(server, client);
    result
}
#[tokio::test]
async fn bounded_tls_decoder_status_timeout_reflection() {
    for status in [201, 301, 401, 429, 500, 599] {
        let response=format!("HTTP/1.1 {status} Synthetic\r\nLocation: https://evil.test/TOKEN\r\nContent-Length: 18\r\n\r\nURL_TOKEN_REFLECTED");
        let e = fixture(response.into_bytes(), false, true)
            .await
            .unwrap_err();
        assert!(!e.contains("TOKEN") && !e.contains("evil") && !e.contains("fixture.test"));
    }
    assert_eq!(
        fixture(
            b"HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\n198.51.100.4\n".to_vec(),
            false,
            true
        )
        .await
        .unwrap(),
        "198.51.100.4\n"
    );
    assert!(fixture(
        b"HTTP/1.1 200 OK\r\nContent-Length: 99999\r\n\r\n".to_vec(),
        false,
        true
    )
    .await
    .is_err());
    let mut response = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n801\r\n".to_vec();
    response.extend(vec![b'x'; 2049]);
    response.extend(b"\r\n0\r\n\r\n");
    assert!(fixture(response, false, true).await.is_err());
    assert!(fixture(
        b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\n\xff".to_vec(),
        false,
        true
    )
    .await
    .is_err());
    assert!(fixture(
        b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\nx".to_vec(),
        true,
        true
    )
    .await
    .is_err());
    assert!(fixture(
        b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
        false,
        false
    )
    .await
    .is_err());
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    assert!(https_body(&client, "https://evil.test/").await.is_err());
    assert_eq!(
        public_urls(4),
        ["https://icanhazip.com/", "https://api.ipify.org/"]
    );
    assert_eq!(
        public_urls(6),
        ["https://icanhazip.com/", "https://api6.ipify.org/"]
    );
}
