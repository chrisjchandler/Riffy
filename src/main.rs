use hyper::{service::{make_service_fn, service_fn}, Body, Client, Request, Response, Server, Uri};
use tokio_rustls::rustls::{Certificate, PrivateKey, ServerConfig, NoClientAuth};
use std::{convert::Infallible, net::SocketAddr, sync::Arc};
use std::fs::File;
use std::io::BufReader;
use rustls_pemfile::{certs, pkcs8_private_keys};
use dotenv::dotenv;
use std::env;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Proxies the incoming request to the upstream server.
async fn handle_proxy(req: Request<Body>, upstream_servers: Arc<Vec<String>>, counter: Arc<AtomicUsize>) -> Result<Response<Body>, Box<dyn std::error::Error + Send + Sync>> {
    let client = Client::new();

    // Round-robin load balancing: Get the next upstream server from the list
    let index = counter.fetch_add(1, Ordering::SeqCst) % upstream_servers.len();
    let upstream_server = &upstream_servers[index];

    // Construct the URI correctly
    let uri_string = format!("{}{}", upstream_server, req.uri());
    let uri: Uri = uri_string.parse()?;

    let proxy_req = Request::builder()
        .method(req.method())
        .uri(uri)
        .body(req.into_body()).map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    let res = client.request(proxy_req).await?;

    Ok(res)
}

#[tokio::main]
async fn main() {
    // Load environment variables from the .env file
    dotenv().ok();

    // Get comma-separated list of upstream servers from environment, default to localhost:8080 if not set
    let upstream_servers_str = env::var("UPSTREAM_SERVERS").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let upstream_servers: Vec<String> = upstream_servers_str.split(',').map(|s| s.trim().to_string()).collect();

    // Use an atomic counter for round-robin load balancing
    let counter = Arc::new(AtomicUsize::new(0));

    // Shared upstream server list
    let upstream_servers = Arc::new(upstream_servers);

    // Get the port from environment, default to 443 if not set
    let listen_port: u16 = env::var("LISTEN_PORT").unwrap_or_else(|_| "443".to_string()).parse().expect("Invalid port number");

    let addr = SocketAddr::from(([0, 0, 0, 0], listen_port));

    // Load your cert and key (optional for future support)
    let ssl_cert_path = env::var("SSL_CERT_PATH").unwrap_or_else(|_| "".to_string());
    let ssl_key_path = env::var("SSL_KEY_PATH").unwrap_or_else(|_| "".to_string());

    if !ssl_cert_path.is_empty() && !ssl_key_path.is_empty() {
        let cert_file = &mut BufReader::new(File::open(ssl_cert_path).expect("Certificate not found"));
        let key_file = &mut BufReader::new(File::open(ssl_key_path).expect("Private key not found"));

        let certs = certs(cert_file).unwrap()
            .into_iter().map(Certificate).collect::<Vec<_>>();
        let mut keys = pkcs8_private_keys(key_file).unwrap();

        // Create the server config with no client authentication (future support)
        let mut config = ServerConfig::new(NoClientAuth::new());

        config.set_single_cert(certs, PrivateKey(keys.remove(0)))
            .expect("Invalid certificate or key");

        // TLS support can be added here if needed
    }

    // Create the service function to handle incoming connections
    let make_svc = make_service_fn(move |_conn| {
        let upstream_servers = Arc::clone(&upstream_servers);
        let counter = Arc::clone(&counter);
        async {
            Ok::<_, Infallible>(service_fn(move |req| {
                handle_proxy(req, Arc::clone(&upstream_servers), Arc::clone(&counter))
            }))
        }
    });

    // Bind the server and start listening for connections
    let server = Server::bind(&addr).serve(make_svc);

    println!("Listening on https://{}", addr);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}
