#[cfg(feature = "collections_adapter")]
pub mod adapter;
#[cfg(feature = "collections_adapter")]
pub use adapter::{ContainerCommon, Queue, QueueLike, Stack, StackLike};

pub mod inplace;

pub use inplace::InplaceVec;
