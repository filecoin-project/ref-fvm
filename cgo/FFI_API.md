Lotus -> FVM: Create a Machine.
    + Input
        - Blockstore
        - state tree root CID
        - epoch
        - basefee
        - initial state root
        - network version
    + Output
        - VM on which any number of messages can be executed

Lotus -> FVM: Machine.execute_message
    + Input
        - Message
        - Whether message is explicit or implicit (cron / reward)
    + Output
        - MessageReceipt (exitcode, return value, gasused) -- this is consensus critical
        - ActorErr
            * Clarify: This needs to only be non-fatal errors (fatal errors should lead to the execute_message call itself erroring)
        - Miner Penalty
        - Miner Reward
        - (optional) Execution Trace
        - (optional) Detailed Gas Costs
        - (optional) Execution time

FVM -> Lotus: Randomness
    + Input
        - Chain or Beacon (could be 2 separate calls)
        - NetworkVersion (may not be needed)
        - Round
        - DomainSeparationTag
        - Entropy
    + Output
        - Byte array (randomness)

// We should really try to obviate the need for this by landing this in the next upgrade, but we still have to support it for mainnet testing
// https://github.com/filecoin-project/actors-private/issues/18
FVM -> Lotus: CircSupply
    + Input
        - The very latest state tree
    + Output
        - CircSupply

FVM -> Lotus: VerifyConsensusFault
    + Input
        - faulty blocks
    + Output
        - Faulty Miner
        - Fault-causing epoch
        - Fault Type

Lotus -> FVM: Machine.finish
    + Input
        - None
    + Output
        - New state root
