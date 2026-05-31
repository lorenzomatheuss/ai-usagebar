PREFIX ?= /usr/local

.PHONY: build install uninstall test smoke clippy fmt clean

build:
	cargo build --release

install: build
	install -d $(DESTDIR)$(PREFIX)/bin
	install -d $(DESTDIR)$(PREFIX)/share/torven
	install -d $(DESTDIR)$(PREFIX)/share/doc/torven
	install -d $(DESTDIR)$(PREFIX)/share/licenses/torven
	install -m755 target/release/torven     $(DESTDIR)$(PREFIX)/bin/torven
	install -m755 target/release/torven-tui $(DESTDIR)$(PREFIX)/bin/torven-tui
	install -m644 config.example.toml            $(DESTDIR)$(PREFIX)/share/torven/config.example.toml
	install -m644 README.md                      $(DESTDIR)$(PREFIX)/share/doc/torven/README.md
	install -m644 LICENSE                        $(DESTDIR)$(PREFIX)/share/licenses/torven/LICENSE

uninstall:
	rm -f $(DESTDIR)$(PREFIX)/bin/torven
	rm -f $(DESTDIR)$(PREFIX)/bin/torven-tui
	rm -rf $(DESTDIR)$(PREFIX)/share/torven
	rm -rf $(DESTDIR)$(PREFIX)/share/doc/torven
	rm -rf $(DESTDIR)$(PREFIX)/share/licenses/torven

test:
	cargo test

smoke:
	@echo "Running live API smoke tests (requires creds in shell env)..."
	cargo test --test live -- --ignored --nocapture

clippy:
	cargo clippy --all-targets -- -D warnings

fmt:
	cargo fmt

clean:
	cargo clean

# === Torven Apple/Rust build targets ===

.PHONY: build-core build-app build-all clean-apple help-apple

# Build Rust core as staticlib (aarch64 Apple Silicon)
build-core:
	cargo build --release --target aarch64-apple-darwin -p torven-core

# Generate Xcode project from project.yml and build the macOS app (Debug)
build-app:
	cd apple && xcodegen generate
	cd apple && xcodebuild -scheme Torven -configuration Debug build

# Full build: core first (xcframework needed by app), then app
build-all: build-core build-app

# Clean both Cargo target/ and Xcode generated artifacts
clean-apple:
	cargo clean
	rm -rf apple/Torven.xcodeproj apple/Frameworks apple/build apple/DerivedData

help-apple:
	@echo "Torven Apple/Rust targets:"
	@echo "  build-core   - cargo build torven-core (release, aarch64-apple-darwin)"
	@echo "  build-app    - xcodegen generate + xcodebuild scheme Torven (Debug)"
	@echo "  build-all    - build-core then build-app"
	@echo "  clean-apple  - cargo clean + remove apple/Torven.xcodeproj, Frameworks, build"
