RUSTFLAGS="-C target-feature=+crt-static"

all:

.PHONY: all

build:
	cargo build
.PHONY: build

example-actor:
	$(MAKE) -C ./examples/actor build
.PHONY: example-actor

example-fvm: example-actor
	$(MAKE) -C ./examples/fvm build
.PHONY: example-fvm

examples: example-actor example-fvm