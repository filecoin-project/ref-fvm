HEADERS=libcgobs_example.h
LIBS=libcgobs_example.a
CARGO_TARGET_DIR ?= "../../target"

.PHONY: bench
bench: $(HEADERS) $(LIBS)
	cd owner && go test -ldflags=-extldflags=../libcgobs_example.a -bench .

libcgobs_example.h: libcgobs_example.a
	cd user && cbindgen -l C > ../$@

.PHONY: libcgobs_example.a
libcgobs_example.a:
	cd user && cargo build
	cp "$(CARGO_TARGET_DIR)/debug/libcgobs_example.a" .
