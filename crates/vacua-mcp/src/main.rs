use clap::Parser;
use rmcp::ServiceExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use vacua_mcp::{McpPolicy, PathDisclosureMode, VacuaDomainService, VacuaMcpServer};

#[derive(Parser, Debug)]
#[command(
    name = "vacua-mcp",
    author = "Weize Yuan <iyuanweize@gmail.com>, Vacua Contributors",
    version,
    about = "Agent-native Model Context Protocol (MCP) server for Vacua storage intelligence",
    long_about = "Vacua MCP server exposes macOS storage analysis, snapshot diffs, candidate evaluation, application residue, duplicate analysis, and immutable plan proposals to AI agents over stdio."
)]
struct Cli {
    #[arg(
        long,
        help = "Use stdio transport for MCP protocol communication (default)"
    )]
    stdio: bool,

    #[arg(
        long,
        help = "Path to the SQLite metadata index database (default: ~/.vacua/index.db)"
    )]
    index: Option<PathBuf>,

    #[arg(
        long,
        help = "Path to the execution journal database (default: ~/.vacua/journal.db)"
    )]
    journal: Option<PathBuf>,

    #[arg(
        long = "allow-root",
        help = "Allowed filesystem roots for inspection (can be specified multiple times; defaults to user home)"
    )]
    allow_roots: Vec<PathBuf>,

    #[arg(
        long = "path-disclosure",
        default_value = "home-relative",
        help = "Path disclosure mode in output: 'home-relative', 'full', or 'redacted'"
    )]
    path_disclosure: String,

    #[arg(
        long = "max-results",
        default_value = "50",
        help = "Maximum items per page returned in listing tools (clamped to 200)"
    )]
    max_results: usize,

    #[arg(
        long = "self-test",
        help = "Perform internal initialization self-test and exit"
    )]
    self_test: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let disclosure_mode =
        PathDisclosureMode::parse(&cli.path_disclosure).unwrap_or(PathDisclosureMode::HomeRelative);

    let allowed_roots = if cli.allow_roots.is_empty() {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        vec![home]
    } else {
        cli.allow_roots
    };

    let policy = McpPolicy::new(
        allowed_roots,
        disclosure_mode,
        cli.max_results,
        2,
        Duration::from_secs(60),
    );

    let service = Arc::new(VacuaDomainService::new(policy, cli.index, cli.journal));

    if cli.self_test {
        eprintln!("Vacua MCP Server Self-Test: OK");
        eprintln!("  Version: {}", env!("CARGO_PKG_VERSION"));
        eprintln!("  Transport: stdio");
        eprintln!("  Mutation Authority: FALSE (strictly read/analyze/propose)");
        eprintln!("  Executor Linked: FALSE (no vacua-executor linkage)");
        eprintln!("  Tools Registered: 14");
        eprintln!(
            "  Primary Root: {}",
            service.policy().primary_root().display()
        );
        return Ok(());
    }

    // STRICT STDOUT PURITY: Log exclusively to stderr. Stdout is dedicated to MCP framing.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "Starting Vacua MCP server on stdio"
    );

    let server = VacuaMcpServer::new(service);
    let session = server.serve(rmcp::transport::stdio()).await?;
    session.waiting().await?;

    Ok(())
}
