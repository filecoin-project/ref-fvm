SCRIPTS_DIR := ./measurements/scripts
OUT_DIR     := ./measurements/out

OBS_DIR  := $(OUT_DIR)/observations
OBS_FIND := find $(OBS_DIR) -type f \( -name "*.jsonline" \)
OBS_JSON := $(shell $(OBS_FIND))
OBS_PNG  := $(patsubst $(OUT_DIR)/observations/%.jsonline, $(OUT_DIR)/charts/%.png, $(OBS_JSON))


all: run visualize


.PHONY: run
run: \
	run/hashing

run/%:
	cargo run --release --bin $*


.PHONY: visualize
visualize: \
	$(OBS_PNG)


$(OUT_DIR)/charts/%.png: \
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
