//! Wire formats carried by standard BLE advertisements and LE CoC SDUs.
//!
//! Session identity is a 128-bit UUID derived from the application service name,
//! so the name itself never travels in the advertising packet. The name remains
//! a local label; only its UUID reaches the air, leaving the payload for
//! application data.

use crate::core::Error;

/// Frame version. Version 1 carried literal service names and is rejected.
pub const VERSION: u8 = 2;
/// Longest accepted local service name. The name is not transmitted, so this is
/// only a sanity bound for the string an application passes to `createSession`.
pub const MAX_SERVICE_ID_BYTES: usize = 64;
/// Application data carried in one legacy advertisement beside the identity.
pub const MAX_ADVERTISING_DATA_BYTES: usize = 8;
pub const SERVICE_UUID_BYTES: usize = 16;
pub const LEGACY_ADVERTISING_BYTES: usize = 31;
/// Flags AD the firmware wrappers do not insert themselves.
const FLAGS_AD: [u8; 3] = [2, 0x01, 0x02];
/// Service Data AD type for a 128-bit UUID.
const SERVICE_DATA_AD_TYPE: u8 = 0x21;
/// Smallest Service Data AD the decoder accepts: type, UUID, version, SPSM.
const SERVICE_DATA_MIN: usize = 1 + SERVICE_UUID_BYTES + 2;

/// FNV-1a over the UTF-8 name yields the advertised identity. Deterministic on
/// both peers, no registry and no wire copy of the name.
pub fn derive_uuid(bytes: &[u8]) -> [u8; SERVICE_UUID_BYTES] {
    const OFFSET: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    const PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;
    let mut hash = OFFSET;
    for byte in bytes {
        hash ^= u128::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash.to_be_bytes()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceId {
    bytes: [u8; MAX_SERVICE_ID_BYTES],
    len: u8,
}

impl ServiceId {
    pub fn new(value: &str) -> Result<Self, Error> {
        if value.is_empty() || value.len() > MAX_SERVICE_ID_BYTES {
            return Err(Error::InvalidArgument);
        }
        let mut bytes = [0; MAX_SERVICE_ID_BYTES];
        bytes[..value.len()].copy_from_slice(value.as_bytes());
        Ok(Self {
            bytes,
            len: value.len() as u8,
        })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }

    pub fn as_str(&self) -> &str {
        // Constructed only from valid UTF-8 and immutable thereafter.
        unsafe { core::str::from_utf8_unchecked(self.as_bytes()) }
    }

    pub fn uuid(self) -> [u8; SERVICE_UUID_BYTES] {
        derive_uuid(self.as_bytes())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdvertisingData {
    bytes: [u8; MAX_ADVERTISING_DATA_BYTES],
    len: u8,
}

impl AdvertisingData {
    pub const EMPTY: Self = Self {
        bytes: [0; MAX_ADVERTISING_DATA_BYTES],
        len: 0,
    };

    pub fn new(data: &[u8]) -> Result<Self, Error> {
        if data.len() > MAX_ADVERTISING_DATA_BYTES {
            return Err(Error::InvalidArgument);
        }
        let mut value = Self::EMPTY;
        value.bytes[..data.len()].copy_from_slice(data);
        value.len = data.len() as u8;
        Ok(value)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Advertisement {
    pub uuid: [u8; SERVICE_UUID_BYTES],
    pub spsm: u8,
    pub data: AdvertisingData,
}

impl Advertisement {
    pub fn encode(&self, output: &mut [u8; LEGACY_ADVERTISING_BYTES]) -> Result<usize, Error> {
        if !(0x80..=0xff).contains(&self.spsm) {
            return Err(Error::InvalidArgument);
        }
        let data = self.data.as_bytes();
        // Content after the AD length byte: type, UUID, version, SPSM, data.
        let ad = 1 + SERVICE_UUID_BYTES + 2 + data.len();
        let end = FLAGS_AD.len() + 1 + ad;
        if end > LEGACY_ADVERTISING_BYTES {
            return Err(Error::InvalidArgument);
        }
        output[..FLAGS_AD.len()].copy_from_slice(&FLAGS_AD);
        output[FLAGS_AD.len()] = ad as u8;
        let mut cursor = FLAGS_AD.len() + 1;
        output[cursor] = SERVICE_DATA_AD_TYPE;
        cursor += 1;
        output[cursor..cursor + SERVICE_UUID_BYTES].copy_from_slice(&self.uuid);
        cursor += SERVICE_UUID_BYTES;
        output[cursor] = VERSION;
        cursor += 1;
        output[cursor] = self.spsm;
        cursor += 1;
        output[cursor..cursor + data.len()].copy_from_slice(data);
        cursor += data.len();
        debug_assert_eq!(cursor, end);
        Ok(cursor)
    }

    /// Parses only the Service Data AD carrying `uuid`; other services are
    /// ignored. A truncated or duplicated matching AD rejects the report.
    pub fn decode(input: &[u8], uuid: [u8; SERVICE_UUID_BYTES]) -> Option<Self> {
        let mut remaining = input;
        let mut found = None;
        while !remaining.is_empty() {
            let len = remaining[0] as usize;
            if len == 0 {
                break;
            }
            if len >= remaining.len() {
                return None;
            }
            let ad = &remaining[1..=len];
            remaining = &remaining[len + 1..];
            if ad[0] != SERVICE_DATA_AD_TYPE
                || ad.len() < SERVICE_DATA_MIN
                || ad[1..1 + SERVICE_UUID_BYTES] != uuid
            {
                continue;
            }
            if found.is_some() || ad[1 + SERVICE_UUID_BYTES] != VERSION {
                return None;
            }
            let spsm = ad[2 + SERVICE_UUID_BYTES];
            if spsm < 0x80 {
                return None;
            }
            let data = AdvertisingData::new(&ad[3 + SERVICE_UUID_BYTES..]).ok()?;
            found = Some(Self { uuid, spsm, data });
        }
        found
    }
}

/// Version plus the identity UUID. The name stays local, so this is fixed size.
pub const MAX_HELLO_BYTES: usize = 1 + SERVICE_UUID_BYTES;

pub fn encode_hello(uuid: [u8; SERVICE_UUID_BYTES], output: &mut [u8; MAX_HELLO_BYTES]) -> usize {
    output[0] = VERSION;
    output[1..].copy_from_slice(&uuid);
    MAX_HELLO_BYTES
}

pub fn validate_hello(uuid: [u8; SERVICE_UUID_BYTES], input: &[u8]) -> Result<(), Error> {
    if input.len() != MAX_HELLO_BYTES || input[0] != VERSION || input[1..] != uuid {
        return Err(Error::InvalidArgument);
    }
    Ok(())
}
