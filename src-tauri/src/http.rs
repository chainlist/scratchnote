//! Where every HTTPS client the app makes starts.

pub fn client() -> reqwest::ClientBuilder {
    let builder = reqwest::Client::builder();
    // Android's certificate store is only reachable through Java, which
    // reqwest's platform verifier would need set up first, so there the app
    // trusts the Mozilla roots it carries.
    #[cfg(target_os = "android")]
    let builder = builder.tls_certs_only(
        webpki_root_certs::TLS_SERVER_ROOT_CERTS
            .iter()
            .filter_map(|der| reqwest::Certificate::from_der(der).ok()),
    );
    builder
}
