fn main() {
    sonde::Builder::new()
        .file("./providerA.d")
        .file("./providerB.d")
        .compile();
}
