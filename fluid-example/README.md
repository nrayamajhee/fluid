## 🚴 Usage

### 📦 Prerequisites  

```
cargo install cargo-generate cargo-watch wasm-bindgen-cli
```

### 🐑 Use `cargo generate` to Clone this Template
```
cargo generate --git https://github.com/nrayamajhee/wasm-template.git --name my-project
cd my-project
chomod +x build
```

### 🛠️ Build and serve:

```
./build build
./build serve
```

```
./build watch
```

will run cargo watch and build the wasm file.

### 📖 Generate documentation with:

```
./build docs
```

This runs `cargo doc --document-private-items --open`.

### 🔬 Test in Headless Browsers with `wasm-pack test`

```
wasm-pack test --headless --firefox
```
