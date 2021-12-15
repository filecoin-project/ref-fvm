use fvm_shared::encoding::tuple::*;
use fvm_shared::sector::SectorNumber;

pub mod init {
    use super::*;

    pub const EXEC_METHOD: u64 = 2;

    /// Init actor Exec Params
    #[derive(Serialize_tuple, Deserialize_tuple)]
    pub struct ExecParams {
        pub code_cid: Cid,
        pub constructor_params: RawBytes,
    }

    /// Init actor Exec Return value
    #[derive(Serialize_tuple, Deserialize_tuple)]
    pub struct ExecReturn {
        /// ID based address for created actor
        pub id_address: Address,
        /// Reorg safe address for actor
        pub robust_address: Address,
    }
}

pub mod miner {
    use super::*;

    pub const CONFIRM_SECTOR_PROOFS_VALID_METHOD: u64 = 17;

    #[derive(Serialize_tuple, Deserialize_tuple)]
    pub struct ConfirmSectorProofsParams {
        pub sectors: Vec<SectorNumber>,
    }
}

pub mod reward {
    use super::*;

    pub const UPDATE_NETWORK_KPI: u64 = 4;
}
