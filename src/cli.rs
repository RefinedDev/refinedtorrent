use anyhow::{Context, Result, anyhow, bail};
use std::path::PathBuf;

pub struct CliArgs {
    pub torrent_file: PathBuf,
    pub port: String,
    pub output_dir: PathBuf,
}

impl Default for CliArgs {
    fn default() -> Self {
        CliArgs {
            torrent_file: PathBuf::new(),
            port: String::from("6881"),
            output_dir: PathBuf::from("."),
        }
    }
}

pub fn parse_args(args: &[String]) -> Result<CliArgs> {
    let mut cli = CliArgs::default();
    let mut port: Option<String> = None;
    let mut output: Option<PathBuf> = None;
    let mut torrent_file: Option<PathBuf> = None;
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }

            "--port" => {
                i += 1;
                let value = args.get(i).context("--port requires a value")?;
                port = Some(value.clone());
            }

            "--output" => {
                i += 1;
                let value = args.get(i).context("--output requires a value")?;
                output = Some(PathBuf::from(value));
            }

            flag if flag.starts_with('-') => bail!("unknown option '{flag}'"),
            path => {
                if torrent_file.is_some() {
                    bail!("unexpected argument '{path}'");
                }
                torrent_file = Some(PathBuf::from(path));
            }
        }

        i += 1;
    }

    if let Some(port) = port {
        port.parse::<u16>()
            .map_err(|_| anyhow!("invalid port '{port}': expected a number between 1 and 65535"))?;

        cli.port = port;
    }

    if let Some(output) = output {
        cli.output_dir = output;
    }

    cli.torrent_file = torrent_file.context("missing required argument '<torrent-file>'")?;

    Ok(cli)
}

pub fn print_usage() {
    let program_path =
        std::fs::canonicalize(std::env::args().next().unwrap_or_default()).unwrap_or_default();
    let program = program_path
        .strip_prefix(std::env::current_dir().unwrap_or_default())
        .unwrap_or(std::path::Path::new(""))
        .to_str()
        .unwrap_or_default();

    print!(
        "\
refinedtorrent - a minimal BitTorrent client

USAGE:
    {program} <torrent-file> [OPTIONS]

ARGS:
    <torrent-file>    Path to the .torrent file

OPTIONS:
    --port <n>        Port advertised to the tracker [default: 6881]
    --output <dir>    Directory to save the download [default: current dir]
    -h, --help        Print this help
"
    );
}
