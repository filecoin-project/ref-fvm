RUSTFLAGS="-C target-feature=+crt-static"

# Detect OS
OS := $(shell uname | tr "[:upper:]" "[:lower:]")
ARCH := $(shell uname -m | tr "[:upper:]" "[:lower:]")
GOPATH ?= $(shell go env GOPATH)
GOFLAGS ?= $(GOFLAGS:)
GO=go
GO_MAJOR_VERSION = $(shell $(GO) version | cut -c 14- | cut -d' ' -f1 | cut -d'.' -f1)
GO_MINOR_VERSION = $(shell $(GO) version | cut -c 14- | cut -d' ' -f1 | cut -d'.' -f2)
GO_OS = $(shell $(GO) version | cut -c 14- | cut -d' ' -f2 | cut -d'/' -f1 | tr "[:upper:]" "[:lower:]")
GO_ARCH = $(shell $(GO) version | cut -c 14- | cut -d' ' -f2 | cut -d'/' -f2 | tr "[:upper:]" "[:lower:]")

define GO_MISMATCH_ERROR

Your go binary does not match your architecture.
	Go binary:    $(GO_OS) - $(GO_ARCH)
	Environment:  $(OS) - $(ARCH)
	GOPATH:       $(GOPATH)

endef
export GO_MISMATCH_ERROR

all: go-arch-alignment build examples
.PHONY: all

build:
	cargo build
.PHONY: build

go-arch-alignment:
mismatch = 
ifneq ($(OS), $(GO_OS))
mismatch = yes
endif 
ifneq ($(ARCH), $(GO_ARCH))
mismatch = yes
endif
ifdef mismatch
$(info $(GO_MISMATCH_ERROR))
$(error Please change your go binary)
endif
.PHONY: go-arch-alignment

#examples: example-actor example-fvm example-blockstore-cgo
# take the fvm examples out of the build tree; the examples will be superseded
# by tests
examples: example-actor example-blockstore-cgo
.PHONY: examples

example-actor:
	$(MAKE) -C ./examples/actor build
.PHONY: example-actor

example-fvm: example-actor
	$(MAKE) -C ./examples/fvm build
.PHONY: example-fvm

example-blockstore-cgo:
	$(MAKE) -C ./examples/blockstore-cgo
.PHONY: example-blockstore-cgo