mod allowlist;
mod compound;
mod denylist;

pub use allowlist::AllowlistFilter;
pub use compound::CompoundFilter;
pub use denylist::DenylistFilter;

pub type DefaultFilter = CompoundFilter<AllowlistFilter, DenylistFilter>;
