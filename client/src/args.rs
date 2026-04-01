use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "ssmgr", about = "Sample manager terminal client")]
pub struct Args {
    #[arg(short, long, default_value = "http://localhost:8080")]
    pub server: String,

    #[arg(short, long)]
    pub config: Option<String>,
}
