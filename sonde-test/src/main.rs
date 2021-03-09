mod tracing;

fn main() {
    tracing::hello::world(42, ::std::ptr::null_mut());

    println!("Hello, World!");
}
