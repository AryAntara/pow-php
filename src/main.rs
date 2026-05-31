//! POW — PHP One-click Workspace.
//! Entry point and CLI command routing.

mod commands;
mod config;
mod db;
mod php;
mod utils;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "pow",
    version,
    about = "POW — PHP One-click Workspace. XAMPP, but ultra-minimal.",
    propagate_version = true
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the PHP built-in server
    Start,
    /// Stop the PHP server
    Stop,
    /// Restart the PHP server
    Restart,
    /// Show the status of all services
    Status,
    /// Download + install a PHP version, e.g. `pow install php@8.2`
    Install {
        /// Version target, e.g. `php@8.2`
        target: String,
    },
    /// List installed PHP versions
    #[command(visible_alias = "list")]
    Ls,
    /// Switch the active PHP version, e.g. `pow use php@8.3`
    Use {
        /// Version target, e.g. `php@8.3`
        target: String,
    },
    /// Enter a subshell with a PHP version on PATH; `exit` to leave
    Shell {
        /// Optional version, e.g. `php@8.3`. Defaults to the active version.
        target: Option<String>,
    },
    /// Database control
    Db {
        #[command(subcommand)]
        action: DbAction,
    },
}

#[derive(Subcommand)]
enum DbAction {
    /// Start the database service
    Start,
    /// Stop the database service
    Stop,
    /// Switch to SQLite
    Sqlite,
    /// Switch to MariaDB
    Mysql,
    /// Back up the active database
    Backup {
        /// Output file (defaults to ~/.pow/backups/<driver>-<timestamp>)
        #[arg(long)]
        out: Option<String>,
    },
    /// Restore the active database from a backup file
    Restore {
        /// Backup file to restore from
        file: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Start => commands::server::start(),
        Commands::Stop => commands::server::stop(),
        Commands::Restart => commands::server::restart(),
        Commands::Status => commands::status::run(),
        Commands::Install { target } => commands::install::run(&target).await,
        Commands::Ls => commands::list::run(),
        Commands::Use { target } => commands::switch::run(&target),
        Commands::Shell { target } => commands::shell::run(target.as_deref()),
        Commands::Db { action } => match action {
            DbAction::Start => commands::db::start().await,
            DbAction::Stop => commands::db::stop(),
            DbAction::Sqlite => commands::db::use_sqlite(),
            DbAction::Mysql => commands::db::use_mysql(),
            DbAction::Backup { out } => commands::db::backup(out.as_deref()),
            DbAction::Restore { file } => commands::db::restore(&file),
        },
    };

    if let Err(e) = result {
        utils::error(&format!("{e:#}"));
        std::process::exit(1);
    }
}
