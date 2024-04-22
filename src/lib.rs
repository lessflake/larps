#![feature(allocator_api)]
#![feature(vec_into_raw_parts)]
#![feature(iter_collect_into)]
#![feature(associated_type_defaults)]
#![feature(never_type)]
#![feature(let_chains)]

pub mod capture;
pub mod definitions;
pub mod meter;
pub mod oodle;
pub mod parser;
pub mod socket;
pub mod ui;
pub mod util;

mod generated {
    pub mod opcode;
    pub mod packet;
}
pub use generated::packet;
