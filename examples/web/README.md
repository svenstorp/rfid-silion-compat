# Browser Example (HTML + TypeScript)

This example uses the wasm-bindgen JavaScript library output from this crate. The UI is written in TypeScript for type safety and is bundled using Vite for proper module resolution. All artifacts are built into a `dist/` folder that the web server serves from.

## Quick Start

From the `examples/web` directory:

```bash
# Install pnpm dependencies
pnpm install

# Build all artifacts into dist/
pnpm build

# Start the HTTPS dev server
pnpm run serve:https
```

Then open `https://localhost:8443` in a browser.

## Project Structure

- **Source files:**
  - `main.ts` — TypeScript UI code (type-checked, bundled by Vite)
  - `index.html` — HTML template with Vite entry point
  - `tsconfig.json` — TypeScript configuration (configured for bundler mode)
  - `vite.config.ts` — Vite configuration
  - `package.json` — pnpm scripts and dependencies

- **Build output (in `dist/`):**
  - `main.js` — Bundled and minified JavaScript (compiled from TypeScript)
  - `index.html` — Copied from source
  - `pkg/` — WASM package (bindings + WebAssembly binary)

## Available pnpm Scripts

### Building

- `pnpm run check` — Type check only (no emit)
- `pnpm build:wasm` — Build WASM library to `pkg/`
- `pnpm build` — Build WASM and bundle TypeScript with Vite (one-shot)

### Development

- `pnpm run dev` — Start Vite dev server with live reload (requires initial `build:wasm`)

### Running

- `pnpm run serve` — HTTP server on port 8080 (Web Serial won't work, HTTPS required)
- `pnpm run serve:https` — HTTPS server on port 8443 (requires cert)
- `pnpm run preview` — Preview production build locally

### Maintenance

- `pnpm run clean` — Remove `dist/` and `pkg/`

## Building Separately

Build only WASM:

```bash
pnpm run build:wasm
```

Use Vite dev server (after building WASM):

```bash
pnpm run dev
```

## Serving the Example

### HTTP (limited, Web Serial won't work)

```bash
pnpm run serve
# Serves on http://localhost:8080 from dist/
```

Note: Web Serial API only works in secure contexts (HTTPS or certain localhost configurations).

### HTTPS (recommended for development)

```bash
pnpm run serve:https
# Serves on https://localhost:8443 from dist/
```

**First time setup:** Generate a self-signed certificate in `examples/web/`:

```bash
openssl req -x509 -newkey rsa:2048 -keyout key.pem -out cert.pem -days 365 -nodes \
  -subj "/CN=localhost"
```

You'll see a certificate warning in your browser—this is expected and safe for local development.

### Manual HTTPS (without npm scripts)

If scripts don't work:

```bash
cd examples/web/dist

# Generate certificate if needed
openssl req -x509 -newkey rsa:2048 -keyout ../key.pem -out ../cert.pem -days 365 -nodes \
  -subj "/CN=localhost"

# Serve with HTTPS via Python
python3 << 'EOF'
import http.server
import ssl
import os

server = http.server.HTTPServer(('localhost', 8443), http.server.SimpleHTTPRequestHandler)
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain('../cert.pem', '../key.pem')
server.socket = context.wrap_socket(server.socket, server_side=True)
print("Serving on https://localhost:8443")
server.serve_forever()
EOF
```

## Demo Flow

1. Click **Connect** and select your serial device
2. Click **Get Version** to query device versions
3. Click **Start Inventory** to begin async tag reading
4. Monitor tags in the event log
5. Click **Stop Inventory** and **Close** when done
