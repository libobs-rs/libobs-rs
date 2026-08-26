#[cfg(feature = "cli")]
mod args;

#[cfg(feature = "cli")]
use colored::Colorize;
#[cfg(feature = "cli")]
use std::env::{self, args};

#[cfg(all(target_os = "linux", feature = "cli"))]
mod ubuntu_install;

#[cfg(feature = "cli")]
fn setup_logger() -> Result<(), fern::InitError> {
    let level = env::var("RUST_LOG")
        .ok()
        .and_then(|val| val.parse().ok()) // Try parsing e.g. "debug", "warn", etc.
        .unwrap_or(log::LevelFilter::Info); // Default if not set

    fern::Dispatch::new()
        .format(|out, message, record| {
            let level_color = match record.level() {
                log::Level::Error => "red",
                log::Level::Warn => "yellow",
                log::Level::Info => "green",
                log::Level::Debug => "blue",
                log::Level::Trace => "bright_black",
            };
            out.finish(format_args!(
                "[{}] {}",
                record.level().to_string().color(level_color),
                message
            ))
        })
        .level(level)
        .chain(std::io::stdout())
        .apply()?;
    Ok(())
}

#[cfg(feature = "cli")]
fn main() -> anyhow::Result<()> {
    use clap::Parser;

    setup_logger()?;

    let mut args: Vec<_> = args().collect();
    if args.get(1).is_some_and(|e| e == "obs-build") {
        args.remove(1);
    }

    let args = args::RunArgs::parse_from(args);
    match args.command {
        #[cfg(target_os = "linux")]
        args::Commands::Install(args) => {
            ubuntu_install::linux_obs_system_install(args)?;
        }
        args::Commands::Build(args) => {
            use cargo_obs_build::{build_obs_binaries, ObsBuildConfig};
            use std::path::PathBuf;

            let config = ObsBuildConfig {
                cache_dir: args.cache_dir,
                tag: args.tag,
                out_dir: PathBuf::from(args.out_dir),
                repo_id: Some(args.repo_id),
                override_zip: args.override_zip,
                rebuild: args.rebuild,
                browser: args.browser,
                skip_compatibility_check: args.skip_compatibility_check,
                remove_pdbs: args.remove_pdbs,
            };

            build_obs_binaries(config)?;
        }
    };
    Ok(())
}

#[cfg(not(feature = "cli"))]
fn main() {
    eprintln!("This binary requires the 'cli' feature to be enabled.");
    std::process::exit(1);
}
