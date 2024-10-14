
# Riffy - A Lightweight Reverse Proxy

Riffy is a high-performance, lightweight reverse proxy written in Rust. It supports multiple upstream servers with round-robin load balancing. Configuration is done using environment variables, allowing for flexible and dynamic setups in cloud-native environments like Kubernetes.

## Features

- HTTP request proxying
- Multiple upstream servers with round-robin load balancing
- Environment variable-based configuration
- SSL/TLS (future support)
- Kubernetes-friendly design

## Getting Started

These instructions will help you set up and deploy Riffy in a Kubernetes environment.

### Prerequisites

- Rust and Cargo (for building the application)
- Kubernetes (for deployment)
- Docker (to containerize the application)
- `dotenv` crate to manage environment variables.

### Building the Application

1. Clone the repository:

   ```bash
   git clone https://github.com/chrisjchandler/riffy.git
   cd riffy
   ```

2. Install the Rust dependencies:

   ```bash
   cargo build --release
   ```

3. The binary will be available in the `target/release` directory:

   ```bash
   ./target/release/riffy
   ```

### Environment Configuration

Riffy uses a `.env` file for configuration. The following environment variables are required:

- `UPSTREAM_SERVERS`: Comma-separated list of upstream servers.
- `LISTEN_PORT`: The port on which Riffy listens (default: 443).
- `SSL_CERT_PATH`: Path to the SSL certificate (optional, for future TLS support).
- `SSL_KEY_PATH`: Path to the SSL private key (optional, for future TLS support).

### Example `.env` File

Create a `.env` file in the root of the project:

```dotenv
# Comma-separated list of upstream servers
UPSTREAM_SERVERS=http://backend1:8080,http://backend2:8080,http://backend3:8080

# Port to listen on
LISTEN_PORT=443

# SSL Certificate and Key (optional for future SSL support)
SSL_CERT_PATH=/path/to/cert.pem
SSL_KEY_PATH=/path/to/key.pem
```

### Running Riffy Locally

1. Ensure you have the `.env` file in place.
2. Run the application:

   ```bash
   cargo run --release
   ```

### Dockerizing Riffy

To deploy Riffy in Kubernetes, you first need to containerize the application:

1. Create a `Dockerfile`:

   ```dockerfile
   FROM rust:latest AS builder
   WORKDIR /usr/src/app
   COPY . .
   RUN cargo build --release

   FROM debian:buster-slim
   WORKDIR /usr/local/bin
   COPY --from=builder /usr/src/app/target/release/riffy .
   COPY .env .
   CMD ["./riffy"]
   ```

2. Build the Docker image:

   ```bash
   docker build -t yourusername/riffy:latest .
   ```

3. Push the image to Docker Hub (or your preferred container registry):

   ```bash
   docker push yourusername/riffy:latest
   ```

### Deploying Riffy in Kubernetes

1. Create a Kubernetes ConfigMap for the `.env` file:

   ```yaml
   apiVersion: v1
   kind: ConfigMap
   metadata:
     name: riffy-config
   data:
     .env: |
       UPSTREAM_SERVERS=http://backend1:8080,http://backend2:8080,http://backend3:8080
       LISTEN_PORT=443
   ```

2. Create a Kubernetes Deployment and Service:

   ```yaml
   apiVersion: apps/v1
   kind: Deployment
   metadata:
     name: riffy-deployment
   spec:
     replicas: 2
     selector:
       matchLabels:
         app: riffy
     template:
       metadata:
         labels:
           app: riffy
       spec:
         containers:
         - name: riffy
           image: yourusername/riffy:latest
           ports:
           - containerPort: 443
           volumeMounts:
           - name: riffy-config-volume
             mountPath: /usr/local/bin/.env
             subPath: .env
         volumes:
         - name: riffy-config-volume
           configMap:
             name: riffy-config

   ---
   apiVersion: v1
   kind: Service
   metadata:
     name: riffy-service
   spec:
     type: LoadBalancer
     ports:
     - port: 443
       targetPort: 443
     selector:
       app: riffy
   ```

3. Apply the Kubernetes manifests:

   ```bash
   kubectl apply -f riffy-configmap.yaml
   kubectl apply -f riffy-deployment.yaml
   ```

### Testing

Once deployed, you can access Riffy through the LoadBalancer service in Kubernetes. Verify that requests are being forwarded to the upstream servers in a round-robin fashion.

```bash
curl http://<LoadBalancer-IP>
```

## License

## License

This project is licensed under the MIT License - see the [LICENSE.txt](https://github.com/chrisjchandler/Riffy/blob/main/LICENSE.txt) file for details.
