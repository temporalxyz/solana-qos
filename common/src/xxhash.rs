//! Thin wrapper around xxhasher-rust to facilitate seeding and hashing
use std::{net::IpAddr, ptr::copy_nonoverlapping};

use solana_sdk::packet::{Meta, Packet, PACKET_DATA_SIZE};
use xxhash_rust::{
    const_xxh3::const_custom_default_secret, xxh3::xxh3_64_with_secret,
};

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct xxHasher {
    secret: [u8; 192],
}

#[allow(non_camel_case_types)]
pub type xxHash = u64;

impl xxHasher {
    #[inline(always)]
    pub const fn initialize_with_seed(seed: u64) -> xxHasher {
        xxHasher {
            secret: const_custom_default_secret(seed),
        }
    }

    #[inline(always)]
    pub fn hash(&self, input: &[u8]) -> u64 {
        xxh3_64_with_secret(input, &self.secret)
    }

    /// This function makes assumptions about the packet, i.e. that it
    /// is a transaction packet and that the ip stored in the
    /// metadata is an ipv4 address. It will panic if given a packet
    /// with ipv6
    #[inline(always)]
    pub fn packet_hash(&self, packet: &Packet) -> xxHash {
        // Preimage for packet + meta
        let mut preimage = [0_u8; PACKET_DATA_SIZE + 12];

        // SAFETY: Packet is repr(C) and we are accessing first field
        // with align = 1
        let packet_array_ref: &[u8; PACKET_DATA_SIZE] = unsafe {
            core::mem::transmute::<&Packet, &[u8; PACKET_DATA_SIZE]>(
                &packet,
            )
        };

        // We only want the payload data, not the full buffer
        let packet_data_size = packet.meta().size;
        // SAFETY:
        //
        // The packet data size has already been validated.
        // We avoid several bounds checks in the rest of this scope
        unsafe {
            preimage
                .get_unchecked_mut(..packet_data_size)
                .copy_from_slice(
                    packet_array_ref.get_unchecked(..packet_data_size),
                );
        }

        // Identical packets could come from a different ip/port
        unsafe {
            let meta_ptr = preimage
                .as_mut_ptr()
                .add(packet_data_size);
            write_packet_meta_bytes(meta_ptr, packet.meta());
        };

        self.hash(unsafe {
            preimage.get_unchecked(..packet_data_size + 12)
        })
    }
}

#[inline(always)]
/// Extracts a compact representation of the metadata with no
/// zero-padding
unsafe fn write_packet_meta_bytes(
    meta_ptr: *mut u8,
    meta: &Meta,
) -> *mut u8 {
    // Unpack fields
    let Meta {
        // 8 byte usize
        size,
        // 4 byte ipv4 addr
        addr: IpAddr::V4(ipv4),
        // 2 byte u16 port (we ignore this)
        port: _,
        // 1 byte flag (we ignore this)
        flags: _,
    } = meta
    else {
        unreachable!("ipv4 has been checked already");
    };

    unsafe {
        copy_nonoverlapping(
            &size.to_le_bytes() as *const _ as *const u8,
            meta_ptr,
            8,
        );
        copy_nonoverlapping(ipv4.octets().as_ptr(), meta_ptr.add(8), 4);
    }

    meta_ptr
}

#[cfg(test)]
mod tests {
    use solana_sdk::packet::PacketFlags;

    use super::*;

    use core::array::from_fn;
    use std::net::Ipv4Addr;

    unsafe fn unpack_packet_meta(meta_ptr: *mut u8) -> Meta {
        Meta {
            size: usize::from_le_bytes(from_fn(|i| *meta_ptr.add(i))),
            addr: IpAddr::V4(Ipv4Addr::from([
                *meta_ptr.add(8),
                *meta_ptr.add(9),
                *meta_ptr.add(10),
                *meta_ptr.add(11),
            ])),
            port: u16::from_le_bytes([
                *meta_ptr.add(12),
                *meta_ptr.add(13),
            ]),
            flags: PacketFlags::from_bits(*meta_ptr.add(14)).unwrap(),
        }
    }

    #[test]
    fn test_packet_meta_round_trip() {
        let meta = Meta {
            size: usize::from_le_bytes([1, 2, 3, 4, 5, 6, 7, 8]),
            addr: IpAddr::V4(Ipv4Addr::from([9_u8, 10, 11, 12])),
            port: u16::from_le_bytes([13, 14]),
            flags: PacketFlags::all(),
        };
        let mut meta_bytes = [0; 12];

        let unpacked_meta = unsafe {
            unpack_packet_meta(write_packet_meta_bytes(
                meta_bytes.as_mut_ptr(),
                &meta,
            ))
        };

        // Only these two are written
        assert_eq!(unpacked_meta.addr, meta.addr);
        assert_eq!(unpacked_meta.size, meta.size);

        // These are not written
        assert_eq!(unpacked_meta.flags, PacketFlags::empty());
        assert_eq!(unpacked_meta.port, 0);
    }
}
