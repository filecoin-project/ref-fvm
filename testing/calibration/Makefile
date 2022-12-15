SCRIPTS_DIR := ./measurements/scripts
OUT_DIR     := ./measurements/out

OBS_DIR  := $(OUT_DIR)/observations
OBS_FIND := find $(OBS_DIR) -type f \( -name "*.jsonline" \)
OBS_JSON := $(shell $(OBS_FIND))
OBS_PNG  := $(patsubst $(OUT_DIR)/observations/%.jsonline, $(OUT_DIR)/charts/charges/%.png, $(OBS_JSON))

RUN_BINS := $(patsubst src/bin/%.rs, run/%, $(shell find src/bin -type f))

GAS_MILLIS_PER_NS := 10000

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
	$(OBS_PNG) \
	$(OUT_DIR)/charts/overall.png

$(OUT_DIR)/charts/charges/%.png: \
		$(OUT_DIR)/observations/%.jsonline \
		$(SCRIPTS_DIR)/observations.sh \
		$(SCRIPTS_DIR)/observations.plt \
		| jq gnuplot
	$(SCRIPTS_DIR)/observations.sh $< $@

$(OUT_DIR)/charts/overall.png: \
		$(OBS_JSON) \
		$(SCRIPTS_DIR)/overall.sh \
		$(SCRIPTS_DIR)/overall.plt \
		| jq gnuplot
	$(SCRIPTS_DIR)/overall.sh $(OBS_DIR) $@


# This is just a convenience tool to convert regression output into milligas prices.
# One should always look at the charts and the regressions to decide which one looks
# reasonable, and where can for example the base cost be ignored.
# Alternatively there could be more sophisticated statistics software used that
# calculates the P-value of the intercept and the slope separately.
# The results are in millis, to be used with `Gas::from_milligas`, not `Gas::new`.
.PHONY: proposals
proposals:
	@for FILE in measurements/out/regressions/*.jsonline ; do \
		cat $$FILE | jq -c "{ \
			name: \"$$(basename $$FILE .jsonline)\", \
			label: .label, \
			base_cost: (.intercept * $(GAS_MILLIS_PER_NS) | ceil), \
			variable_cost: (.slope * $(GAS_MILLIS_PER_NS) | ceil) \
		}" ; \
	done


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
