use anyhow::Result;
use clap::{Arg, Command};

mod app;
mod collectors;
mod config;
mod data;
mod ui;

use app::App;

#[tokio::main]
async fn main() -> Result<()> {
    let matches = Command::new("lazynet")
        .version("0.1.0")
        .about("Terminal User Interface (TUI) system tool for network device inspection")
        .arg(
            Arg::new("export")
                .long("export")
                .help("Export inventory to JSON and exit")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file path")
                .default_value("~/.lazynet/config.toml"),
        )
        .get_matches();

    let config_path = matches.get_one::<String>("config").unwrap();
    let export_mode = matches.get_flag("export");

    let mut app = App::new(config_path).await?;

    if export_mode {
        app.export_json().await?;
    } else {
        app.run().await?;
    }

    Ok(())
}
