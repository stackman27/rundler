// This file is part of Rundler.
//
// Rundler is free software: you can redistribute it and/or modify it under the
// terms of the GNU Lesser General Public License as published by the Free Software
// Foundation, either version 3 of the License, or (at your option) any later version.
//
// Rundler is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY;
// without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
// See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with Rundler.
// If not, see https://www.gnu.org/licenses/.

use anyhow::Context;
use clap::{builder::PossibleValuesParser, Args, Parser, Subcommand};

mod builder;
mod json;
mod node;
mod pool;
mod prometheus_exporter;
mod rpc;
mod tracing;

use builder::BuilderCliArgs;
use node::NodeCliArgs;
use pool::PoolCliArgs;
use rpc::RpcCliArgs;
use rundler_rpc::EthApiSettings;
use rundler_sim::{
    EstimationSettings, PrecheckSettings, PriorityFeeMode, SimulationSettings, MIN_CALL_GAS_LIMIT,
};

/// Main entry point for the CLI
///
/// Parses the CLI arguments and runs the appropriate subcommand.
/// Listens for a ctrl-c signal and shuts down all components when received.
pub async fn run() -> anyhow::Result<()> {
    let opt = Cli::parse();
    let _guard = tracing::configure_logging(&opt.logs)?;
    tracing::info!("Parsed CLI options: {:#?}", opt);

    let metrics_addr = format!("{}:{}", opt.metrics.host, opt.metrics.port).parse()?;
    prometheus_exporter::initialize(metrics_addr, &opt.metrics.tags)
        .context("metrics server should start")?;

    match opt.command {
        Command::Node(args) => node::run(args, opt.common).await?,
        Command::Pool(args) => pool::run(args, opt.common).await?,
        Command::Rpc(args) => rpc::run(args, opt.common).await?,
        Command::Builder(args) => builder::run(args, opt.common).await?,
    }

    tracing::info!("Shutdown, goodbye");
    Ok(())
}

/// CLI commands
#[derive(Debug, Subcommand)]
enum Command {
    /// Bundler command
    ///
    /// Runs the Pool, Builder, and RPC servers in a single process.
    #[command(name = "node")]
    Node(NodeCliArgs),

    /// Rpc command
    ///
    /// Runs the Rpc server
    #[command(name = "rpc")]
    Rpc(RpcCliArgs),

    /// Pool command
    ///
    /// Runs the Pool server
    #[command(name = "pool")]
    Pool(PoolCliArgs),

    /// Builder command
    ///
    /// Runs the Builder server
    #[command(name = "builder")]
    Builder(BuilderCliArgs),
}

/// CLI common options
#[derive(Debug, Args)]
#[command(next_help_heading = "Common")]
pub struct CommonArgs {
    /// Entry point address to target
    #[arg(
        long = "entry_points",
        name = "entry_points",
        env = "ENTRY_POINTS",
        default_values_t = Vec::<String>::new(), // required or will error
        value_delimiter = ',',
        global = true
    )]
    entry_points: Vec<String>,

    /// Chain ID to target
    #[arg(
        long = "chain_id",
        name = "chain_id",
        env = "CHAIN_ID",
        default_value = "1337",
        global = true
    )]
    chain_id: u64,

    /// ETH Node HTTP URL to connect to
    #[arg(
        long = "node_http",
        name = "node_http",
        env = "NODE_HTTP",
        global = true
    )]
    node_http: Option<String>,

    #[arg(
        long = "max_verification_gas",
        name = "max_verification_gas",
        default_value = "5000000",
        env = "MAX_VERIFICATION_GAS",
        global = true
    )]
    max_verification_gas: u64,

    #[arg(
        long = "max_bundle_gas",
        name = "max_bundle_gas",
        default_value = "25000000",
        env = "MAX_BUNDLE_GAS",
        global = true
    )]
    max_bundle_gas: u64,

    #[arg(
        long = "min_stake_value",
        name = "min_stake_value",
        env = "MIN_STAKE_VALUE",
        default_value = "1000000000000000000",
        global = true
    )]
    min_stake_value: u128,

    #[arg(
        long = "min_unstake_delay",
        name = "min_unstake_delay",
        env = "MIN_UNSTAKE_DELAY",
        default_value = "84600",
        global = true
    )]
    min_unstake_delay: u32,

    /// Amount of blocks to search when calling eth_getUserOperationByHash.
    /// Defaults from 0 to latest block
    #[arg(
        long = "user_operation_event_block_distance",
        name = "user_operation_event_block_distance",
        env = "USER_OPERATION_EVENT_BLOCK_DISTANCE",
        global = true
    )]
    user_operation_event_block_distance: Option<u64>,

    #[arg(
        long = "max_simulate_handle_ops_gas",
        name = "max_simulate_handle_ops_gas",
        env = "MAX_SIMULATE_HANDLE_OPS_GAS",
        default_value = "20000000",
        global = true
    )]
    max_simulate_handle_ops_gas: u64,

    #[arg(
        long = "use_bundle_priority_fee",
        name = "use_bundle_priority_fee",
        env = "USE_BUNDLE_PRIORITY_FEE"
    )]
    use_bundle_priority_fee: Option<bool>,

    #[arg(
        long = "bundle_priority_fee_overhead_percent",
        name = "bundle_priority_fee_overhead_percent",
        env = "BUNDLE_PRIORITY_FEE_OVERHEAD_PERCENT",
        default_value = "0"
    )]
    bundle_priority_fee_overhead_percent: u64,

    #[arg(
        long = "priority_fee_mode_kind",
        name = "priority_fee_mode_kind",
        env = "PRIORITY_FEE_MODE_KIND",
        value_parser = PossibleValuesParser::new(["base_fee_percent", "priority_fee_increase_percent"]),
        default_value = "priority_fee_increase_percent"
    )]
    priority_fee_mode_kind: String,

    #[arg(
        long = "priority_fee_mode_value",
        name = "priority_fee_mode_value",
        env = "PRIORITY_FEE_MODE_VALUE",
        default_value = "0"
    )]
    priority_fee_mode_value: u64,

    /// Interval at which the builder polls an Eth node for new blocks and
    /// mined transactions.
    #[arg(
        long = "eth_poll_interval_millis",
        name = "eth_poll_interval_millis",
        env = "ETH_POLL_INTERVAL_MILLIS",
        default_value = "100"
    )]
    pub eth_poll_interval_millis: u64,

    #[arg(
        long = "aws_region",
        name = "aws_region",
        env = "AWS_REGION",
        default_value = "us-east-1"
    )]
    aws_region: String,

    #[arg(
        long = "mempool_config_path",
        name = "mempool_config_path",
        env = "MEMPOOL_CONFIG_PATH"
    )]
    pub mempool_config_path: Option<String>,

    #[arg(
        long = "num_builders",
        name = "num_builders",
        env = "NUM_BUILDERS",
        default_value = "1"
    )]
    pub num_builders: u64,
}

