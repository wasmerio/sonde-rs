use crate::d::{self, ast::Names};
use std::{
    env,
    fs::{read_to_string, File},
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

        // Tell Cargo to rerun the build script if one of the `.d` files has changed.
        {
            for d_file in &self.d_files {
                println!(
                    "cargo:rerun-if-changed={file}",
                    file = d_file.as_path().display()
                );
            }
        }

        // Collect all contents of the `.d` files, and parse the declared providers.
        {
            for d_file in &self.d_files {
                let content = read_to_string(d_file).unwrap();
                contents.push_str(&content);

                let script = d::parser::parse(&content).unwrap();

                for provider in script.providers {
                    providers.push(provider);
                }
            }
        }

        // Let's get a unique `.h` file from the `.d` files.
        let h_file = tempfile::Builder::new()
            .prefix("sonde")
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
                .arg("-arch")
                .arg(match env::var("CARGO_CFG_TARGET_ARCH").unwrap().as_str() {
                    "aarch64" => "arm64",
                    arch => arch,
                })
                .arg("-o")
                .arg(h_file_name.as_os_str())
                .arg("-h")
                .arg("-s")
                .arg(&d_file.path().as_os_str())
                .status()
                .unwrap();
        }

        // Generate the FFI `.c` file. The probes are defined behind C
        // macros; they can't be call from Rust, so we need to wrap
        // them inside C functions.
        let mut ffi_file = tempfile::Builder::new()
            .prefix("sonde-ffi")
            .suffix(".c")
            .tempfile_in(&out_dir)
            .unwrap();

        {
            let ffi = format!(
                r#"#include {header_file:?}

{wrappers}"#,
                header_file = h_file_name,
                wrappers = providers
                    .iter()
                    .map(|provider| {
                        provider
                            .probes
                            .iter()
                            .map(|probe| {
                                format!(
                                    r#"
void {prefix}_probe_{suffix}({arguments}) {{
    {macro_prefix}_{macro_suffix}({argument_names});
}}
"#,
                                    prefix = provider.name_for_c(),
                                    suffix = probe.name_for_c(),
                                    macro_prefix = provider.name_for_c_macro(),
                                    macro_suffix = probe.name_for_c_macro(),
                                    arguments = probe.arguments_for_c(),
                                    argument_names = probe
                                        .arguments
                                        .iter()
                                        .enumerate()
                                        .map(|(nth, _)| { format!("arg{nth}", nth = nth) })
                                        .collect::<Vec<String>>()
                                        .join(", ")
                                )
                            })
                            .collect::<Vec<String>>()
                            .join("")
                    })
                    .collect::<Vec<String>>()
                    .join("\n")
            );

            ffi_file.write_all(ffi.as_bytes()).unwrap();
        }

        // Let's compile the FFI `.c` file to a `.a` file.
        {
            cc::Build::new().file(&ffi_file).compile("sonde-ffi");
        }

        // Finally, let's generate the nice API for Rust.
        let mut rs_path = PathBuf::new();
        rs_path.push(&out_dir);
        rs_path.push("sonde.rs");
        let mut rs_file = File::create(&rs_path).unwrap();

        {
            let rs = format!(
                r#"/// Bindings from Rust to the C FFI small library that calls the
/// probes.

use std::os::raw::*;

extern "C" {{
{externs}
}}

{wrappers}
"#,
                externs = providers
                    .iter()
                    .map(|provider| {
                        provider
                            .probes
                            .iter()
                            .map(|probe| {
                                format!(
                                    r#"    #[doc(hidden)]
    fn {ffi_prefix}_probe_{ffi_suffix}({arguments});"#,
                                    ffi_prefix = provider.name_for_c(),
                                    ffi_suffix = probe.name_for_c(),
                                    arguments = probe.arguments_for_c_from_rust(),
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
                        format!(
                            r#"/// Probes for the `{provider_name}` provider.
pub mod r#{provider_name} {{
    use std::os::raw::*;

{probes}
}}"#,
                            provider_name = provider.name_for_rust(),
                            probes = provider
                                .probes
                                .iter()
                                .map(|probe| {
                                    format!(
                                        r#"    /// Call the `{probe_name}` probe of the `{provider_name}` provider.
    pub fn r#{probe_name}({arguments}) {{
        unsafe {{ super::{ffi_prefix}_probe_{ffi_suffix}({argument_names}) }};
    }}"#,
                                        provider_name = provider.name_for_rust(),
                                        probe_name = probe.name_for_rust(),
                                        ffi_prefix = provider.name_for_c(),
                                        ffi_suffix = probe.name_for_c(),
                                        arguments = probe.arguments_for_c_from_rust(),
                                        argument_names = probe
                                            .arguments
                                            .iter()
                                            .enumerate()
                                            .map(|(nth, _)| { format!("arg{nth}", nth = nth) })
                                            .collect::<Vec<String>>()
                                            .join(", ")
                                    )
                                })
                                .collect::<Vec<String>>()
                                .join("\n\n")
                        )
                    })
                    .collect::<Vec<String>>()
                    .join("\n\n")
            );

            println!(
                "cargo:rustc-env={name}={value}",
                name = SONDE_RUST_API_FILE_ENV_NAME,
                value = rs_path.as_path().display(),
            );

            rs_file.write_all(rs.as_bytes()).unwrap();
        }

        if self.keep_h_file {
            h_file.keep().unwrap();
        }

        if self.keep_c_file {
            ffi_file.keep().unwrap();
        }
    }
}
