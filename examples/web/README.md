# Browser Example (HTML + TypeScript)

This example uses the wasm-bindgen JavaScript library output from this crate. The UI is written in TypeScript for type safety and builds all artifacts into a `dist/` folder that the web server serves from.

## Quick Start

From the `examples/web` directory:

```bash
# Install npm dependencies
npm install

# Build all artifacts into dist/
npm run build

# Start the HTTPS dev server
npm run serve:https
```

Then open `https://localhost:8443` in a browser.

## Project Structure

- **Source files:**
  - `main.ts` — TypeScript UI code (type-checked)
  - `index.html` — HTML template
  - `tsconfig.json` — TypeScript configuration
  - `package.json` — npm scripts and dependencies

- **Build output (in `dist/`):**
  - `main.js` — Compiled TypeScript
  - `index.html` — Copied from source
  - `pkg/` — WASM package (bindings + WebAssembly binary)

## Available npm Scripts

### Building

- `npm run check` — Type check only (no emit)
- `npm run build:wasm` — Build WASM library to `pkg/`
- `npm run build:ts` — Compile TypeScript to `dist/main.js`
- `npm run build:dist` — Copy files to `dist/`
- `npm run build` — Run all build steps (wasm → ts → dist)
- `npm run clean` — Remove `dist/` and `pkg/`

### Running

- `npm run serve` — HTTP server on port 8080 (Web Serial won't work, HTTPS required)
- `npm run serve:https` — HTTPS server on port 8443 (requires cert)

## Building Separately

Build only TypeScript:

```bash
npm run build:ts
```

Build only WASM:

```bash
npm run build:wasm
```

Prepare dist folder:

```bash
npm run build:dist
```

## Serving the Example

### HTTP (limited, Web Serial won't work)

```bash
npm run serve
# Serves on http://localhost:8080 from dist/
```

Note: Web Serial API only works in secure contexts (HTTPS or certain localhost configurations).

### HTTPS (recommended for development)

```bash
npm run serve:https
# Serves on https://localhost:8443 from dist/
```

**First time setup:** Generate a self-signed certificate in `examples/web/`:

```bash
openssl req -x509 -newkey rsa:2048 -keyout key.pem -out cert.pem -days 365 -nodes \
  -subj "/CN=localhost"
```

You'll see a certificate warning in your browser—this is expected and safe for local development.

### Manual HTTPS (without npm scripts)

If npm scripts don't work:

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