const SIMULATION_GAS_OVERHEAD: u64 = 100_000;

impl TryFrom<&CommonArgs> for EstimationSettings {
    type Error = anyhow::Error;

    fn try_from(value: &CommonArgs) -> Result<Self, Self::Error> {
        if value.max_verification_gas
            > (value.max_simulate_handle_ops_gas - SIMULATION_GAS_OVERHEAD)
        {
            anyhow::bail!(
                "max_verification_gas ({}) must be less than max_simulate_handle_ops_gas ({}) by at least {}",
                value.max_verification_gas,
                value.max_simulate_handle_ops_gas,
                SIMULATION_GAS_OVERHEAD
            );
        }
        let max_call_gas = value.max_simulate_handle_ops_gas - value.max_verification_gas;
        if max_call_gas < MIN_CALL_GAS_LIMIT.as_u64() {
            anyhow::bail!(
                "max_simulate_handle_ops_gas ({}) must be greater than max_verification_gas ({}) by at least {MIN_CALL_GAS_LIMIT}",
                value.max_verification_gas,
                value.max_simulate_handle_ops_gas,
            );
        }
        Ok(Self {
            max_verification_gas: value.max_verification_gas,
            max_call_gas,
            max_simulate_handle_ops_gas: value.max_simulate_handle_ops_gas,
        })
    }
}

impl TryFrom<&CommonArgs> for PrecheckSettings {
    type Error = anyhow::Error;

    fn try_from(value: &CommonArgs) -> anyhow::Result<Self> {
        Ok(Self {
            chain_id: value.chain_id,
            max_verification_gas: value.max_verification_gas.into(),
            max_total_execution_gas: value.max_bundle_gas.into(),
            use_bundle_priority_fee: value.use_bundle_priority_fee,
            bundle_priority_fee_overhead_percent: value.bundle_priority_fee_overhead_percent,
            priority_fee_mode: PriorityFeeMode::try_from(
                value.priority_fee_mode_kind.as_str(),
                value.priority_fee_mode_value,
            )?,
        })
    }
}

impl From<&CommonArgs> for SimulationSettings {
    fn from(value: &CommonArgs) -> Self {
        Self::new(
            value.min_unstake_delay,
            value.min_stake_value,
            value.max_simulate_handle_ops_gas,
            value.max_verification_gas,
        )
    }
}

impl From<&CommonArgs> for EthApiSettings {
    fn from(value: &CommonArgs) -> Self {
        Self::new(value.user_operation_event_block_distance)
    }
}

/// CLI options for the metrics server
#[derive(Debug, Args)]
#[command(next_help_heading = "Metrics")]
pub struct MetricsArgs {
    /// Port to listen on for metrics requests
    #[arg(
        long = "metrics.port",
        name = "metrics.port",
        env = "METRICS_PORT",
        default_value = "8080",
        global = true
    )]
    port: u16,

    /// Host to listen on for metrics requests
    #[arg(
        long = "metrics.host",
        name = "metrics.host",
        env = "METRICS_HOST",
        default_value = "0.0.0.0",
        global = true
    )]
    host: String,

    /// Tags for metrics
    ///
    /// Format: key1=value1,key2=value2,...
    #[arg(
        long = "metrics.tags",
        name = "metrics.tags",
        env = "METRICS_TAGS",
        default_values_t = Vec::<String>::new(),
        value_delimiter = ',',
        global = true
    )]
    tags: Vec<String>,
}

/// CLI options for logging
#[derive(Debug, Args)]
#[command(next_help_heading = "Logging")]
pub struct LogsArgs {
    /// Log file
    ///
    /// If not provided, logs will be written to stdout
    #[arg(
        long = "log.file",
        name = "log.file",
        env = "LOG_FILE",
        default_value = None,
        global = true
    )]
    file: Option<String>,

    /// Log JSON
    ///
    /// If set, logs will be written in JSON format
    #[arg(
        long = "log.json",
        name = "log.json",
        env = "LOG_JSON",
        required = false,
        num_args = 0,
        global = true
    )]
    json: bool,
}

/// CLI options
#[derive(Debug, Parser)]
pub struct Cli {
    #[clap(subcommand)]
    command: Command,

    #[clap(flatten)]
    common: CommonArgs,

    #[clap(flatten)]
    metrics: MetricsArgs,

    #[clap(flatten)]
    logs: LogsArgs,
}
