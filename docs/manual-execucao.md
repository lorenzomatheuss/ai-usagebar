# Manual de Execução: Torven (macOS & Rust)

Este guia descreve como configurar o ambiente e executar os componentes do Torven (Side Bar Swift + Core Rust).

## 1. Pré-requisitos (macOS)

### Xcode & Command Line Tools
sudo xcode-select -s /Applications/Xcode.app/Contents/Developer

### Ferramentas Adicionais
brew install xcodegen
rustup target add aarch64-apple-darwin

---

## 2. Executando o App Swift (Side Bar)
1. make build-app
2. open apple/Torven.xcodeproj
3. Cmd + R no Xcode (Esquema Torven > My Mac)

---

## 3. Executando Componentes Rust
cargo run -p torven-tui
cargo run -p torven -- --vendor openai
