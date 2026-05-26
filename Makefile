# xerotop — convenience wrapper around cargo.
# The release binary lives at target/release/xerotop; `make install` symlinks it
# onto your PATH (~/.local/bin) so `xerotop` and the labwc menu pick up rebuilds.

PREFIX  ?= $(HOME)/.local
BINDIR  ?= $(PREFIX)/bin
BIN      = target/release/xerotop

.PHONY: build run restart install uninstall debug check fmt clean

# Default: optimized build. The symlink means this alone updates `xerotop`.
build:
	cargo build --release

# Build, then launch in the foreground.
run: build
	./$(BIN)

# Rebuild and hot-swap the running bar (same as the labwc "XeroTop" menu item).
restart: build
	-pkill -x xerotop
	@sleep 0.4
	setsid ./$(BIN) >/dev/null 2>&1 < /dev/null &
	@echo "xerotop restarted"

# Symlink the release binary onto PATH (idempotent).
install: build
	mkdir -p $(BINDIR)
	ln -sf $(CURDIR)/$(BIN) $(BINDIR)/xerotop
	@echo "linked $(BINDIR)/xerotop -> $(CURDIR)/$(BIN)"

uninstall:
	rm -f $(BINDIR)/xerotop

# Fast unoptimized build for iterating on logic.
debug:
	cargo build

check:
	cargo check

fmt:
	cargo fmt

clean:
	cargo clean
