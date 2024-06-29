use std::env;
use std::io;
use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};

mod filesystem;

/// Repro build system
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Mount {
        #[arg()]
        mountpoint: PathBuf,

        #[arg(short, long)]
        foreground: bool,

        #[arg(short, long)]
        remount: bool,
    },
}

fn main() -> io::Result<()> {
    env_logger::init();
    let cli = Cli::parse();

    match cli.command {
        Command::Mount {
            mountpoint,
            foreground,
            remount,
        } => {
            if let Some(remote) = filesystem::remote(&mountpoint) {
                let pid = remote.pid()?;
                if remount {
                    println!("Unmounting previous filesystem (pid {pid})");
                    remote.unmount()?;
                } else {
                    println!("Already mounted (pid {pid})");
                    return Ok(());
                }
            }

            if foreground {
                filesystem::mount(&mountpoint)?
            } else {
                let pid = process::Command::new(env::args().nth(0).unwrap())
                    .args(env::args().skip(1))
                    .arg("--foreground")
                    .spawn()?
                    .id();

                println!("Mounted at {} (pid {})", mountpoint.display(), pid)
            }
        }
    }

    Ok(())
}
