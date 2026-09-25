use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    PlayerAlreadyVerified = 4,
    InvalidTitle = 5,
    InvalidThreshold = 6,
    AttestationAlreadyExists = 7,
    BadgeNotFound = 8,
    AlreadyRevoked = 9,
}
