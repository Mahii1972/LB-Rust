use tokio::net::{TcpListener, TcpStream};
use std::env;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use async_nats::connect;
use futures::StreamExt; // Added this line to import StreamExt

struct Server {
    addr: String,
    weight: usize,
    active_connections: AtomicUsize,
}

struct LoadBalancer {
    servers: Vec<Server>,
    request_count: AtomicUsize,
}

impl LoadBalancer {
    fn new(servers: Vec<Server>) -> Self {
        LoadBalancer {
            servers,
            request_count: AtomicUsize::new(0),
        }
    }

    async fn next_server(&self) -> Option<&Server> {
        if self.servers.is_empty() {
            return None;
        }

        let mut min_ratio = f64::MAX;
        let mut selected_server = None;

        for server in &self.servers {
            let active_connections = server.active_connections.load(Ordering::Relaxed);
            let target_ratio = server.weight as f64;
            let current_ratio = active_connections as f64 / target_ratio;

            if current_ratio < min_ratio {
                min_ratio = current_ratio;
                selected_server = Some(server);
            }
        }

        selected_server
    }

    async fn update_active_connections(&self, ratios: &[usize]) {
        for (i, server) in self.servers.iter().enumerate() {
            server.active_connections.store(ratios[i], Ordering::Relaxed);
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <lb_port> --ports <port>,<weight> ...", args[0]);
        std::process::exit(1);
    }

    let lb_port = args[1].parse::<u16>()?;

    let mut servers = Vec::new();
    for arg in &args[3..] {
        let parts: Vec<&str> = arg.split(',').collect();
        if parts.len() != 2 {
            eprintln!("Invalid server specification: {}", arg);
            std::process::exit(1);
        }
        let port = parts[0].parse::<u16>()?;
        let weight = parts[1].parse::<usize>()?;
        let server_addr = format!("127.0.0.1:{}", port);
        servers.push(Server {
            addr: server_addr.clone(),
            weight,
            active_connections: AtomicUsize::new(0),
        });
    }

    println!("Configured servers:");
    for server in &servers {
        println!("  {} (weight: {})", server.addr, server.weight);
    }

    let lb = Arc::new(LoadBalancer::new(servers));

    let nats_client = connect("nats://localhost:4222").await?;
    let mut subscription = nats_client.subscribe("active_connections").await?;

    let listener = TcpListener::bind(format!("0.0.0.0:{}", lb_port)).await?;
    println!("Load balancer listening on port {}", lb_port);

    let lb_clone = Arc::clone(&lb);
    tokio::spawn(async move {
        while let Some(msg) = subscription.next().await {
            let payload = String::from_utf8_lossy(&msg.payload);
            println!("Received active connection message: {}", payload); // Log the received message
            let ratios: Vec<usize> = payload
                .split(':')
                .filter_map(|s| s.parse().ok())
                .collect();

            if ratios.len() == 3 {
                lb_clone.update_active_connections(&ratios).await;
            }
        }
    });

    loop {
        let (client_stream, client_addr) = listener.accept().await?;
        println!("Received connection from: {}", client_addr);

        let lb = Arc::clone(&lb);
        tokio::spawn(async move {
            if let Some(server) = lb.next_server().await {
                server.active_connections.fetch_add(1, Ordering::Relaxed);
                if let Err(e) = handle_connection(client_stream, &server.addr).await {
                    eprintln!("Error handling connection: {}", e);
                }
                server.active_connections.fetch_sub(1, Ordering::Relaxed);
                
                let request_count = lb.request_count.fetch_add(1, Ordering::Relaxed) + 1;
                if request_count % 10 == 0 {
                    // Trigger active connections update after every 10 requests
                    let ratios: Vec<usize> = lb.servers.iter()
                        .map(|s| s.active_connections.load(Ordering::Relaxed))
                        .collect();
                    lb.update_active_connections(&ratios).await;
                }
            } else {
                eprintln!("No servers available");
            }
        });
    }
}

async fn handle_connection(mut client_stream: TcpStream, server_addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut server_stream = TcpStream::connect(server_addr).await?;

    tokio::io::copy_bidirectional(&mut client_stream, &mut server_stream).await?;

    Ok(())
}