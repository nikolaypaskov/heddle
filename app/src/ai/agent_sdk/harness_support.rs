//! `warp harness-support` — the callback CLI a harness uses from *inside* a run.
//!
//! Heddle (FOSS): every subcommand here (`ping`, `report-artifact`, `notify-user`,
//! `finish-task`, `report-shutdown`) exists so a harness executing inside a Warp cloud
//! run can report progress back to Warp's servers, keyed by the `--run-id` of that run.
//! There is no such run and no such server in this build, so the command reports that
//! plainly instead of failing with a connection error that reads like a transient outage.
//!
//! A harness running locally does not use this path — it is driven directly by
//! `agent_sdk::driver`.

use anyhow::Result;
use warp_cli::GlobalOptions;
use warp_cli::harness_support::HarnessSupportArgs;
use warpui::AppContext;

/// Run harness-support commands.
pub fn run(
    _ctx: &mut AppContext,
    _global_options: GlobalOptions,
    _args: HarnessSupportArgs,
) -> Result<()> {
    Err(anyhow::anyhow!(
        "`harness-support` reports a cloud run's progress back to Warp's servers, which \
         this build does not talk to."
    ))
}
