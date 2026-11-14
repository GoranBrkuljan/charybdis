use std::time::Duration;

use crate::args::Args;
use openssl::ssl::{SslConnector, SslFiletype, SslMethod};
use openssl::x509::X509;
use openssl::x509::store::X509StoreBuilder;
use scylla::client::session::{OpenSsl010Config, Session};
use scylla::client::session_builder::SessionBuilder;

pub async fn initialize_session(args: &Args) -> Session {
    let mut builder = SessionBuilder::new()
        .known_node(&args.host)
        .use_keyspace(&args.keyspace, false)
        .connection_timeout(Duration::from_secs(args.timeout));

    if let (Some(user), Some(password)) = (&args.user, &args.password) {
        builder = builder.user(user, password);
    }

    if let Some(ca) = &args.ca {
        let tls_config = tls_config(ca, args)
            .map_err(|e| {
                eprintln!("{}", e);
                std::process::exit(1);
            })
            .unwrap();

        builder = builder.tls_context(Some(tls_config));
    }

    builder
        .build()
        .await
        .map_err(|e| {
            eprintln!("Failed to create session: {}", e);
            std::process::exit(1);
        })
        .unwrap()
}

/// Builds the TLS configuration from the `--ca`, `--cert` and `--key` arguments.
///
/// The driver can only support TLS session tickets for a context it builds itself, so we hand it
/// an `SslConnectorBuilder` instead of a ready `SslContext`.
fn tls_config(ca: &str, args: &Args) -> Result<OpenSsl010Config, String> {
    // `SslConnector` verifies the peer certificate by default.
    let mut connector_builder =
        SslConnector::builder(SslMethod::tls()).map_err(|e| format!("Failed to create SSL context: {}", e))?;

    // Trust the provided CA and nothing else: `SslConnector` would otherwise also trust the
    // system's CA store.
    let ca_pem = std::fs::read(ca).map_err(|e| format!("Failed to read CA file: {}", e))?;
    let ca_certs = X509::stack_from_pem(&ca_pem).map_err(|e| format!("Failed to parse CA file: {}", e))?;

    if ca_certs.is_empty() {
        return Err(format!("Failed to set CA file: no certificates found in {}", ca));
    }

    let mut ca_store = X509StoreBuilder::new().map_err(|e| format!("Failed to create CA store: {}", e))?;

    for ca_cert in ca_certs {
        ca_store
            .add_cert(ca_cert)
            .map_err(|e| format!("Failed to set CA file: {}", e))?;
    }

    connector_builder.set_cert_store(ca_store.build());

    if let Some(cert) = &args.cert {
        connector_builder
            .set_certificate_file(cert, SslFiletype::PEM)
            .map_err(|e| format!("Failed to set certificate file: {}", e))?;

        let key = args
            .key
            .as_ref()
            .expect("Private key file is required when certificate is provided");

        connector_builder
            .set_private_key_file(key, SslFiletype::PEM)
            .map_err(|e| format!("Failed to set private key file: {}", e))?;
    }

    Ok(OpenSsl010Config::new(connector_builder))
}
