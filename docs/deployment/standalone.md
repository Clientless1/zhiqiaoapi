# Standalone Deployment

Standalone deployment is suitable for evaluation, personal projects, small teams, and early production environments. It does not require Redis and is easier to operate and troubleshoot. Move to [cluster deployment](cluster.md) when you need multiple replicas or horizontal scaling.

## Docker Compose Deployment

Docker Compose is the recommended standalone deployment path. It does not require Rust, Node.js, or pnpm on the host.

```bash
docker compose up -d
```

This command pulls prebuilt images, so the server does not need to compile the Rust backend or frontend assets.

If you need to build images from the current source tree, use:

```bash
docker compose -f docker-compose.build.yml up -d --build
```

It starts:

- `web`: frontend Nginx service that serves static assets and proxies backend API traffic.
- `backend`: backend API, worker, and scheduler.
- `postgres`: PostgreSQL database.

After startup, open:

```text
http://SERVER_IP:8080
```

The bootstrap wizard will guide you through the admin account, service mode, initial upstream, pricing, SMTP, and payment settings.

### Check Runtime Status

Check container status:

```bash
docker compose ps
```

You should normally see `postgres`, `backend`, and `web` in `running` or `healthy` state.

View all logs:

```bash
docker compose logs -f
```

View backend container logs only:

```bash
docker compose logs -f backend
```

If the bootstrap wizard or login page opens in the browser, the frontend, backend, and reverse proxy path are usually working.

### Common Commands

Stop services:

```bash
docker compose down
```

Rebuild and start:

```bash
docker compose -f docker-compose.build.yml up -d --build
```

Pull prebuilt images and start:

```bash
docker compose pull
docker compose up -d
```

Restart the backend container:

```bash
docker compose restart backend
```

## Source Deployment

Source deployment can be used for development or for manually managed production processes. Development mode is useful for debugging and trying the bootstrap flow; production mode uses release backend and scheduler builds plus a static frontend served by Nginx. Docker Compose is still the recommended production path for most standalone deployments.

### Prerequisites

Prepare these dependencies:

| Dependency | Recommended Version |
| --- | --- |
| PostgreSQL | 16 or compatible |
| Rust | 1.94 or newer |
| Node.js | 20 or compatible |
| pnpm | Version compatible with the frontend project |

Confirm the commands are available:

```bash
psql --version
cargo --version
node --version
pnpm --version
```

If `cargo --version` shows an older version such as 1.75, upgrade the Rust toolchain before running the backend or scheduler.

Create a dedicated zhiqiaoapi database user and database:

```bash
sudo -u postgres psql
```

Run this in `psql`:

```sql
CREATE USER zhiqiaoapi WITH PASSWORD 'change-me';
CREATE DATABASE zhiqiaoapi OWNER zhiqiaoapi;
\q
```

Use this PostgreSQL connection URL in the bootstrap wizard:

```text
postgres://zhiqiaoapi:change-me@localhost:5432/zhiqiaoapi
```

On first startup, if runtime configuration is incomplete, the backend enters bootstrap mode. The bootstrap page writes the database connection, site settings, and random secrets. You usually do not need to edit `.env` manually first.

### Development Mode

Development mode uses the Rust debug build and the Vite dev server.

| Service | Command | Default URL |
| --- | --- | --- |
| Backend | `cargo run -p zhiqiaoapi` | `http://127.0.0.1:8080` |
| Scheduler | `cargo run -p zhiqiaoapi-scheduler` | No HTTP URL |
| Frontend | `cd frontend && pnpm install && pnpm dev --host 0.0.0.0` | `http://SERVER_IP:5173` |

Open `http://SERVER_IP:5173`. The page will redirect to the bootstrap wizard. Complete runtime configuration, admin account setup, service mode, initial upstream, and optional SMTP settings. If saving runtime configuration asks for a restart, restart the backend and refresh the page.

### Production Mode

For manual production deployment, build the backend and scheduler in release mode and serve the frontend static files with Nginx. Keep both Rust processes running with systemd, supervisord, or another process manager.

