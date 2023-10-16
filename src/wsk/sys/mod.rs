#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unused)]

use winapi::{
    km::wdm::{
        IRP as _IRP,
        IRP,
    },
    shared::ntdef::UNICODE_STRING,
};
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
