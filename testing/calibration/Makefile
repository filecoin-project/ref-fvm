SCRIPTS_DIR := ./measurements/scripts
OUT_DIR     := ./measurements/out

OBS_DIR  := $(OUT_DIR)/observations
OBS_FIND := find $(OBS_DIR) -type f \( -name "*.jsonline" \)
OBS_JSON := $(shell $(OBS_FIND))
OBS_PNG  := $(patsubst $(OUT_DIR)/observations/%.jsonline, $(OUT_DIR)/charts/charges/%.png, $(OBS_JSON))

RUN_BINS := $(patsubst src/bin/%.rs, run/%, $(shell find src/bin -type f))


.PHONY: all
all:
	$(MAKE) run
	$(MAKE) visualize


.PHONY: run
run: $(RUN_BINS)

run/%:
	cargo run --release --bin $*


.PHONY: visualize
visualize: \
	$(OBS_PNG)


$(OUT_DIR)/charts/charges/%.png: \
		$(OUT_DIR)/observations/%.jsonline \
		$(SCRIPTS_DIR)/observations.sh \
		$(SCRIPTS_DIR)/observations.plt \
		| jq gnuplot
	$(SCRIPTS_DIR)/observations.sh $< $@


.PHONY: gnuplot
gnuplot:
	@if [ -z "$(shell which gnuplot)" ]; then \
		echo "Please install gnuplot. See http://www.gnuplot.info/"; \
		exit 1; \
	fi

.PHONY: jq
jq:
	@if [ -z "$(shell which jq)" ]; then \
		echo "Please install jq. See https://stedolan.github.io/jq/"; \
		exit 1; \
	fi
