# rfidlibrs

Async Rust reader library for Silion RFID reader protocol communication.

This crate provides:
- Low-level packet build/parse helpers
- A transport-agnostic async client
- A high-level reader API (`SilionReader`) for common reader operations
- Async inventory session support
- Optional native serial transport (`tokio-serial`)
- Optional browser Web Serial + wasm-bindgen JS bindings

The protocol implementation follows Silion reader protocol docs:
https://en.silion.com.cn/En/doc_center/ModuleAPI_Docs/Communication_Protocol_Doc/html/Protocol_Introduction.html

## Crate Features

- `serial`: enables native serial transport via `tokio-serial`
- `web-serial`: enables wasm/browser support and JS bindings (for `wasm32` targets)

Default features are empty.

## Requirements

Core Rust development:
1. Rust toolchain (stable)

For browser/wasm workflows:
1. `wasm-pack`
2. Node.js + npm

Install wasm-pack:

```bash
cargo install wasm-pack
```

## Install

Add to your project:

```bash
cargo add rfidlibrs
```

Enable native serial support when needed:

```bash
cargo add rfidlibrs --features serial
```

## Quick Start

### Build protocol packets

```bash
cargo run --example build_packets
```

### High-level reader API with mock transport

```bash
cargo run --example high_level_host
```

### Native serial reader example

```bash
cargo run --features serial --example serial_query -- /dev/ttyUSB0 115200
```

### Minimal Rust API usage

```rust
use rfidlibrs::serial::SerialTransport;
use rfidlibrs::SilionReader;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let transport = SerialTransport::open("/dev/ttyUSB0", 115_200)?;
	let mut reader = SilionReader::new(transport);

	let version = reader.get_version().await?;
	println!("Firmware: {:02X?}", version.firmware_version);

	let temperature = reader.get_current_temperature().await?;
	println!("Temperature: {temperature} C");

	Ok(())
}
```

Build with the `serial` feature enabled.

## Browser and Web Serial

A browser demo is available in [examples/web/README.md](examples/web/README.md).

Build browser output directly:

```bash
wasm-pack build --target web --out-dir examples/web/pkg -- --features web-serial
```

Then serve:

```bash
cd examples/web
python3 -m http.server 8080
```

Open http://localhost:8080 in a browser with Web Serial support.

## Build npm Package from wasm

Use the helper script:

```bash
./scripts/build-npm-package.sh
```

What it does:
1. Reads version from [Cargo.toml](Cargo.toml)
2. Runs `wasm-pack build --target bundler --release -- --features web-serial`
3. Updates `pkg/package.json` to match Cargo version

Publish package:

```bash
npm publish ./pkg
```

## Validate Locally

```bash
cargo check
cargo test
cargo check --features serial
cargo check --target wasm32-unknown-unknown --features web-serial
```

## Project Layout

- [src/lib.rs](src/lib.rs): exports and crate-level docs
- [src/reader.rs](src/reader.rs): high-level async reader API
- [src/client.rs](src/client.rs): transport-level async client
- [src/session.rs](src/session.rs): async inventory session API
- [src/serial.rs](src/serial.rs): native serial transport (`serial` feature)
- [src/web_serial.rs](src/web_serial.rs): browser Web Serial transport (`web-serial` + wasm32)
- [src/web_bindings.rs](src/web_bindings.rs): wasm-bindgen JS bindings
- [examples/](examples/): runnable examples
