use serde_tuple::*;
use crate::smooth::FilterEstimate;

#[derive(Serialize_tuple, Deserialize_tuple)]
pub struct DeferredCronEventParams {
    #[serde(with = "serde_bytes")]
    pub event_payload: Vec<u8>,
    pub reward_smoothed: FilterEstimate,
    pub quality_adj_power_smoothed: FilterEstimate,
}
