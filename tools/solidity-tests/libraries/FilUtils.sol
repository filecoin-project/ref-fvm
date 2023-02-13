// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.0;

library FilUtils {

    // keccak([])
    bytes32 constant EVM_EMPTY_CODEHASH = 0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470;
    // keccak([0xFE])
    bytes32 constant FIL_NATIVE_CODEHASH = 0xbcc90f2d6dada5b18e155c17a1c0a55920aae94f39857d39d0d8ed07ae8f228b;

    // bytes20 constant NULL = 0x0000000000000000000000000000000000000000;

    bytes22 constant F4_ADDR_EXAMPLE = 0x040Aff00000000000000000000000000000000000001;

    // FIL BUILTIN ACTORS
    address constant SYSTEM_ACTOR = 0xfF00000000000000000000000000000000000000;
    address constant INIT_ACTOR = 0xff00000000000000000000000000000000000001;
    address constant REWARD_ACTOR = 0xff00000000000000000000000000000000000002;
    address constant CRON_ACTOR = 0xFF00000000000000000000000000000000000003;
    address constant POWER_ACTOR = 0xFf00000000000000000000000000000000000004;
    address constant MARKET_ACTOR = 0xff00000000000000000000000000000000000005;
    address constant VERIFIED_REGISTRY_ACTOR = 0xFF00000000000000000000000000000000000006;
    address constant DATACAP_TOKEN_ACTOR = 0xfF00000000000000000000000000000000000007;
    address constant EAM_ACTOR = 0xfF0000000000000000000000000000000000000a;
    // address constant CHAOS_ACTOR = 0xFF00000000000000000000000000000000000000; // 98
    // address constant BURNT_FUNDS_ACTOR = 0xFF00000000000000000000000000000000000000; // 99

    // FIL Precompiles
    address constant RESOLVE_ADDR = 0xFE00000000000000000000000000000000000001;
    address constant LOOKUP_DELEGATED_ADDR = 0xfE00000000000000000000000000000000000002;
    address constant CALL_ACTOR = 0xfe00000000000000000000000000000000000003;
    // address constant GET_ACTOR_TYPE = 0xFe00000000000000000000000000000000000004;
    address constant CALL_ACTOR_BY_ID = 0xfe00000000000000000000000000000000000005;

    uint64 constant MAX_RESERVED_METHOD = 1023;
    bytes4 constant NATIVE_METHOD_SELECTOR = 0x868e10c4;

    uint64 constant DEFAULT_FLAG = 0x00000000;
    uint64 constant READONLY_FLAG = 0x00000001;

    function callActor(
        uint64 _id, 
        uint64 _method, 
        uint _value, 
        uint64 _codec, 
        bytes memory _data
    ) internal returns (bool, bytes memory) {
        return callHelper(false, _id, _method, _value, _codec, _data);
    }

    function callActorReadonly(
        uint64 _id,
        uint64 _method,
        uint64 _codec,
        bytes memory _data
    ) internal view returns (bool, bytes memory) {
        function(bool, uint64, uint64, uint, uint64, bytes memory) internal view returns (bool, bytes memory) callFn;
        function(bool, uint64, uint64, uint, uint64, bytes memory) internal returns (bool, bytes memory) helper = callHelper;
        assembly { callFn := helper }
        return callFn(true, _id, _method, 0, _codec, _data);
    }

    function callHelper(
        bool _readonly,
        uint64 _id, 
        uint64 _method, 
        uint _value, 
        uint64 _codec, 
        bytes memory _data
    ) private returns (bool, bytes memory) {
        uint64 flags = _readonly ? READONLY_FLAG : DEFAULT_FLAG;
        require(!_readonly || _value == 0); // sanity check - shouldn't hit this in a private method
        bytes memory input = abi.encode(_method, _value, flags, _codec, _data, _id);
        return CALL_ACTOR_BY_ID.delegatecall(input);
    }

    /**
     * Checks whether a given address is an ID address. If it is, the ID is returned.
     * An ID address is defined as:
     * [0xFF] [bytes11(0)] [uint64(id)]
     */
    function isIDAddress(address _a) internal pure returns (bool isID, uint64 id) {
        uint64 ID_MASK = type(uint64).max;
        address system = SYSTEM_ACTOR;
        assembly {
            let id_temp := and(_a, ID_MASK) // Last 8 bytes of _a are the ID
            let a_mask := and(_a, not(id_temp)) // Zero out the last 8 bytes of _a
            // _a is an ID address if we zero out the last 8 bytes and it's equal to
            // the SYSTEM_ACTOR addr, which is an ID address where ID is 0.
            if eq(a_mask, system) {
                isID := true
                id := id_temp
            }
        }
    }

    /**
     * Given an Actor ID, converts it to an EVM-compatible ID address. See
     * above for ID address definition.
     */
    function toIDAddress(uint64 _id) internal pure returns (address addr) {
        assembly { addr := or(SYSTEM_ACTOR, _id) }
    }

    // function getEthAddress(uint64 _id) internal view returns (bool success, address eth) {
    //     bytes memory data = abi.encodePacked(_id);
    //     (success, data) = LOOKUP_DELEGATED_ADDR.staticcall(data);
        
    //     // If we reverted the ID does not have a corresponding Eth address.
    //     if (!success) {
    //         return (false, address(0));
    //     }

    //     (success, eth) = fromF4Address(data);
    // }

    /**
     * Given an Actor id, queries LOOKUP_DELEGATED_ADDRESS precompile to try to convert
     * it to an Eth address. If the id does not have an associated Eth address, this
     * returns (false, 0x00)
     */
    function getEthAddress(uint64 _id) internal view returns (bool success, address eth) {
        uint160 ADDRESS_MASK = type(uint160).max;
        assembly {
            mstore(0, _id)
            // LOOKUP_DELEGATED_ADDR returns an f4-encoded address. For Eth addresses,
            // this looks like the 20-byte address, prefixed with 0x040A.
            // So, our return size is 22 bytes.
            success := staticcall(gas(), LOOKUP_DELEGATED_ADDR, 0, 0x20, 0x20, 22)
            let result := mload(0x20)
            eth := and(
                shr(80, result),
                ADDRESS_MASK
            )
            // Sanity-check f4 prefix - should be 0x040A prepended to address
            let prefix := shr(240, result)
            if iszero(eq(prefix, 0x040A)) {
                success := false
                eth := 0
            }
        }
        if (!success || returnSize() != 22) {
            return (false, address(0));
        }
    }

    /**
     * Given an Eth address, queries RESOLVE_ADDR precompile to look up the corresponding
     * ID address. 
     * If there is no corresponding ID address, this returns (false, 0)
     * If the address passed in is already an ID address, returns (true, id)
     */
    function getActorID(address _eth) internal view returns (bool success, uint64 id) {
        // If we were passed an ID address already, just return it
        (success, id) = isIDAddress(_eth);
        if (success) { 
            return(success, id); 
        }

        assembly {
            // Convert EVM address to f4-encoded format:
            // 22 bytes, prepended with:
            // * protocol  (0x04) - "f4" address
            // * namespace (0x0A) - "10" for the EAM actor
            _eth := or(
                shl(240, 0x040A),
                shl(80, _eth)
            )
            mstore(0, _eth)
            success := staticcall(gas(), RESOLVE_ADDR, 0, 22, 0, 0x20)
            id := mload(0)
        }
        // If we got empty return data or the call reverted, return (false, 0)
        if (!success || returnSize() == 0) {
            return (false, 0);
        }
    }

    function returnSize() internal pure returns (uint size) {
        assembly { size := returndatasize() }
    }
}