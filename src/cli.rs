use clap::{Parser, Subcommand};
use clap_complete::Shell;

/// Manage configuration of reverse proxies for local development domain using Nginx.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    /// Specific path of the nginx config file
    #[clap(short, long)]
    pub nginx_path: Option<String>,
    /// command to execute, like list / add / remove
    #[clap(subcommand)]
    pub command: Option<Commands>,
    /// Turn verbose information on
    #[clap(short, long, parse(from_occurrences))]
    pub verbose: usize,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// List all the servers and their proxies
    List {
    },
    /// Find a specific server and its proxies
    Find {
        /// Name of the server to find, will also try with adding a local domain.
        server_name: String,
        /// If we should open it in the browser if found
        #[clap(short, long)]
        open: bool,
    },
    /// Open specific server in the browser, like find with --open
    Open {
        /// Name of the server to find, will also try with adding a .localdev domain.
        server_name: String,
    },
    /// Add a server or proxy
    Add {
        /// Name of the server to configure, if found will update the config else will create a new config. Auto adds a .localdev domain.
        server_name: String,
        /// The default (/) proxy target, eg: http://localhost:3000
        #[clap(default_value="http://localhost:3000")]
        default_target: String,
        /// The websocket proxy, eg: --ws ws:localhost:3000, added by default
        #[clap(short, long, default_value="/ws:localhost:3000")]
        ws: String,
        /// Other proxies, for example for a backend: api=http://localhost:8080 or api:8080
        #[clap(short, long)]
        proxy: Vec<String>,
        /// Force the reconfiguration even if the server is already configured
        #[clap(long)]
        force: bool,
        /// If we should open it in the browser right after adding it
        #[clap(short, long)]
        open: bool,
    },
    /// Remove a server or proxy
    Remove {
        /// Name of the server to remove.
        server_name: String,
    },
    /// Reload nginx config
    Reload {
    },
    /// Generate completion script
    Completion {
        #[clap(long,short,arg_enum)]
        shell: Shell,
    },
}

