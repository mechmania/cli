mod config;
mod request;
mod login;
mod submit;
mod version;
mod update;

use std::{env, path::{Path, PathBuf}, process::Stdio};
use colored::Colorize;

use anyhow::{bail, Context};
use mm_engine::args;
use clap::{
    Parser, 
    Subcommand
};

use crate::config::Lang;

pub const CONFIG_NAME: &str = "mm-config.toml";
pub const JWT_NAME: &str = ".mm-token.txt";


#[derive(Parser, Clone)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// do not check for updates
    #[arg(long = "ignore-updates")]
    no_updates: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Clone)]
pub enum Commands {
    /// log into your team mechmania account
    Login,
    /// run your bot against itself
    Run(Run),
    /// direct passthrough to the mm-engine (for more output control)
    Engine(args::ArgConfig),
    /// submit bot for tournaments
    Submit,
    /// switch which bot version you would like to compete
    Version(Version),
    /// update mm-cli and starterpack
    Update,
}

#[derive(Parser, Clone)]
#[command(about = "easy interface to run matches")]
pub struct Run { 
    /// suppress bot output
    #[arg(short = 'q', long = "quiet")]
    quiet: bool,
}

#[derive(Parser, Clone)]
#[command(about = "")]
pub struct Version {
    #[command(subcommand)]
    command: VersionCommands
}

#[derive(Subcommand, Clone)]
pub enum VersionCommands {
    List,
    Switch(Switch)
}


#[derive(Parser, Clone)]
#[command(about = "")]
pub struct Switch { 
    /// select version
    #[arg(short = 'v', long = "version", value_parser = version::parse_version)]
    version: Option<version::Version>,
}

fn strategy_path(config: &config::Config) -> PathBuf {
    PathBuf::from(match config.language {
        Lang::Rust => "src/strategy",
        Lang::Python => "strategy",
        Lang::Java => "src/com/bot/strategy",
    })
}

fn abs_strategy_path(root: &Path, config: &config::Config) -> PathBuf {
    let strategy_path = match config.language {
        Lang::Rust => "src/strategy",
        Lang::Python => "strategy",
        Lang::Java => "src/com/bot/strategy",
    };
    root.join(strategy_path)
}

async fn run() -> anyhow::Result<()> {

    // let root = find_project_root()?;
    let root = find_project_root();

    let cli = Cli::parse();
    // let conf = config::read(&root)?;
    let conf = root
        .as_ref()
        .or_else(|_| Err(anyhow::anyhow!("could not read root")))
        .and_then(|r| config::read(r));


    match cli.command {
        Commands::Login => login::login(&conf?).await?,
        Commands::Submit => submit::submit(&root?, &conf?).await?,
        Commands::Version(version) => match version.command {
            VersionCommands::List => version::list(&root?, &conf?).await?,
            VersionCommands::Switch(v) => version::switch(v, &root?, &conf?).await?,
        },
        Commands::Run(run) => {

            let root = root?;

            if !cli.no_updates {
                println!("checking for updates...");
                let needs_update = update::check_all_updates(&root, &conf?)
                    .await
                    .context("update check failed")?;
                if needs_update {
                    println!("updates needed!\nplease run mm-cli update\nif you really wish to run the match, use --ignore-updates");
                    return Ok(());
                }
            }

            let scripts_path = root.join("scripts");
            if !scripts_path.exists() {
                bail!("unable to find build scripts");
            }

            let extension = if cfg!(windows) {
                ".bat"
            } else {
                ""
            };

            let build_path = scripts_path.join(format!("build{}", extension));
            let run_path = scripts_path.join(format!("run{}", extension));

            if !build_path.exists() {
                bail!("unable to find build file");
            }

            if !run_path.exists() {
                bail!("unable to find run file");
            }

            println!("building bot...");

            tokio::process::Command::new(build_path)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .await
                .with_context(|| "failed to build bot")?;


            use mm_engine::args::{ OutputSource, OutputMapping };
            use chrono::Utc;

            let engine_args = mm_engine::args::ArgConfig {
                bot_a: run_path.clone(),
                bot_b: run_path,
                print: if run.quiet {
                    None
                } else {
                    Some(vec![
                        OutputSource::BotA,
                        OutputSource::BotB,
                    ])
                },
                output: Some(vec![
                    OutputMapping { 
                        sources: vec![ OutputSource::Gamelog ], 
                        path: root.join("logs").join(format!("log-{}.mmgl", Utc::now().format("%Y%m%d_%H%M%S")))
                    },
                ]),
            };


            mm_engine::engine::run(engine_args)
                .await
                .with_context(|| "fatal engine error")?;
        },
        Commands::Engine(arg_config) => {
            println!("engine ArgConfig: {:#?}", arg_config);
            mm_engine::engine::run(arg_config)
                .await
                .with_context(|| "fatal engine error")?;
        },
        Commands::Update => update::update_all(&root?, &conf?).await?
    }

    Ok(())
}

fn find_project_root() -> anyhow::Result<PathBuf> {
    let current_dir = env::current_dir().with_context(|| "failed to get current directory")?;
    
    for ancestor in current_dir.ancestors() {
        if is_project_root(ancestor) {
            return Ok(ancestor.to_path_buf());
        }
    }
    
    anyhow::bail!("could not find {}. make sure you are in your mechmania repository", CONFIG_NAME)
}

fn is_project_root(dir: &Path) -> bool {
    dir.join(CONFIG_NAME).exists()
}


#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("{}", format!("{:#}", err).red());
        eprintln!("for help, please reach out to us on discord");
        std::process::exit(1);
    }
}
