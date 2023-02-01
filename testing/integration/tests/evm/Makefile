CONTRACTS_DIR = contracts
ARTIFACTS_DIR = artifacts
CONTRACTS_SOL = $(shell find $(CONTRACTS_DIR) -type f -name "*.sol")
CONTRACTS_ART = $(patsubst $(CONTRACTS_DIR)/%.sol, $(ARTIFACTS_DIR)/%.sol, $(CONTRACTS_SOL))

.PHONY: all
all: \
	compile-contracts

# Compile all Solidity test contracts.
# This is normally done with `build.rs`, but it looks like if the contract has problems
# the ethers tool will silently do nothing, so I included this as a fallback to investigate.
.PHONY: compile-contracts
compile-contracts: $(CONTRACTS_ART)

# Compile all contracts in a Solidity file
$(ARTIFACTS_DIR)/%.sol: $(CONTRACTS_DIR)/%.sol | solc
	echo compiling...
	solc --bin --abi --overwrite $< -o $@
	for f in $@/*.bin; do echo $$f; mv $$f $@/$$(basename $$f .bin).hex; done

# Remove artifacts then run the build.
.PHONY: compile-contracts
force-regen:
	rm -rf artifacts cache
	touch build.rs
	cargo build --release

# Requirements checks.

.PHONY: solc
solc:
	@if [ -z "$(shell which solc)" ]; then \
		echo "Please install solc, the Solidity compiler. See https://github.com/crytic/solc-select"; \
		exit 1; \
	fi
