use tokio::net::{TcpListener, TcpStream};
use std::env;
use std::sync::Arc;
// use tokio::sync::RwLock;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Server {
    addr: String,
    weight: usize,
}

struct LoadBalancer {
    servers: Vec<Server>,
    current_index: AtomicUsize,
}

impl LoadBalancer {
    fn new(servers: Vec<Server>) -> Self {
        LoadBalancer {
            servers,
            current_index: AtomicUsize::new(0),
        }
    }

    async fn next_server(&self) -> Option<&Server> {
        if self.servers.is_empty() {
            return None;
        }

        let mut weighted_servers = Vec::new();
        for (index, server) in self.servers.iter().enumerate() {
            for _ in 0..server.weight {
                weighted_servers.push(index);
            }
        }

        if weighted_servers.is_empty() {
            return None;
        }

        let index = self.current_index.fetch_add(1, Ordering::Relaxed) % weighted_servers.len();
        let server_index = weighted_servers[index];
        Some(&self.servers[server_index])
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
        });
    }

    println!("Configured servers:");
    for server in &servers {
        println!("  {} (weight: {})", server.addr, server.weight);
    }

    let lb = Arc::new(LoadBalancer::new(servers));

    let listener = TcpListener::bind(format!("0.0.0.0:{}", lb_port)).await?;
    println!("Load balancer listening on port {}", lb_port);

    loop {
        let (client_stream, client_addr) = listener.accept().await?;
        println!("Received connection from: {}", client_addr);

        let lb = Arc::clone(&lb);
        tokio::spawn(async move {
            if let Some(server) = lb.next_server().await {
                if let Err(e) = handle_connection(client_stream, &server.addr).await {
                    eprintln!("Error handling connection: {}", e);
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