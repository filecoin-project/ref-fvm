SCRIPTS_DIR := ./scripts
OUT_DIR     := ./out
TRACES_DIR  := ../traces

.PHONY: all
all: \
	${OUT_DIR}/overall-time-vs-gas.png

${OUT_DIR}/overall-time-vs-gas.png: \
		${TRACES_DIR}/traces.jsonline \
		${SCRIPTS_DIR}/overall-time-vs-gas.sh \
		${SCRIPTS_DIR}/overall-time-vs-gas.plt \
		| jq gnuplot
	${SCRIPTS_DIR}/overall-time-vs-gas.sh $< ${OUT_DIR}




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
