use serde::{Serialize, Serializer};

use crate::dmx::Address;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unknown fixture type \"{0}\"")]
    UnknownProfile(String),
    #[error("\"{profile}\" has no mode called \"{mode}\"")]
    UnknownMode { profile: String, mode: String },
    #[error("fixture {0} does not exist")]
    UnknownFixture(u32),
    #[error("fixture number {0} is already in use")]
    DuplicateFixtureId(u32),
    #[error("fixture numbers must be 1 or more")]
    InvalidFixtureId,
    #[error("address {address} overlaps fixture {other}")]
    AddressOverlap { address: Address, other: u32 },
    #[error("a {footprint} channel fixture does not fit at address {address}")]
    AddressOutOfRange { address: Address, footprint: usize },
    #[error("there is no free block of {0} channels left in the universe")]
    UniverseFull(usize),
    #[error("group {0} does not exist")]
    UnknownGroup(u32),
    #[error("preset {0} does not exist")]
    UnknownPreset(u32),
    #[error("there is nothing in the programmer to record")]
    NothingToRecord,
    #[error("invalid fixture profile: {0}")]
    InvalidProfile(String),
    #[error("not supported yet: {0}")]
    Unsupported(String),
    #[error("invalid show file: {0}")]
    InvalidShow(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

// Errors cross into the UI as their message.
impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
