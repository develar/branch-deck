//! The Disk window (spec/repository/disk-usage.spec.md): what the lease pools of this Mac and of the hosts of
//! `bd-host` hold on disk, and the trim of this Mac's pools. This Mac is `pool_disk` in process, with the home of the
//! host (`AppHost::tool_environment`, a fake one in the e2e server); the hosts are what `AppHost::host_disks` answers
//! (the app runs the `bd-host` it bundles). The view is words-ready numbers: sizes in MiB as `u32` (up to 4 PiB; the
//! bindings carry no `u64`, and an `f64` comes as `number | null`), the decisions of Rust (which slots a trim deletes next) as flags the page only shows.

use crate::context::AppContext;
use crate::operation::run_blocking;
use domain_types::error::ApiError;
use git_ops::revs::repository_dirs;
use lease_pool::lease::SlotState;
use lease_pool::trim::{TrimParams, trim_core};
use pool_disk::report::{DiskReport, PoolReport, SlotReport};
use pool_disk::{DiskParams, disk_usage_core};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use sync_utils::span::in_current_span;
use tracing::instrument;

/// How long `bd-host disk` may take: each host walks its pools, a minute for 1 TB
const HOSTS_TIMEOUT: Duration = Duration::from_mins(10);
/// The wait for a lock of a pool (`leases.lock`, `disk.lock`)
const LOCK_TIMEOUT: Duration = Duration::from_secs(10);
/// How long the trim waits for the Bazel servers of a slot it asked to exit
const SERVER_TIMEOUT: Duration = Duration::from_secs(30);

/// The disk report of a host of `bd-host`, or why it has none
#[derive(Debug, Clone)]
pub struct HostDisk {
  pub alias: String,
  pub report: Result<DiskReport, String>,
}

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DiskUsageParams {
  /// A checkout of a repository: its pool is reported wherever it lives, and its config gives the minimum
  pub repository_path: String,
  /// Walk every slot again, whatever `disk.json` keeps
  pub fresh: bool,
}

/// What the Disk window shows: this Mac first, then each host
#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DiskOverview {
  pub machines: Vec<MachineDisk>,
}

/// One machine and its pools
#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MachineDisk {
  /// The alias of the host; `null` for this Mac
  pub alias: Option<String>,
  /// Why the host has no report (no agent answers, an agent without the feature `disk`); the rest is empty then
  pub error: Option<String>,
  /// MiB
  pub free_mib: Option<u32>,
  /// `branchdeck.leaseMinFree` in MiB: below it a get trims the pools; `null` without one
  pub min_free_mib: Option<u32>,
  /// The MiB of its slots and orphans
  pub mib: u32,
  pub pools: Vec<PoolDisk>,
  /// The Bazel output bases of gone slots, which a trim deletes
  pub orphans: u32,
  pub orphan_mib: u32,
  /// The free slots a trim deletes now, in every pool
  pub past_limit: u32,
}

/// A pool of a machine
#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PoolDisk {
  /// The name of the repository: the directory of its main worktree
  pub name: String,
  /// The main worktree, else the common dir
  pub path: String,
  pub max_slots: u32,
  pub past_limit: u32,
  pub mib: u32,
  /// The Bazel output bases of the main worktree: no trim deletes them
  pub checkout_bases_mib: u32,
  pub slots: Vec<SlotDisk>,
}

/// A slot and the bytes its builds wrote
#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SlotDisk {
  /// The number of its directory in the pool (`<pool>/<n>/<name>`), else its worktree id
  pub name: String,
  pub path: String,
  pub state: SlotDiskState,
  pub holder: Option<String>,
  pub mib: u32,
  /// The directories its clean rules name (`target/`, `out/`)
  pub build_mib: u32,
  /// Its Bazel output bases
  pub bases_mib: u32,
  /// A trim deletes it now
  pub trims_next: bool,
}

/// What a slot is to the window
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SlotDiskState {
  Free,
  /// A session or a host run holds it
  Leased,
  /// A get, a return or a clean works on it now
  Changing,
}

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TrimPoolsParams {
  /// A checkout of a repository: its pool is trimmed wherever it lives, with every pool of `~/.branch-deck/pool`
  pub repository_path: String,
}

