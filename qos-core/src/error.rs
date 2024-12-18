pub type PacketProcessorResult<T = (), E = PacketProcessorError> =
    Result<T, E>;

pub enum PacketProcessorError {
    AddrNotIpv4,
    NonTransactionPacket,
    FailedTransactionView,
    FailedSanitize,
    InvalidMetadata,
    DuplicatePacket,
    RecentlyProcessed,
}
