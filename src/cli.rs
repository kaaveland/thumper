use clap::{Parser, Subcommand};

#[derive(Subcommand)]
pub enum SubCommand {
    /// Sync a local folder to a path within a bunny.net Storage Zone
    Sync(SyncArgs),
    /// Provide shell completions
    Completions {
        #[arg(short, long, default_value = "bash", value_parser=clap::builder::PossibleValuesParser::new(["bash", "zsh", "fish", "pwsh", "powershell"]))]
        shell: String,
    },
    /// Purge a URL from the bunny.net cache
    PurgeUrl(PurgeUrlArgs),
    /// Purge an entire pull zone from bunny.net cache
    PurgeZone(PurgeZoneArgs),
}

#[derive(Parser)]
#[command(name = "thumper")]
#[command(arg_required_else_help = true)]
#[command(about = "Sync your files to bunny cdn storage zone")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(
    long_about = "thumper is a tool for synchronizing files to bunny cdn storage zones

thumper can sync to subtrees of your storage zone, the entire storage zone, or selectively skip
parts of the tree. It can easily deploy a static site with a single command.

thumper refuses to sync if it looks like there's already an active sync job to the storage
zone. It places a lockfile into the storage zone during the sync to have rudimentary concurrency
control.

thumper aims to make the local_path and the path within the storage zone exactly equal. It will sync
HTML at the end, to ensure other assets like CSS are already updated by the time they sync."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: SubCommand,

    /// API key for bunny CDN --  looked up in environment variable THUMPER_API_KEY if not present
    #[arg(short, long)]
    pub api_key: Option<String>,
}

#[derive(Parser)]
pub struct SyncArgs {
    /// Which bunny cdn endpoint to use
    #[arg(short, long, default_value = "storage.bunnycdn.com")]
    pub endpoint: String,
    /// Local directory to put in the storage zone
    #[arg(name = "local_path", required = true, num_args = 1)]
    pub local_path: String,
    /// Which storage zone to sync to
    #[arg(name = "storage_zone", required = true, num_args = 1)]
    pub storage_zone: String,
    /// Path inside the storage zone to sync to, path to a directory
    #[arg(short, long = "path", default_value = "/")]
    pub remote_path: String,
    /// Don't sync, just show what would change
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
    /// Force a sync despite a hanging lock file
    #[arg(short, long, default_value_t = false)]
    pub force: bool,
    /// Filename to use for the lockfile. thumper will not sync if this file exists in the destination.
    #[arg(long, default_value = ".thumper.lock")]
    pub lockfile: String,
    /// Do not delete anything in the storage zone paths that start with this prefix (can pass multiple times)
    #[arg(short, long)]
    pub ignore: Vec<String>,
    #[arg(short, long, default_value_t = false)]
    pub verbose: bool,
    /// Number of threads to use when calling bunny.net API (default to number of cpus)
    #[arg(short, long)]
    pub concurrency: Option<usize>,
}

#[derive(Parser)]
pub struct PurgeUrlArgs {
    /// URL to purge, wildcard * is allowed at the end
    #[arg(name = "url")]
    pub url: String,
}

#[derive(Parser)]
pub struct PurgeZoneArgs {
    /// Numeric ID of pull zone to purge
    #[arg(name = "pullzone")]
    pub pullzone: u64,
    /// Optional Cache Tag to target
    #[arg(short, long)]
    pub cache_tag: Option<String>,
}

#[cfg(test)]
mod tests {
    use std::fs;
    use clap::CommandFactory;
    use crate::cli::Cli;

    #[test]
    fn render_help() {
        let mut cli = Cli::command();
        let help = cli.render_help().to_string();
        fs::write(
            "docs/src/help", help
        ).unwrap();
    }

    #[test]
    fn render_sync_help() {
        let mut cli = Cli::command();
        for subcommand in cli.get_subcommands_mut() {
            if subcommand.get_name() == "sync" {
                let help = subcommand.render_help().to_string()
                    .replacen("sync", "thumper sync", 1);
                fs::write(
                    "docs/src/synchelp", help
                ).unwrap();
            }
        }
    }
}
