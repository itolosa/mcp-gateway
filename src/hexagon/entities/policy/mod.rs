pub mod allowlist;
pub mod compound;
pub mod denylist;

pub type DefaultFilter =
    compound::CompoundFilter<allowlist::AllowlistFilter, denylist::DenylistFilter>;
