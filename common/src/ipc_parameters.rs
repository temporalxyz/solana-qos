//! Given the circular IPC topology, we define these constants to allow
//! for preallocation of all huge pages prior to running any qos process
//! or thread.
//!
//! RE1 and RE2 are relayer 1 (TPU) and relayer 2 (TPU FWD)

pub const IPC_QOS_TO_SIG_CAP: usize = 32768;
pub const IPC_TPU_TO_QOS_CAP: usize = 32768;
pub const IPC_FWD_TO_QOS_CAP: usize = IPC_TPU_TO_QOS_CAP; // needs to be the same!
pub const IPC_RE1_TO_QOS_CAP: usize = IPC_TPU_TO_QOS_CAP; // needs to be the same!
pub const IPC_RE2_TO_QOS_CAP: usize = IPC_TPU_TO_QOS_CAP; // needs to be the same!
pub const IPC_SIG_TO_QOS_CAP: usize = 32768;
pub const IPC_SCH_TO_QOS_CAP: usize = 32768;
pub const IPC_STATUS_CACHE_CAP: usize = 1024 * 1024;

pub const IPC_QOS_TO_SIG_NAME: &str = "qos_to_sig";
pub const IPC_TPU_TO_QOS_NAME: &str = "tpu_to_qos";
pub const IPC_FWD_TO_QOS_NAME: &str = "fwd_to_qos";
pub const IPC_RE1_TO_QOS_NAME: &str = "re1_to_qos";
pub const IPC_RE2_TO_QOS_NAME: &str = "re2_to_qos";
pub const IPC_SIG_TO_QOS_NAME: &str = "sig_to_qos";
pub const IPC_SCH_TO_QOS_NAME: &str = "sch_to_qos";
pub const IPC_STATUS_CACHE_NAME: &str = "tx_status_cache";
