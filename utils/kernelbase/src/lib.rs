#![no_std]

mod def;
mod resolve;
mod utils;

pub use resolve::{
    get,
    initialize,
};
