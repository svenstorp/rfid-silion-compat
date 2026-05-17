#!/bin/bash
set -e

if ! command -v wasm-pack >/dev/null 2>&1; then
  echo "Error: wasm-pack is not installed."
  echo "Install it with: cargo install wasm-pack"
  exit 1
fi

if ! command -v node >/dev/null 2>&1; then
  echo "Error: node is not installed."
  echo "Install Node.js (includes npm) and re-run this script."
  exit 1
fi

# Extract version from Cargo.toml
CARGO_TOML="Cargo.toml"
VERSION=$(grep '^version' "$CARGO_TOML" | head -1 | sed 's/version = "\(.*\)"/\1/')

if [ -z "$VERSION" ]; then
  echo "Error: Could not extract version from Cargo.toml"
  exit 1
fi

echo "Building npm package for rfid-silion-compat v$VERSION..."

# Run wasm-pack to build the bundler target (suitable for Node.js and bundlers)
wasm-pack build --target bundler --release -- --features web-serial

# Copy npm-focused README into generated package output.
NPM_README_SOURCE="npm/README.md"
NPM_README_TARGET="pkg/README.md"
if [ -f "$NPM_README_SOURCE" ]; then
  cp "$NPM_README_SOURCE" "$NPM_README_TARGET"
  echo "Copied npm README to $NPM_README_TARGET"
else
  echo "Warning: $NPM_README_SOURCE not found; package README was not updated"
fi

# Copy license files into generated npm package output.
for LICENSE_FILE in LICENSE-MIT LICENSE-APACHE; do
  if [ -f "$LICENSE_FILE" ]; then
    cp "$LICENSE_FILE" "pkg/$LICENSE_FILE"
  else
    echo "Warning: $LICENSE_FILE not found; npm package will miss this license file"
  fi
done

# Update package.json with the version from Cargo.toml
PACKAGE_JSON="pkg/package.json"
if [ -f "$PACKAGE_JSON" ]; then
  # Use Node.js to update the version field (more reliable than sed)
  node -e "
    const fs = require('fs');
    const pkg = JSON.parse(fs.readFileSync('$PACKAGE_JSON', 'utf8'));
    pkg.version = '$VERSION';
    pkg.license = '(MIT OR Apache-2.0)';
    pkg.description = 'WebAssembly bindings for the Silion RFID reader API (Web Serial).';
    pkg.repository = {
      type: 'git',
      url: 'git+https://github.com/svenstorp/rfid-silion-compat.git'
    };
    pkg.homepage = 'https://github.com/svenstorp/rfid-silion-compat';
    pkg.bugs = {
      url: 'https://github.com/svenstorp/rfid-silion-compat/issues'
    };
    pkg.keywords = ['rfid', 'uhf', 'webserial', 'wasm'];
    fs.writeFileSync('$PACKAGE_JSON', JSON.stringify(pkg, null, 2) + '\n');
  "
  echo "Updated package.json metadata (version, license, repository, homepage, bugs, keywords)"
else
  echo "Warning: pkg/package.json not found"
  exit 1
fi

echo ""
echo "✓ npm package built successfully!"
echo ""
echo "Package location: ./pkg"
echo "Package version:  $VERSION"
echo ""
echo "Next steps:"
echo "  npm publish ./pkg              # Publish to npm registry"
echo "  npm install ./pkg              # Install locally"
echo ""
