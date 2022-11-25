SCRIPTS_DIR := ./scripts
OUT_DIR     := ./out

TRACES_DIR  := ../traces
TRACES_FIND := find $(TRACES_DIR) -type f \( -name "*.jsonline" -a -not -name "traces.jsonline" \)
TRACES_JSON := $(shell $(TRACES_FIND))

TRACES_MERGED := $(OUT_DIR)/traces.merged.jsonline

CHARGES_DIR := $(OUT_DIR)/charges
CHARGES_TXT := $(OUT_DIR)/charges.txt
CHARGES     := $(shell cat $(CHARGES_TXT) || echo "")
CHARGES_TIME_VS_GAS_PNG := $(patsubst %, $(CHARGES_DIR)/%.time-vs-gas.png, $(CHARGES))

OVERALL_DIR := $(OUT_DIR)/overall
OVERALL_TIME_VS_GAS_PNG := $(OVERALL_DIR)/time-vs-gas.png


.PHONY: all
all:
	$(MAKE) prepare
	$(MAKE) visualize

.PHONY: prepare
prepare: \
	$(CHARGES_TXT)

.PHONY: charts
visualize: \
	$(OVERALL_TIME_VS_GAS_PNG) \
	$(CHARGES_TIME_VS_GAS_PNG)

$(TRACES_MERGED): $(TRACES_JSON)
	rm -rf $@
	for JSON in $$($(TRACES_FIND)); do \
		cat $$JSON >> $@; \
	done

$(CHARGES_TXT): $(TRACES_MERGED) | jq
	cat $< | jq -r "select(.elapsed_nanos != null) | .name" | sort | uniq > $@

$(OVERALL_TIME_VS_GAS_PNG): \
		$(TRACES_DIR)/traces.jsonline \
		$(SCRIPTS_DIR)/overall/time-vs-gas.sh \
		$(SCRIPTS_DIR)/overall/time-vs-gas.plt \
		| jq gnuplot
	$(SCRIPTS_DIR)/overall/time-vs-gas.sh $< $(OVERALL_DIR)


$(CHARGES_DIR)/%.time-vs-gas.png: \
		$(TRACES_MERGED) \
		$(SCRIPTS_DIR)/charges/time-vs-gas.sh \
		$(SCRIPTS_DIR)/charges/time-vs-gas.plt \
		| jq gnuplot
	$(SCRIPTS_DIR)/charges/time-vs-gas.sh $< $(CHARGES_DIR)	$*

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
