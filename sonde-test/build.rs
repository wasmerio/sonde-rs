use sonde::Builder;

fn main() {
    Builder::new()
        .d_file("./providerA.d")
        .d_file("./providerB.d")
        .compile();
}
