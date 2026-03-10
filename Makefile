BIN_NAME := clp
INSTALL_DIR := $(HOME)/bin

.PHONY: build release install uninstall clean

build:
	cargo build

release:
	cargo build --release

install: release
	@mkdir -p $(INSTALL_DIR)
	cp target/release/$(BIN_NAME) $(INSTALL_DIR)/$(BIN_NAME)
	@echo "Installed to $(INSTALL_DIR)/$(BIN_NAME)"
	@echo "Make sure $(INSTALL_DIR) is in your PATH."

uninstall:
	rm -f $(INSTALL_DIR)/$(BIN_NAME)
	@echo "Removed $(INSTALL_DIR)/$(BIN_NAME)"

clean:
	cargo clean
