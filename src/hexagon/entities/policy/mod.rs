pub mod allowlist;
pub mod compound;
pub mod denylist;

pub type DefaultPolicy =
    compound::CompoundPolicy<allowlist::AllowlistPolicy, denylist::DenylistPolicy>;
