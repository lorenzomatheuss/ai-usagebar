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
