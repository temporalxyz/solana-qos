use que::headless_spmc::producer::Producer;
use solana_qos_common::{
    ipc_parameters::{
        IPC_FWD_TO_QOS_CAP, IPC_FWD_TO_QOS_NAME, IPC_QOS_TO_SIG_CAP,
        IPC_QOS_TO_SIG_NAME, IPC_RE1_TO_QOS_CAP, IPC_RE1_TO_QOS_NAME,
        IPC_RE2_TO_QOS_CAP, IPC_RE2_TO_QOS_NAME, IPC_SCH_TO_QOS_CAP,
        IPC_SCH_TO_QOS_NAME, IPC_SIG_TO_QOS_CAP, IPC_SIG_TO_QOS_NAME,
        IPC_STATUS_CACHE_CAP, IPC_STATUS_CACHE_NAME,
        IPC_TPU_TO_QOS_CAP, IPC_TPU_TO_QOS_NAME,
    },
    packet_bytes::PacketBytes,
    remaining_meta::QoSRemainingMeta,
};
use solana_qos_core::get_page_size;

macro_rules! initialize_huge {
    ($([$msg:ty, $name:ident, $size:ident]),+) => {
        $(
            // TODO: fix for huge pages
            // if let Err(e) = cleanup_shmem($name, $size as i64, #[cfg(target_os = "linux")] true) {
            //     println!("failed to cleanup shmem {}: {e:?}", $name);
            // }

            if let Err(e) = unsafe { Producer::<$msg, $size>::join_or_create_shmem($name,  get_page_size(#[cfg(target_os = "linux")]true)) } {
                println!("failed to cleanup shmem {}: {e:?}", $name);
            }
        )+

    };
}

fn main() {
    initialize_huge!(
        [PacketBytes, IPC_FWD_TO_QOS_NAME, IPC_FWD_TO_QOS_CAP],
        [PacketBytes, IPC_TPU_TO_QOS_NAME, IPC_TPU_TO_QOS_CAP],
        [PacketBytes, IPC_RE1_TO_QOS_NAME, IPC_RE1_TO_QOS_CAP],
        [PacketBytes, IPC_RE2_TO_QOS_NAME, IPC_RE2_TO_QOS_CAP],
        [PacketBytes, IPC_QOS_TO_SIG_NAME, IPC_QOS_TO_SIG_CAP],
        [PacketBytes, IPC_SIG_TO_QOS_NAME, IPC_SIG_TO_QOS_CAP],
        [QoSRemainingMeta<()>, IPC_SCH_TO_QOS_NAME, IPC_SCH_TO_QOS_CAP],
        [[u8; 64], IPC_STATUS_CACHE_NAME, IPC_STATUS_CACHE_CAP]
    );

    println!("IPC buffers initialized");
}
