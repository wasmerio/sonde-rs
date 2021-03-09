use crate::dparser;
use std::{
    env,
    fs::read_to_string,
    io::prelude::*,
    path::{Path, PathBuf},
    process::Command,
};

const SONDE_RUST_API_FILE_ENV_NAME: &str = "SONDE_RUST_API_FILE";

#[derive(Default)]
pub struct Builder {
    d_files: Vec<PathBuf>,
    keep_h_file: bool,
    keep_c_file: bool,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    pub fn file<P>(&mut self, path: P) -> &mut Self
    where
        P: AsRef<Path>,
    {
        self.d_files.push(path.as_ref().to_path_buf());

        self
    }

    pub fn files<P>(&mut self, paths: P) -> &mut Self
    where
        P: IntoIterator,
        P::Item: AsRef<Path>,
    {
        for path in paths.into_iter() {
            self.file(path);
        }

        self
    }

    pub fn keep_h_file(&mut self, keep: bool) -> &mut Self {
        self.keep_h_file = keep;

        self
    }

    pub fn keep_c_file(&mut self, keep: bool) -> &mut Self {
        self.keep_c_file = keep;

        self
    }

    pub fn compile(&self) {
        let out_dir = env::var("OUT_DIR")
            .map_err(|_| "The Cargo `OUT_DIR` variable is missing")
            .unwrap();
        let mut contents = String::new();
        let mut providers = Vec::with_capacity(self.d_files.len());

        // Collect all contents of the `.d` files, and parse the declared providers.
        {
            for d_file in &self.d_files {
                let content = read_to_string(d_file).unwrap();
                contents.push_str(&content);

                let script = dparser::parse(&content).unwrap();

                for provider in script.providers {
                    providers.push(provider);
                }
            }
        }

        // Let's get a unique `.h` file from the `.d` files.
        let h_file = tempfile::Builder::new()
            .prefix("sonde-")
            .suffix(".h")
            .tempfile_in(&out_dir)
            .unwrap();

        let h_file_name = h_file.path();

        {
            let mut d_file = tempfile::Builder::new()
                .prefix("sonde-")
                .suffix(".d")
                .tempfile_in(&out_dir)
                .unwrap();
            d_file.write_all(contents.as_bytes()).unwrap();

            Command::new("dtrace")
                .arg("-o")
                .arg(h_file_name.as_os_str())
                .arg("-h")
                .arg("-s")
                .arg(&d_file.path().as_os_str())
                .status()
                .unwrap();
        }

        // Generate the FFI `.c` file.
        let mut ffi_file = tempfile::Builder::new()
            .prefix("sonde-ffi-")
            .suffix(".c")
            .tempfile_in(&out_dir)
            .unwrap();

        {
            let ffi = format!(
                "#include {header_file:?}\n\n\
                {wrappers}",
                header_file = h_file_name,
                wrappers = providers
                    .iter()
                    .map(|provider| {
                        let provider_name = &provider.name;

                        provider
                            .probes
                            .iter()
                            .map(|probe| {
                                let probe_name = probe.name.replace("__", "_");

                                format!(
                                    "void {prefix}_probe_{suffix}() {{ \
                                        {macro_prefix}_{macro_suffix}(); \
                                    }}",
                                    prefix = provider_name.to_lowercase(),
                                    suffix = probe_name.to_lowercase(),
                                    macro_prefix = provider_name.to_uppercase(),
                                    macro_suffix = probe_name.to_uppercase(),
                                )
                            })
                            .collect::<Vec<String>>()
                            .join("\n\n")
                    })
                    .collect::<Vec<String>>()
                    .join("\n\n")
            );

            ffi_file.write_all(ffi.as_bytes()).unwrap();
        }

        // Let's compile the FFI `.c` file to a `.a` file.
        {
            cc::Build::new().file(&ffi_file).compile(
                ffi_file
                    .path()
                    .with_extension("")
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap(),
            );
        }

        // Finally, let's generate the nice API for Rust.
        let mut rs_file = tempfile::Builder::new()
            .prefix("sonde-")
            .suffix(".rs")
            .tempfile_in(&out_dir)
            .unwrap();

        {
            let rs = format!(
                "extern \"C\" {{ \
                    {externs} \
                }} \
                \n \
                {wrappers}",
                externs = providers
                    .iter()
                    .map(|provider| {
                        let provider_name = &provider.name;

                        provider
                            .probes
                            .iter()
                            .map(|probe| {
                                let probe_name = probe.name.replace("__", "_");

                                format!(
                                    "fn {prefix}_probe_{suffix}();",
                                    prefix = provider_name.to_lowercase(),
                                    suffix = probe_name.to_lowercase(),
                                )
                            })
                            .collect::<Vec<String>>()
                            .join("\n\n")
                    })
                    .collect::<Vec<String>>()
                    .join("\n\n"),
                wrappers = providers
                    .iter()
                    .map(|provider| {
                        let provider_name = &provider.name;

                        provider
                            .probes
                            .iter()
                            .map(|probe| {
                                let probe_name = probe.name.replace("__", "_");

                                format!(
                                    "pub fn {probe_name}() {{ \
                                        unsafe {{ {ffi_prefix}_probe_{ffi_suffix}() }}; \
                                    }}",
                                    probe_name = probe_name,
                                    ffi_prefix = provider_name.to_lowercase(),
                                    ffi_suffix = probe_name.to_lowercase(),
                                )
                            })
                            .collect::<Vec<String>>()
                            .join("\n\n")
                    })
                    .collect::<Vec<String>>()
                    .join("\n\n")
            );

            println!(
                "cargo:rustc-env={name}={value}",
                name = SONDE_RUST_API_FILE_ENV_NAME,
                value = rs_file.path().display(),
            );

            rs_file.write_all(rs.as_bytes()).unwrap();
        }

        if self.keep_h_file {
            h_file.keep().unwrap();
        }

        if self.keep_c_file {
            ffi_file.keep().unwrap();
        }

        rs_file.keep().unwrap();
    }
}
