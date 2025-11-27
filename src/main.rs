use crate::cli::{SubCommand, Cli, PurgeUrlArgs, PurgeZoneArgs, SyncArgs};
use crate::sync::SyncJob;
use anyhow::{Context, anyhow};
use clap::{CommandFactory, Parser};
use clap_complete::Shell::{Bash, Elvish, Fish, PowerShell, Zsh};
use clap_complete::generate;
use fxhash::FxHashMap;
use std::{env, io};

mod api;
mod cli;
mod sync;
mod lock;

fn do_sync(api_key: &str, args: SyncArgs) -> anyhow::Result<()> {
    let job = SyncJob::new(
        api_key,
        &args.endpoint,
        &args.storage_zone,
        &args.local_path,
        &args.remote_path,
        &args.lockfile,
        args.force,
        args.dry_run,
        args.verbose,
        args.ignore,
        args.concurrency
    )?;

    job.execute()?;

    Ok(())
}

fn do_purge_url(api_key: &str, args: PurgeUrlArgs) -> anyhow::Result<()> {
    let client = reqwest::blocking::Client::new();
    let encoded = urlencoding::encode(&args.url);
    let response = client
        .post("https://api.bunny.net/purge")
        .query(&[("url", encoded.as_ref())])
        .header("AccessKey", api_key)
        .send()?;
    Ok(response
        .error_for_status()
        .map(|_| println!("Purged {}", args.url))?)
}

fn do_purge_zone(api_key: &str, args: PurgeZoneArgs) -> anyhow::Result<()> {
    let client = reqwest::blocking::Client::new();
    let request = client
        .post(format!(
            "https://api.bunny.net/pullzone/{}/purgeCache", args.pullzone
        ))
        .header("AccessKey", api_key);
    let response = if let Some(tag) = args.cache_tag {
        let mut form = FxHashMap::default();
        form.insert("CacheTag", tag);
        request.form(&form).send()
    } else {
        request.send()
    }?;
    Ok(response
        .error_for_status()
        .map(|_| println!("Purged {}", args.pullzone))?)
}

fn generate_completions(shell: &str) -> anyhow::Result<()> {
    let sh = match shell {
        "bash" => Ok(Bash),
        "zsh" => Ok(Zsh),
        "fish" => Ok(Fish),
        "pwsh" | "powershell" => Ok(PowerShell),
        "elvish" => Ok(Elvish),
        _ => Err(anyhow!("Unsupported shell: {shell}")),
    }?;
    
    let mut com = Cli::command();
    
    generate(sh, &mut com, "thumper", &mut io::stdout());

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    let api_key = args.api_key
        .or_else(|| env::var("THUMPER_API_KEY").ok())
        .context("No API key provided with --api-key or thumper_API_KEY")?;

    match args.command {
        SubCommand::Sync(args ) => do_sync(&api_key, args),
        SubCommand::PurgeUrl(args) => do_purge_url(&api_key, args),
        SubCommand::PurgeZone(args) => do_purge_zone(&api_key, args),
        SubCommand::Completions { shell } => generate_completions(&shell)
    }
}
