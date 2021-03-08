fn main() {
    sonde::Builder::new()
        .d_file("./providerA.d")
        .d_file("./providerB.d")
        .compile();
}
