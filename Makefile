PREFIX ?= /usr/local

.PHONY: build install uninstall test smoke clippy fmt clean

build:
	cargo build --release

install: build
	install -d $(DESTDIR)$(PREFIX)/bin
	install -d $(DESTDIR)$(PREFIX)/share/ai-usagebar
	install -d $(DESTDIR)$(PREFIX)/share/doc/ai-usagebar
	install -d $(DESTDIR)$(PREFIX)/share/licenses/ai-usagebar
	install -m755 target/release/ai-usagebar     $(DESTDIR)$(PREFIX)/bin/ai-usagebar
	install -m755 target/release/ai-usagebar-tui $(DESTDIR)$(PREFIX)/bin/ai-usagebar-tui
	install -m644 config.example.toml            $(DESTDIR)$(PREFIX)/share/ai-usagebar/config.example.toml
	install -m644 README.md                      $(DESTDIR)$(PREFIX)/share/doc/ai-usagebar/README.md
	install -m644 LICENSE                        $(DESTDIR)$(PREFIX)/share/licenses/ai-usagebar/LICENSE

uninstall:
	rm -f $(DESTDIR)$(PREFIX)/bin/ai-usagebar
	rm -f $(DESTDIR)$(PREFIX)/bin/ai-usagebar-tui
	rm -rf $(DESTDIR)$(PREFIX)/share/ai-usagebar
	rm -rf $(DESTDIR)$(PREFIX)/share/doc/ai-usagebar
	rm -rf $(DESTDIR)$(PREFIX)/share/licenses/ai-usagebar

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