/// What the trim of this Mac's pools did
#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TrimSummary {
  pub removed_slots: u32,
  pub orphans: u32,
  pub freed_mib: u32,
  /// The free slots past the limit that stayed, as a process works in them
  pub busy: u32,
  /// What failed after a slot left its pool
  pub errors: Vec<String>,
}

/// This Mac and the hosts of `bd-host`, side by side: the walk of this Mac's pools in process, the hosts by the host
#[instrument(skip(ctx))]
pub async fn disk_usage(ctx: &AppContext, params: DiskUsageParams) -> Result<DiskOverview, ApiError> {
  let (git, host, home) = (ctx.git.clone(), std::sync::Arc::clone(&ctx.host), ctx.host.tool_environment().home);
  run_blocking(move || {
    let dirs = repository_dirs(&git, &params.repository_path)?.ok_or_else(|| anyhow::anyhow!("{} is no git repository", params.repository_path))?;
    let (this_mac, hosts) = std::thread::scope(|scope| {
      let hosts = scope.spawn(in_current_span(|| host.host_disks(params.fresh)));
      let disk = DiskParams {
        repository: &dirs,
        home: home.as_deref(),
        fresh: params.fresh,
        lock_timeout: LOCK_TIMEOUT,
      };
      let this_mac = disk_usage_core(&git, &disk).map(|usage| DiskReport::of(&usage));
      (this_mac, hosts.join().unwrap_or_default())
    });
    let mut machines = vec![machine(None, &this_mac?)];
    machines.extend(hosts.into_iter().map(|host| match host.report {
      Ok(report) => machine(Some(host.alias), &report),
      Err(error) => MachineDisk {
        alias: Some(host.alias),
        error: Some(error),
        free_mib: None,
        min_free_mib: None,
        mib: 0,
        pools: Vec::new(),
        orphans: 0,
        orphan_mib: 0,
        past_limit: 0,
      },
    }));
    Ok::<_, anyhow::Error>(DiskOverview { machines })
  })
  .await
}

/// The trim of the pools of this Mac (`lease_pool::trim`), as `bd lease trim` runs it
#[instrument(skip(ctx))]
pub async fn trim_pools(ctx: &AppContext, params: TrimPoolsParams) -> Result<TrimSummary, ApiError> {
  let (git, home) = (ctx.git.clone(), ctx.host.tool_environment().home);
  run_blocking(move || {
    let dirs = repository_dirs(&git, &params.repository_path)?.ok_or_else(|| anyhow::anyhow!("{} is no git repository", params.repository_path))?;
    let trim = TrimParams {
      repository: &dirs,
      home: home.as_deref(),
      max_slots: None,
      apply: true,
      server_timeout: SERVER_TIMEOUT,
      bazel: None,
      lock_timeout: LOCK_TIMEOUT,
    };
    let trimmed = trim_core(&git, &trim)?;
    let removed = trimmed.pools.iter().flat_map(|pool| &pool.removed);
    let errors = removed
      .clone()
      .flat_map(|slot| slot.errors.iter().cloned())
      .chain(trimmed.orphans.iter().filter_map(|base| base.error.clone()))
      .collect();
    Ok::<_, anyhow::Error>(TrimSummary {
      removed_slots: count(trimmed.removed_slots()),
      orphans: count(trimmed.orphans.len()),
      freed_mib: mib(trimmed.freed_bytes()),
      busy: count(trimmed.pools.iter().map(|pool| pool.skipped.len()).sum()),
      errors,
    })
  })
  .await
}

/// The hosts of the `bd-host` that the app bundles beside its binary (`externalBin`, macOS): `bd-host disk --json`,
/// which asks every host of the user's client one after the other. No such binary (a development build elsewhere, Linux,
/// Windows) has no hosts; a `bd-host` that fails says so as a host of the alias `bd-host`
pub fn bundled_host_disks(fresh: bool) -> Vec<HostDisk> {
  let Some(binary) = std::env::current_exe()
    .ok()
    .and_then(|exe| exe.parent().map(|dir| dir.join("bd-host")))
    .filter(|binary| binary.is_file())
  else {
    return Vec::new();
  };
  let mut command = sync_utils::child_process::windowless_command(&binary);
  command.args(["disk", "--json"]);
  if fresh {
    command.arg("--fresh");
  }
  match sync_utils::child_process::run_child(&mut command, None, HOSTS_TIMEOUT, &|| false) {
    // a run whose every host failed exits with the code of its refusal and still prints the hosts
    Ok(output) => hosts_of(&output.stdout).unwrap_or_else(|error| {
      vec![HostDisk {
        alias: "bd-host".to_owned(),
        report: Err(format!("{error}: {}", output.failure_message("bd-host disk"))),
      }]
    }),
    Err(error) => vec![HostDisk {
      alias: "bd-host".to_owned(),
      report: Err(format!("cannot run {}: {error:?}", binary.display())),
    }],
  }
}

