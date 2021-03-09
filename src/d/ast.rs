pub trait Names {
    fn name(&self) -> &str;

    fn safe_name(&self) -> String {
        self.name().replace("__", "_")
    }

    fn name_for_c_macro(&self) -> String {
        self.safe_name().to_uppercase()
    }

    fn name_for_c(&self) -> String {
        self.safe_name().to_lowercase()
    }

    fn name_for_rust(&self) -> String {
        self.safe_name().to_lowercase()
    }
}

/// Contains `provider` blocks from a `.d` file.
#[derive(Debug, PartialEq)]
pub struct Script {
    pub providers: Vec<Provider>,
}

/// Describes a `provider` block.
#[derive(Debug, PartialEq)]
pub struct Provider {
    /// The provider's name.
    pub name: String,

    /// The probes defined inside the the block.
    pub probes: Vec<Probe>,
}

impl Names for Provider {
    fn name(&self) -> &str {
        &self.name
    }
}

/// Describes a `probe`.
#[derive(Debug, PartialEq)]
pub struct Probe {
    /// THe probe's name.
    pub name: String,

    /// The probe's arguments.
    pub arguments: Vec<String>,
}

impl Names for Probe {
    fn name(&self) -> &str {
        &self.name
    }
}

impl Probe {
    pub fn arguments_for_c(&self) -> String {
        self.arguments
            .iter()
            .enumerate()
            .map(|(nth, argument)| format!("{ty} arg{nth}", ty = argument, nth = nth,))
            .collect::<Vec<String>>()
            .join(", ")
    }

    pub fn arguments_for_c_from_rust(&self) -> String {
        self.arguments
            .iter()
            .enumerate()
            .map(|(nth, argument_ty)| {
                let number_of_pointers = argument_ty.chars().filter(|c| *c == '*').count();
                let ty = match argument_ty.trim_end_matches(|c| c == ' ' || c == '*') {
                    "char" => "c_char",
                    "short" => "c_short",
                    "int" => "c_int",
                    "long" => "c_long",
                    "long long" => "c_longlong",
                    "int8_t" => "i8",
                    "int16_t" => "i16",
                    "int32_t" => "i32",
                    "int64_t" => "i64",
                    "intptr_t" => "isize",
                    "uint8_t" => "u8",
                    "uint16_t" => "u16",
                    "uint32_t" => "u32",
                    "uint64_t" => "u64",
                    "uintptr_t" => "usize",
                    "float" => "c_float",
                    "double" => "c_double",
                    t => panic!("D type `{}` isn't supported yet", t),
                };

                format!(
                    "arg{nth}: {ptr}{ty}",
                    ty = ty,
                    ptr = "*mut ".repeat(number_of_pointers),
                    nth = nth
                )
            })
            .collect::<Vec<String>>()
            .join(", ")
    }
}
