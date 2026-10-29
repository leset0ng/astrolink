//! Wire formats carried by standard BLE advertisements and LE CoC SDUs.
//!
//! The service UUID identifies Astrolink's format; service names remain literal
//! UTF-8 strings, and are never interpreted as Bluetooth UUIDs.

use crate::core::Error;

pub const VERSION: u8 = 1;
pub const MAX_SERVICE_ID_BYTES: usize = 6;
pub const MAX_ADVERTISING_DATA_BYTES: usize = 1;
pub const LEGACY_ADVERTISING_BYTES: usize = 31;
/// Private 128-bit service UUID 9c43e0a1-f2d8-4b5c-8e67-1a623fb094d5,
/// encoded least-significant octet first as required by the GAP data format.
pub const SERVICE_UUID: [u8; 16] = [
    0xd5, 0x94, 0xb0, 0x3f, 0x62, 0x1a, 0x67, 0x8e, 0x5c, 0x4b, 0xd8, 0xf2, 0xa1, 0xe0, 0x43, 0x9c,
];

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
    pub service: ServiceId,
    pub spsm: u8,
    pub data: AdvertisingData,
}

impl Advertisement {
    pub fn encode(&self, output: &mut [u8; LEGACY_ADVERTISING_BYTES]) -> Result<usize, Error> {
        if !(0x80..=0xff).contains(&self.spsm) {
            return Err(Error::InvalidArgument);
        }
        let service = self.service.as_bytes();
        let data = self.data.as_bytes();
        // LE General Discoverable. The host also supports BR/EDR.
        output[..3].copy_from_slice(&[2, 0x01, 0x02]);
        let end = 24 + service.len() + data.len();
        output[3] = (end - 4) as u8;
        output[4] = 0x21; // Service Data - 128-bit UUID.
        output[5..21].copy_from_slice(&SERVICE_UUID);
        output[21] = VERSION;
        output[22] = self.spsm;
        output[23] = service.len() as u8;
        output[24..24 + service.len()].copy_from_slice(service);
        output[24 + service.len()..end].copy_from_slice(data);
        Ok(end)
    }

    /// Parse complete AD structures. A truncated report is never delivered.
    pub fn decode(mut input: &[u8]) -> Option<Self> {
        let mut found = None;
        while !input.is_empty() {
            let len = input[0] as usize;
            if len == 0 {
                break;
            }
            if len >= input.len() {
                return None;
            }
            let ad = &input[1..=len];
            input = &input[len + 1..];
            if ad[0] != 0x21 || ad.len() < 20 || ad[1..17] != SERVICE_UUID {
                continue;
            }
            if found.is_some() || ad[17] != VERSION || ad[18] < 0x80 {
                return None;
            }
            let service_len = ad[19] as usize;
            if service_len == 0 || service_len > MAX_SERVICE_ID_BYTES || 20 + service_len > ad.len()
            {
                return None;
            }
            let service =
                ServiceId::new(core::str::from_utf8(&ad[20..20 + service_len]).ok()?).ok()?;
            let data = AdvertisingData::new(&ad[20 + service_len..]).ok()?;
            found = Some(Self {
                service,
                spsm: ad[18],
                data,
            });
        }
        found
    }
}

pub const MAX_HELLO_BYTES: usize = 2 + MAX_SERVICE_ID_BYTES;

pub fn encode_hello(service: ServiceId, output: &mut [u8; MAX_HELLO_BYTES]) -> usize {
    let bytes = service.as_bytes();
    output[0] = VERSION;
    output[1] = bytes.len() as u8;
    output[2..2 + bytes.len()].copy_from_slice(bytes);
    2 + bytes.len()
}

pub fn validate_hello(service: ServiceId, input: &[u8]) -> Result<(), Error> {
    if input.len() != service.as_bytes().len() + 2
        || input[0] != VERSION
        || input[1] as usize != service.as_bytes().len()
        || &input[2..] != service.as_bytes()
    {
        return Err(Error::InvalidArgument);
    }
    Ok(())
}
