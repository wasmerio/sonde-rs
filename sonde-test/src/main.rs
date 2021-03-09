mod tracing;
use std::ffi::CString;

fn main() {
    let who = CString::new("Gordon").unwrap();

    tracing::hello::you(who.as_ptr() as *mut _, who.as_bytes().len() as _);

    println!("Hello, World!");
}
