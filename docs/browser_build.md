# Browser (WASM) build – Battle Arena

Build the Battle Arena for the browser so you can run and test it in a WebGPU-capable browser. Useful for **AI agent testing** and automated browser-based tests.

## Prerequisites

- Rust toolchain with wasm target:
  ```bash
  rustup target add wasm32-unknown-unknown
  ```
- **wasm-bindgen-cli** (for JS glue and HTML integration):
  ```bash
  cargo install wasm-bindgen-cli
  ```
- A browser with **WebGPU** (Chrome 113+, Edge 113+, or Firefox with `dom.webgpu.enabled`).

## Build steps

```bash
# 1. Build the game for wasm (debug; use --release for smaller/faster)
cargo build --bin battle_arena --target wasm32-unknown-unknown

# 2. Generate JS glue and copy wasm into www/
wasm-bindgen --target web --out-dir www target/wasm32-unknown-unknown/debug/battle_arena.wasm

# 3. Serve the www/ directory (required for loading wasm; file:// often fails)
# Option A: Python
cd www && python -m http.server 8080

# Option B: Node (npx)
npx serve www -p 8080
```

Then open **http://localhost:8080** (or the URL shown) in a WebGPU-capable browser.

## Release build (smaller, faster)

```bash
cargo build --bin battle_arena --target wasm32-unknown-unknown --release
wasm-bindgen --target web --out-dir www target/wasm32-unknown-unknown/release/battle_arena.wasm
```

Use the same `www/` and serve it as above.

## Automation / agent testing

- The game runs in the browser like the desktop build (same controls).
- You can drive it with **Playwright**, **Puppeteer**, or similar by loading the same URL, injecting input, and taking screenshots or checking the DOM/console.
- WebGPU must be enabled in the browser context (Chrome/Edge usually have it on by default).

## Troubleshooting

- **"can't find crate for `core`"** → run `rustup target add wasm32-unknown-unknown`. If rustup says the target is installed but the error persists (common on Windows with multiple Rust installs), use rustup’s cargo by full path so the correct toolchain is used:
  - **PowerShell:** `& "$env:USERPROFILE\.cargo\bin\cargo.exe" build --bin battle_arena --target wasm32-unknown-unknown`
  - Ensure `%USERPROFILE%\.cargo\bin` is first in your PATH so `cargo`/`rustup` come from rustup, not e.g. Visual Studio.
- **Blank screen / no WebGPU** → use a supported browser and ensure WebGPU is not disabled.
- **CORS / file://** → serve `www/` over HTTP(S); avoid opening the HTML file directly from disk.
