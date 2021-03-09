mod tracing {
    #![allow(unused)]

    include!(env!("SONDE_RUST_API_FILE"));
}

fn main() {
    {
        let who = std::ffi::CString::new("Gordon").unwrap();
        tracing::hello::you(who.as_ptr() as *mut _, who.as_bytes().len() as _);
    }

    println!("Hello, World!");
}
