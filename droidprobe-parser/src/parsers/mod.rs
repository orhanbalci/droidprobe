//! Concrete [`Parse`] implementations, one submodule per command family.
//!
//! Each submodule defines a zero-sized parser type (e.g. [`getprop::GetpropParser`])
//! implementing [`crate::Parse`]. Keeping them as types (rather than free
//! functions) lets the command crate name the parser as an associated type.

pub mod battery;
pub mod cpuinfo;
pub mod getprop;
pub mod imei;
pub mod logcat;
pub mod meminfo;
pub mod package_dump;
pub mod packages;
pub mod screen;
pub mod storage;

#[cfg(test)]
mod tests;