Build the backend and scheduler:

```bash
cargo build --release -p zhiqiaoapi -p zhiqiaoapi-scheduler
```

Run the backend:

```bash
BIND_ADDR=127.0.0.1:8080 ./target/release/zhiqiaoapi
```

Run the scheduler:

```bash
./target/release/zhiqiaoapi-scheduler
```

You can also manage both processes with systemd. This example assumes the project is installed at `/opt/zhiqiaoapi` and has already been built in release mode from the repository root:

```ini
[Unit]
Description=zhiqiaoapi backend

[Service]
WorkingDirectory=/opt/zhiqiaoapi
Environment=BIND_ADDR=127.0.0.1:8080
Environment=RUST_LOG=info
Environment=ZHIQIAOAPI_ENV_FILE=/opt/zhiqiaoapi/.env
ExecStart=/opt/zhiqiaoapi/deploy/systemd/start-zhiqiaoapi.sh
KillMode=control-group
TimeoutStopSec=30
Restart=always
StandardOutput=append:/var/log/zhiqiaoapi/backend.log
StandardError=append:/var/log/zhiqiaoapi/backend-error.log

[Install]
WantedBy=multi-user.target
```

Save it as `/etc/systemd/system/zhiqiaoapi.service`, then start it:

```bash
sudo mkdir -p /var/log/zhiqiaoapi
sudo systemctl daemon-reload
sudo systemctl enable --now zhiqiaoapi
sudo systemctl status zhiqiaoapi
```

Add logrotate for zhiqiaoapi logs to avoid unbounded log growth:

```bash
sudo tee /etc/logrotate.d/zhiqiaoapi >/dev/null <<'EOF'
/var/log/zhiqiaoapi/*.log {
    daily
    rotate 14
    compress
    missingok
    notifempty
    copytruncate
}
EOF
```

Build the frontend:

```bash
cd frontend
pnpm install
pnpm build
cd ..
```

After the build, serve `frontend/dist` with Nginx or another static web server. The repository includes `deploy/nginx/source-build.conf.example`; it assumes `/usr/share/nginx/html` for static files and proxies backend API and health-check paths to `http://127.0.0.1:8080`.

Use it like this:

```bash
sudo install -d /usr/share/nginx/html
sudo cp -r frontend/dist/. /usr/share/nginx/html/
sudo cp deploy/nginx/source-build.conf.example /etc/nginx/conf.d/zhiqiaoapi.conf
sudo nginx -t
sudo systemctl reload nginx
```

## Capacity and Runtime Settings

- Set `CORS_ALLOWED_ORIGINS` for cross-origin deployments; same-origin reverse proxy deployments usually need no additional CORS configuration.
- The backend request-body limit is controlled by `RELAY_BODY_LIMIT_BYTES` and defaults to 64 MiB. The reverse proxy limit should not be lower than this value.
- Non-streaming JSON and SSE billing usage parsing uses a fixed 16 MiB buffer; usage cannot be extracted from larger upstream responses.
- `USER_CONCURRENT_REQUEST_LIMIT` defaults to 100 concurrent model requests per user. `GLOBAL_CONCURRENT_REQUEST_LIMIT` defaults to 0, which disables the global concurrency limit.
- Channel probes run every 10 minutes and model catalogs sync once per day by default. Adjust them with `CHANNEL_PROBE_INTERVAL_SECONDS` and `UPSTREAM_MODEL_SYNC_INTERVAL_SECONDS`.
- When using background image generation or other workflows that store response assets locally, place `ZHIQIAOAPI_ASSET_DIR` on writable, sufficiently sized, persistent storage.

## Troubleshooting

- Services do not become `running` or `healthy`: start with `docker compose logs -f`.
- `backend` does not start: check PostgreSQL health and the database connection URL.
- The page does not open: confirm host port `8080` is free and check firewall or security group rules.
- Frontend cannot reach the backend in source deployment: confirm Nginx proxies to `http://127.0.0.1:8080` and the backend process is running.
