use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "ssmgr-server", about = "Sample management server")]
pub struct Args {
    #[arg(short, long, default_value = "8080")]
    pub port: u16,

    #[arg(long, default_value = "8081")]
    pub strudel_port: u16,

    #[arg(short, long)]
    pub config: Option<String>,
}
