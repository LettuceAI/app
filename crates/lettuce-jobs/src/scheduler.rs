//! Admission primitives shared by a future scheduler and the fake store.

pub use crate::{ResourceAvailability, ResourceClass};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceClaim {
    pub class: ResourceClass,
}
