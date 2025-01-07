use solana_sdk::signature::Signature;

pub type SignatureBytes = [u8; 64];

pub fn sig_bytes(s: &Signature) -> &SignatureBytes {
    // SAFETY: repr transparent 64 byte array
    unsafe { core::mem::transmute(s) }
}

/// compiler should be able to skip bounds checks for this since the input is a len = 64 array
#[inline(always)]
#[rustfmt::skip]
pub fn u64_key(signature: &[u8; 64]) -> u64 {
    u64::from_le_bytes([
        signature[00] ^ signature[01] ^ signature[02] ^ signature[03] ^ signature[04] ^ signature[05] ^ signature[06] ^ signature[07],
        signature[08] ^ signature[09] ^ signature[10] ^ signature[11] ^ signature[12] ^ signature[13] ^ signature[14] ^ signature[15],
        signature[16] ^ signature[17] ^ signature[18] ^ signature[19] ^ signature[20] ^ signature[21] ^ signature[22] ^ signature[23],
        signature[24] ^ signature[25] ^ signature[26] ^ signature[27] ^ signature[28] ^ signature[29] ^ signature[30] ^ signature[31],
        signature[32] ^ signature[33] ^ signature[34] ^ signature[35] ^ signature[36] ^ signature[37] ^ signature[38] ^ signature[39],
        signature[40] ^ signature[41] ^ signature[42] ^ signature[43] ^ signature[44] ^ signature[45] ^ signature[46] ^ signature[47],
        signature[48] ^ signature[49] ^ signature[50] ^ signature[51] ^ signature[52] ^ signature[53] ^ signature[54] ^ signature[55],
        signature[56] ^ signature[57] ^ signature[58] ^ signature[59] ^ signature[60] ^ signature[61] ^ signature[62] ^ signature[63],
    ])
}
