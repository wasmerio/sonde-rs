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
