use hyper::{service::{make_service_fn, service_fn}, Body, Client, Request, Response, Server, Uri};
use tokio_rustls::rustls::{Certificate, PrivateKey, ServerConfig, NoClientAuth};
use tokio_rustls::TlsAcceptor;
use std::{convert::Infallible, net::SocketAddr, sync::Arc};
use std::fs::File;
use std::io::BufReader;
use rustls_pemfile::{certs, pkcs8_private_keys};
use dotenv::dotenv;
use std::env;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::net::TcpListener;
use hyper::server::conn::Http;

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
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from the .env file
    dotenv().ok();

    // Get comma-separated list of upstream servers from environment
    let upstream_servers_str = env::var("UPSTREAM_SERVERS").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let upstream_servers: Vec<String> = upstream_servers_str.split(',').map(|s| s.trim().to_string()).collect();

    // Use an atomic counter for round-robin load balancing
    let counter = Arc::new(AtomicUsize::new(0));

    // Shared upstream server list
    let upstream_servers = Arc::new(upstream_servers);

    // Get the port from environment, default to 443 if SSL is enabled or 80 if not
    let ssl_enabled = env::var("SSL_ENABLED").unwrap_or_else(|_| "false".to_string()) == "true";
    let listen_port: u16 = if ssl_enabled {
        env::var("LISTEN_PORT").unwrap_or_else(|_| "443".to_string()).parse().expect("Invalid port number")
    } else {
        env::var("LISTEN_PORT").unwrap_or_else(|_| "80".to_string()).parse().expect("Invalid port number")
    };

    let addr = SocketAddr::from(([0, 0, 0, 0], listen_port));

    if ssl_enabled {
        // SSL certificate and key
        let ssl_cert_path = env::var("SSL_CERT_PATH").expect("SSL_CERT_PATH not set");
        let ssl_key_path = env::var("SSL_KEY_PATH").expect("SSL_KEY_PATH not set");

        // Load SSL certificate and key
        let cert_file = &mut BufReader::new(File::open(ssl_cert_path).expect("Certificate not found"));
        let key_file = &mut BufReader::new(File::open(ssl_key_path).expect("Private key not found"));

        let certs = certs(cert_file).unwrap()
            .into_iter().map(Certificate).collect::<Vec<_>>();
        let mut keys = pkcs8_private_keys(key_file).unwrap();

        // Create the server config with no client authentication
        let mut config = ServerConfig::new(NoClientAuth::new());
        config.set_single_cert(certs, PrivateKey(keys.remove(0)))
            .expect("Invalid certificate or key");

        // Create a TlsAcceptor to wrap the server
        let tls_acceptor = TlsAcceptor::from(Arc::new(config));

        // Create a TCP listener to listen for incoming TLS connections
        let listener = TcpListener::bind(&addr).await.expect("Failed to bind");

        println!("Listening on https://{}", addr);

        loop {
            let (stream, _) = listener.accept().await?;

            let tls_acceptor = tls_acceptor.clone();
            let upstream_servers = Arc::clone(&upstream_servers);
            let counter = Arc::clone(&counter);

            tokio::spawn(async move {
                let stream = match tls_acceptor.accept(stream).await {
                    Ok(stream) => stream,
                    Err(e) => {
                        eprintln!("Failed to accept TLS connection: {:?}", e);
                        return;
                    }
                };

                let service = service_fn(move |req| {
                    handle_proxy(req, Arc::clone(&upstream_servers), Arc::clone(&counter))
                });

                let http = Http::new();
                if let Err(e) = http.serve_connection(stream, service).await {
                    eprintln!("Server error: {}", e);
                }
            });
        }
    } else {
        // Non-SSL setup: Bind and listen for plain HTTP connections
        let make_svc = make_service_fn(move |_conn| {
            let upstream_servers = Arc::clone(&upstream_servers);
            let counter = Arc::clone(&counter);
            async {
                Ok::<_, Infallible>(service_fn(move |req| {
                    handle_proxy(req, Arc::clone(&upstream_servers), Arc::clone(&counter))
                }))
            }
        });

        let server = Server::bind(&addr).serve(make_svc);

        println!("Listening on http://{}", addr);

        if let Err(e) = server.await {
            eprintln!("server error: {}", e);
        }
    }

    Ok(())
}
