#[cfg(feature = "collections_adapter")]
pub mod adapter;
#[cfg(feature = "collections_adapter")]
pub use adapter::{ContainerCommon, Queue, QueueLike, Stack, StackLike};

/// inplace collections.
///
/// algorithms are from std library.
pub mod inplace;

pub use inplace::{InplaceDeque, InplaceHeap, InplaceVec};
