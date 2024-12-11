TARGET_DIR ?= ~/.task/hooks/
PROJECT_DIR = on-exit-hook-waybar

# The name of the resulting binary (default is the package name in Cargo.toml)
BINARY_NAME = on-exit-hook-waybar

.PHONY: all build install clean

all: build install

build:
	@echo "Building the Rust project..."
	@cargo build --release --manifest-path $(PROJECT_DIR)/Cargo.toml

install: build
	@echo "Copying the binary to $(TARGET_DIR)..."
	@mkdir -p $(TARGET_DIR)
	@cp $(PROJECT_DIR)/target/release/$(BINARY_NAME) $(TARGET_DIR)

clean:
	@echo "Cleaning up build artifacts..."
	@cargo clean --manifest-path $(PROJECT_DIR)/Cargo.toml
