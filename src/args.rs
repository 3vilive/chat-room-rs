use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = env!("CARGO_PKG_NAME"), about = "a tcp chat room base on tokio")]
pub struct Args {
    #[structopt(short, long)]
    pub debug: bool,

    #[structopt(short, long, parse(from_occurrences))]
    pub verbose: u8,

    #[structopt(short, long, default_value = "0.0.0.0")]
    pub host: String,

    #[structopt(short, long, default_value = "6000")]
    pub port: u16,
}