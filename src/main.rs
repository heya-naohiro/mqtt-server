use std::fs::File;
use std::io::{self, BufReader};
//use tokio::io::{AsyncReadExt, AsyncWriteExt};
use pki_types::{CertificateDer, PrivateKeyDer};
use rustls_pemfile::{certs, rsa_private_keys};
use std::path::Path;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

fn load_certs(path: &Path) -> io::Result<Vec<CertificateDer<'static>>> {
    certs(&mut BufReader::new(File::open(path)?)).collect()
}

fn load_keys(path: &Path) -> io::Result<PrivateKeyDer<'static>> {
    rsa_private_keys(&mut BufReader::new(File::open(path)?))
        .next()
        .unwrap()
        .map(Into::into)
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let certs = load_certs(Path::new("server.crt"))?;
    let key = load_keys(Path::new("private.key"))?;

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let acceptor = TlsAcceptor::from(Arc::new(config));
    let listener = TcpListener::bind("127.0.0.1:7878").await.unwrap();

    loop {
        let (mut stream, addr) = listener.accept().await.unwrap();
        let acceptor = acceptor.clone();
        if let Err(err) = mqttserver::process_connection(&mut stream, acceptor).await {
            println!("Error process from {:?}, err {:?}", addr, err);
        } else {
            println!("Shutdown process from {:?} Successfully", addr);
        }
    }
}
