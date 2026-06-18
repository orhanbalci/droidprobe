//! Concrete [`Parse`] implementations, one submodule per command family.
//!
//! Each submodule defines a zero-sized parser type (e.g. [`getprop::GetpropParser`])
//! implementing [`crate::Parse`]. Keeping them as types (rather than free
//! functions) lets the command crate name the parser as an associated type.

pub mod battery;
pub mod getprop;
pub mod logcat;
pub mod packages;

#[cfg(test)]
mod tests;
