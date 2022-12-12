SCRIPTS_DIR := ./scripts
OUT_DIR     := ./out

TRACES_DIR  := ../traces
TRACES_FIND := find $(TRACES_DIR) -type f \( -name "*.jsonline" -a -not -name "traces.jsonline" \)
TRACES_JSON := $(shell $(TRACES_FIND))

TRACES_MERGED := $(OUT_DIR)/traces.merged.jsonline
TRACES_SUMMED := $(OUT_DIR)/traces.summed.jsonline

CHARGES_DIR := $(OUT_DIR)/charges
CHARGES_TXT := $(OUT_DIR)/charges.txt
CHARGES     := $(shell cat $(CHARGES_TXT) || echo "")
CHARGES_TVG_PNG        := $(patsubst %, $(CHARGES_DIR)/%.time-vs-gas.png, $(CHARGES))
CHARGES_TVG_SUMMED_PNG := $(patsubst %, $(CHARGES_DIR)/%.time-vs-gas.summed.png, $(CHARGES))

OVERALL_DIR     := $(OUT_DIR)/overall
OVERALL_TVG_PNG := $(OVERALL_DIR)/time-vs-gas.png


.PHONY: all
all:
	$(MAKE) prepare
	$(MAKE) visualize
	$(MAKE) regression

.PHONY: prepare
prepare: \
	$(CHARGES_TXT)

.PHONY: charts
visualize: \
	$(OVERALL_TVG_PNG) \
	$(CHARGES_TVG_PNG) \
	$(CHARGES_TVG_SUMMED_PNG) \

.PHONY: regression
regression: $(OUT_DIR)/regression.jsonline


# Concatenate all traces as-is
$(TRACES_MERGED): $(TRACES_JSON)
	rm -rf $@
	for JSON in $$($(TRACES_FIND)); do \
		cat $$JSON >> $@; \
	done

# Aggregate charges in each trace, so we have a single line per charge from each test vector
# then concatenate them all into one.
$(TRACES_SUMMED): $(TRACES_JSON)
	rm -rf $@
	for JSON in $$($(TRACES_FIND)); do \
		cat $$JSON | \
		jq --slurp -c "group_by(.name) | map({ name: .[0].name, elapsed_nanos: map(.elapsed_nanos) | add, compute_gas: map(.compute_gas) | add, storage_gas: map(.storage_gas) | add }) | .[]" \
		>> $@; \
	done


# Collect all unique charge names
$(CHARGES_TXT): $(TRACES_MERGED) | jq
	cat $< | jq -r "select(.elapsed_nanos != null) | .name" | sort | uniq > $@


# Show time vs gas on the level of test vectors.
$(OVERALL_TVG_PNG): \
		$(TRACES_DIR)/traces.jsonline \
		$(SCRIPTS_DIR)/overall/time-vs-gas.sh \
		$(SCRIPTS_DIR)/overall/time-vs-gas.plt \
		| jq gnuplot
	$(SCRIPTS_DIR)/overall/time-vs-gas.sh $< $(OVERALL_DIR)


# Show time vs gas on the level of individual charges.
$(CHARGES_DIR)/%.time-vs-gas.png: \
		$(TRACES_MERGED) \
		$(SCRIPTS_DIR)/charges/time-vs-gas.sh \
		$(SCRIPTS_DIR)/charges/time-vs-gas.plt \
		| jq gnuplot
	$(SCRIPTS_DIR)/charges/time-vs-gas.sh $< $(CHARGES_DIR) time-vs-gas $*


# Show time vs gas on the level of charges aggregated within test vectors.
$(CHARGES_DIR)/%.time-vs-gas.summed.png: \
		$(TRACES_SUMMED) \
		$(SCRIPTS_DIR)/charges/time-vs-gas.sh \
		$(SCRIPTS_DIR)/charges/time-vs-gas.plt \
		| jq gnuplot
	$(SCRIPTS_DIR)/charges/time-vs-gas.sh $< $(CHARGES_DIR)	time-vs-gas.summed $*


# Least Squares regression on the aggregate level.
$(OUT_DIR)/regression.jsonline: \
		$(TRACES_SUMMED) \
		../src/bin/least-squares.rs
	cd .. && cargo run --bin least-squares -- measurements/$< measurements/$@


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
