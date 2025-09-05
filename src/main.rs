mod config;
mod request;
mod login;
mod submit;

use std::{env, path::{Path, PathBuf}};
use colored::Colorize;

use anyhow::{bail, Context};
use mm_engine::args;
use clap::{
    Parser, 
    Subcommand
};

pub const CONFIG_NAME: &str = "mm-config.toml";
pub const JWT_NAME: &str = ".mm-token";


#[derive(Parser, Clone)]
#[command(version, about, long_about = None)]
pub struct Cli {
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
}

#[derive(Parser, Clone)]
#[command(about = "easy interface to run matches")]
pub struct Run { 
    /// suppress bot output
    #[arg(short = 'q', long = "quiet")]
    quiet: bool,
}

async fn run() -> anyhow::Result<()> {

    let root = find_project_root()?;

    let cli = Cli::parse();
    let conf = config::read(&root)?;

    match cli.command {
        Commands::Login => login::login(&conf).await?,
        Commands::Submit => submit::submit(&root, &conf).await?,
        Commands::Run(run) => {

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
                bail!("unable to find bot file");
            }

            if !run_path.exists() {
                bail!("unable to find bot file");
            }

            tokio::process::Command::new(build_path)
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
