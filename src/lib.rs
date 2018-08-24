extern crate libc;

#[cfg(debug_assertions)]
#[cfg_attr(debug_assertions, doc(hidden))]
pub mod id;
#[cfg(not(debug_assertions))]
mod id;
mod invar;
mod platform;
pub mod stable;
mod swap;
#[cfg(debug_assertions)]
#[cfg_attr(debug_assertions, doc(hidden))]
pub mod tests;
mod ucontext;
mod uninit;
mod void;
mod volatile;
mod zero;

#[doc(inline)]
pub use ucontext::*;
