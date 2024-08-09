# Rust Load Balancer

A simple, weighted round-robin load balancer implemented in Rust using Tokio for asynchronous I/O.

## Features

- Weighted round-robin load balancing algorithm
- Configurable backend servers with custom weights
- Asynchronous handling of connections using Tokio
- Simple command-line interface for configuration

## Prerequisites

- Rust programming language (latest stable version)
- Cargo package manager

## Dependencies

- tokio
- reqwest
- futures
- 

## Usage

1. Clone the repository
2. Run `cargo run` to start the load balancer
3. Configure the backend servers and their weights in the command line. for eg
```
cargo run -- 8080 10 --ports 8081,1 8082,1 8083,1
```
4. Start your backend servers
5. Connect to the load balancer using a TCP client


## How it works

The load balancer listens for incoming connections on the specified port. When a connection is received, it selects the next available server using the weighted round-robin algorithm and forwards the connection to that server.

## License

[MIT License](https://opensource.org/licenses/MIT)









