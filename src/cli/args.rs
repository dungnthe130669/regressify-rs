use std::path::PathBuf;

use reqwest::Method;

#[derive(clap::Args)]
pub struct PublishArgs {
    /// Cleanup the project before building
    #[clap(short = 'c', long, default_value = "false")]
    pub clean: bool,

    /// Output directory
    #[clap(short = 'o', long, default_value = "publish")]
    pub out_dir: PathBuf,
}

#[derive(Clone, clap:: Args)]
pub struct CloneArgs {
    /// Repository
    #[clap(short = 'r', long, visible_alias = "repo", visible_alias = "url")]
    pub repository: String,

    /// Branch
    #[clap(short = 'b', long)]
    pub branch: String,

    /// Username
    #[clap(short = 'u', long)]
    pub username: Option<String>,

    /// Email
    #[clap(short = 'e', long)]
    pub email: Option<String>,

    /// Password
    #[clap(short = 'p', long)]
    pub password: Option<String>,

    /// Directory
    #[clap(short = 'd', long)]
    pub directory: Option<String>,
}

#[derive(clap::Args)]
pub struct FormatArgs {
    /// Input path
    #[clap(short = 'i', long, visible_alias = "src")]
    pub input: PathBuf,

    /// Output path (by default overwrite the input file)
    #[clap(short = 'o', long, visible_alias = "dest")]
    pub output: Option<PathBuf>,

    /// Format json file
    #[clap(action, long)]
    pub json: bool,

    /// Format xml file
    #[clap(action, long)]
    pub xml: bool,

    /// XML version
    #[clap(long)]
    pub xml_variant: Option<u32>,
}

#[derive(clap::Args)]
pub struct PingArgs {
    /// The url to request
    #[clap(short = 'u', long)]
    pub url: String,

    /// Delay between requests, in seconds
    #[clap(short = 's', long, default_value = "10")]
    pub sleep: u64,

    /// Request timeout, in seconds
    #[clap(long, default_value = "600")]
    pub timeout: u64,
}

#[derive(clap::Args)]
pub struct PruneArgs {
    /// Also remove additional folders, i.e. modules
    #[clap(short = 'x', long, action, visible_alias = "ex")]
    pub extreme: bool,

    /// Additional extensions to remove
    /// i.e. --ext log --ext bak
    /// i.e. -e log -e bak
    /// i.e. -e log,bak
    #[clap(
        short = 'e',
        long,
        visible_alias = "ext",
        visible_alias = "extension",
        value_delimiter = ',',
        verbatim_doc_comment
    )]
    pub extra_extension: Vec<String>,
}

#[derive(clap::Args)]
pub struct SendArgs {
    /// Request method
    #[clap(short = 'm', long, default_value = "GET")]
    pub method: Method,

    /// Custom headers to include in the request
    #[clap(short = 'H', long)]
    pub headers: Vec<String>,

    /// Request timeout in seconds
    #[clap(long, default_value = "600")]
    pub timeout: u64,

    /// Whether to log all headers
    #[clap(short, long)]
    pub all: Option<bool>,

    /// Whether to log the response body
    #[clap(short, long, default_value = "true")]
    pub body: Option<bool>,

    /// Output file path to save the response body
    #[clap(short = 'o', long)]
    pub output: Option<PathBuf>,

    /// Whether to send Accept-Encoding: "gzip, deflate, br, zstd" header
    #[clap(short, long)]
    pub zip: Option<bool>,

    /// URL to send the request to
    pub url: String,
}

#[derive(clap::Args)]
pub struct PasswordArgs {
    /// Password length, if not provided, a random length between 18 and 22
    #[clap(short = 'l', long)]
    pub length: Option<u32>,

    /// Number of password instances to generate
    #[clap(short = 'c', long, default_value = "1")]
    pub count: usize,
}

#[derive(clap::Args)]
pub struct LipsumArgs {
    /// Number of characters to generate
    #[clap(short = 'c', long)]
    pub char_count: Option<u32>,

    /// Number of words to generate
    #[clap(short = 'w', long)]
    pub word_count: Option<u32>,
}

#[derive(clap::Args)]
pub struct DiffArgs {
    /// Left file path
    #[clap(short = 'l', long)]
    pub left: PathBuf,

    /// Middle file path
    #[clap(short = 'm', long)]
    pub middle: Option<PathBuf>,
    /// Right file path
    #[clap(short = 'r', long)]
    pub right: PathBuf,

    /// Title for the report
    #[clap(short, long, default_value = "Diff Report")]
    pub title: String,

    /// Left title
    #[clap(
        long,
        visible_alias = "lt",
        visible_alias = "ltitle",
        default_value = "Old Version"
    )]
    pub left_title: String,

    /// Right title
    #[clap(
        long,
        visible_alias = "rt",
        visible_alias = "rtitle",
        default_value = "New Version"
    )]
    pub right_title: String,

    /// Output file path
    #[clap(short = 'o', long)]
    pub output: PathBuf,
}

#[derive(clap::Args)]
pub struct TagArgs {
    /// Remote name
    #[clap(short, long, default_value = "origin")]
    pub remote: String,

    /// The tag name
    pub tag_name: String,
}

#[derive(clap::Args)]
pub struct ClippyArgs {
    /// Format the code before running clippy
    #[clap(long, default_value = "true")]
    pub format: Option<bool>,

    /// Automatically fix clippy warnings
    #[clap(long, default_value = "true")]
    pub fix: Option<bool>,

    /// Run tests after clippy
    #[clap(long, default_value = "true")]
    pub test: Option<bool>,
}
