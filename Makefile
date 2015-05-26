.PHONY: all tests
all:
	@echo > /dev/null

tests:
	cargo test --verbose
	cargo test --features texture_surface --verbose