/// The hosts of the output of `bd-host disk --json`: `{"hosts": [{"alias", "report"} or {"alias", "refusal"}]}`
pub fn hosts_of(stdout: &[u8]) -> Result<Vec<HostDisk>, serde_json::Error> {
  #[derive(Deserialize)]
  struct Output {
    hosts: Vec<Host>,
  }
  #[derive(Deserialize)]
  struct Host {
    alias: String,
    report: Option<DiskReport>,
    refusal: Option<Refusal>,
  }
  #[derive(Deserialize)]
  struct Refusal {
    message: String,
  }
  let output: Output = serde_json::from_slice(stdout)?;
  Ok(
    output
      .hosts
      .into_iter()
      .map(|host| HostDisk {
        report: match (host.report, host.refusal) {
          (Some(report), _) => Ok(report),
          (None, refusal) => Err(refusal.map_or_else(|| "the host answered no report".to_owned(), |refusal| refusal.message)),
        },
        alias: host.alias,
      })
      .collect(),
  )
}

fn machine(alias: Option<String>, report: &DiskReport) -> MachineDisk {
  let pools: Vec<PoolDisk> = report.pools.iter().map(pool).collect();
  MachineDisk {
    alias,
    error: None,
    free_mib: report.free_bytes.map(mib),
    min_free_mib: report.min_free.map(mib),
    mib: mib(report.bytes),
    past_limit: pools.iter().map(|pool| pool.past_limit).sum(),
    pools,
    orphans: count(report.orphans.len()),
    orphan_mib: mib(report.orphans.iter().map(|base| base.bytes).sum()),
  }
}

fn pool(pool: &PoolReport) -> PoolDisk {
  let path = pool.repository.clone().unwrap_or_else(|| pool.git_common_dir.clone());
  PoolDisk {
    name: Path::new(&path).file_name().map_or_else(|| path.clone(), |name| name.to_string_lossy().into_owned()),
    path,
    max_slots: count(pool.max_slots),
    past_limit: count(pool.past_limit),
    mib: mib(pool.bytes),
    checkout_bases_mib: mib(pool.checkout_bases.iter().map(|base| base.bytes).sum()),
    slots: pool.slots.iter().map(slot).collect(),
  }
}

fn slot(slot: &SlotReport) -> SlotDisk {
  let number = Path::new(&slot.path).parent().and_then(Path::file_name).map(|name| name.to_string_lossy().into_owned());
  SlotDisk {
    name: number.filter(|name| name.parse::<u32>().is_ok()).unwrap_or_else(|| slot.slot.clone()),
    path: slot.path.clone(),
    state: match slot.state {
      SlotState::Free | SlotState::Abandoned => SlotDiskState::Free,
      SlotState::Ready => SlotDiskState::Leased,
      SlotState::Preparing | SlotState::Templating | SlotState::Returning | SlotState::Cleaning => SlotDiskState::Changing,
    },
    holder: slot.holder.clone(),
    mib: mib(slot.bytes),
    build_mib: mib(slot.build.iter().map(|dir| dir.bytes).sum()),
    bases_mib: mib(slot.output_bases.iter().map(|base| base.bytes).sum()),
    trims_next: slot.trims_next,
  }
}

/// `bytes` in MiB, rounded up so a few bytes are not nothing, at most `u32::MAX` (4 PiB)
fn mib(bytes: u64) -> u32 {
  u32::try_from(bytes.div_ceil(1 << 20)).unwrap_or(u32::MAX)
}

fn count(value: usize) -> u32 {
  u32::try_from(value).unwrap_or(u32::MAX)
}

#[cfg(test)]
#[path = "disk_test.rs"]
mod disk_test;
