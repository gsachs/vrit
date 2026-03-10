// CLI argument parsing and subcommand dispatch
use clap::{Parser, Subcommand};

use crate::commands;

#[derive(Parser)]
#[command(name = "vrit", about = "A learning-oriented version control system")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a new .vrit repository
    Init,

    /// Compute (and optionally store) a blob hash for a file
    HashObject {
        /// Write the object to the object store
        #[arg(short)]
        w: bool,
        /// File to hash
        file: String,
    },

    /// Display contents, type, or size of a repository object
    CatFile {
        /// Pretty-print the object
        #[arg(short)]
        p: bool,
        /// Show object type
        #[arg(short)]
        t: bool,
        /// Show object size
        #[arg(short)]
        s: bool,
        /// Object SHA
        sha: String,
    },

    /// List tree entries
    LsTree {
        /// Tree object SHA
        sha: String,
    },

    /// Stage files for the next commit
    Add {
        /// Paths to add
        paths: Vec<String>,
    },

    /// Remove a file from the index
    Rm {
        /// Path to remove
        path: String,
    },

    /// Record changes to the repository
    Commit {
        /// Commit message
        #[arg(short)]
        m: String,
    },

    /// Show working tree status
    Status,

    /// Show commit history
    Log,

    /// Show changes between working tree and index, or index and HEAD
    Diff {
        /// Show staged changes (index vs HEAD)
        #[arg(long)]
        staged: bool,
    },

    /// Write the current index as a tree object
    WriteTree,

    /// List or create branches
    Branch {
        /// Branch name to create
        name: Option<String>,
        /// Delete the named branch
        #[arg(short)]
        d: Option<String>,
    },

    /// Switch branches or restore files
    Checkout {
        /// Branch name, commit SHA, or file to restore (after --)
        target: Option<String>,
        /// Restore a file from HEAD (use: checkout -- <file>)
        #[arg(last = true)]
        file: Option<String>,
    },

    /// Merge a branch into the current branch
    Merge {
        /// Branch to merge
        branch: Option<String>,
        /// Abort an in-progress merge
        #[arg(long)]
        abort: bool,
    },

    /// Create, list, or delete tags
    Tag {
        /// Tag name
        name: Option<String>,
        /// Target commit (defaults to HEAD)
        commit: Option<String>,
        /// Create an annotated tag
        #[arg(short)]
        a: bool,
        /// Tag message (for annotated tags)
        #[arg(short)]
        m: Option<String>,
        /// Delete a tag
        #[arg(short)]
        d: Option<String>,
    },

    /// Reset HEAD to a commit and unstage changes
    Reset {
        /// Target commit (defaults to HEAD)
        commit: Option<String>,
    },

    /// Stash or restore uncommitted changes
    Stash {
        #[command(subcommand)]
        action: Option<StashAction>,
    },
}

#[derive(Subcommand)]
enum StashAction {
    /// Apply and remove the top stash entry
    Pop,
    /// List stash entries
    List,
}

pub fn run() -> Result<(), String> {
    let cli = Cli::parse();

    match cli.command {
        Command::Init => commands::init::execute(),
        Command::HashObject { w, file } => commands::hash_object::execute(&file, w),
        Command::CatFile { p, t, s, sha } => commands::cat_file::execute(&sha, p, t, s),
        Command::LsTree { sha } => commands::ls_tree::execute(&sha),
        Command::Add { paths } => commands::add::execute(&paths),
        Command::Rm { path } => commands::rm::execute(&path),
        Command::Commit { m } => commands::commit::execute(&m),
        Command::Status => commands::status::execute(),
        Command::Log => commands::log::execute(),
        Command::Diff { staged } => commands::diff_cmd::execute(staged),
        Command::WriteTree => commands::write_tree::execute(),
        Command::Branch { name, d } => commands::branch::execute(name.as_deref(), d.as_deref()),
        Command::Checkout { target, file } => {
            match (target, file) {
                // checkout -- <file>
                (None, Some(f)) => commands::checkout::execute_restore(&f),
                (Some(t), Some(f)) if t == "--" => commands::checkout::execute_restore(&f),
                // checkout <target>
                (Some(t), None) => commands::checkout::execute(&t, None),
                (Some(t), Some(f)) => commands::checkout::execute(&t, Some(f.as_str())),
                (None, None) => Err("must specify a branch, commit SHA, or -- <file>".into()),
            }
        }
        Command::Merge { branch, abort } => commands::merge::execute(branch.as_deref(), abort),
        Command::Tag { name, commit, a, m, d } => {
            commands::tag::execute(name.as_deref(), commit.as_deref(), a, m.as_deref(), d.as_deref())
        }
        Command::Reset { commit } => commands::reset::execute(commit.as_deref()),
        Command::Stash { action } => {
            match action {
                None => commands::stash::execute_stash(),
                Some(StashAction::Pop) => commands::stash::execute_pop(),
                Some(StashAction::List) => commands::stash::execute_list(),
            }
        }
    }
}
