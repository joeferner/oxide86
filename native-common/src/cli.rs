use clap::Parser;

#[derive(Parser)]
pub struct CommonCli {
    /// Path to the program binary to load and execute
    #[arg()]
    pub program: Option<String>,

    /// Starting segment address (default: 0x1000)
    #[arg(long, default_value = "0x1000")]
    pub segment: String,

    /// Starting offset address (default: 0x0100, like .COM files)
    #[arg(long, default_value = "0x0100")]
    pub offset: String,

    /// CPU type to emulate (8086, 286, 386, 486)
    #[arg(long = "cpu", default_value = "286")]
    pub cpu_type: String,
}
