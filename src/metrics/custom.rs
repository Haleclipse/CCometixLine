//! `cmd`: the first line a shell command prints. Options: `run` (the
//! command line, required), `cache_s` (how long output stays fresh,
//! default 10), `timeout_ms` (limit for a run, default 2000) and `sample`
//! (what previews show instead of running it).

use super::{Commands, Ctx, Metric, MetricMeta, Options, Reading};
use crate::state::cmd_cache::{self, Job};
use crate::style::{Category, IconKey};
use crate::widgets::WidgetKind;
use std::path::Path;
use std::time::Duration;

const DEFAULT_CACHE_S: u64 = 10;
const DEFAULT_TIMEOUT_MS: u64 = 2_000;

pub(super) fn metrics() -> Vec<Metric> {
    vec![Metric {
        meta: MetricMeta {
            id: "cmd",
            category: Category::Custom,
            name: "Command",
            summary: "First line printed by a shell command, which gets the status line JSON on stdin",
            icon: IconKey::Command,
            widget: WidgetKind::Text,
            templates: ["{out}", "{out}", "{out}"],
            fields: &["out"],
            levels: &[],
            min_cc: None,
            needs_mod: false,
            stateful: false,
            priority: 2,
        },
        compute: cmd,
    }]
}

fn number(opts: &Options, key: &str) -> Option<u64> {
    opts.get(key)?
        .as_integer()
        .and_then(|n| u64::try_from(n).ok())
}

fn cmd(ctx: &Ctx, opts: &Options) -> Option<Reading> {
    let run = opts.get("run")?.as_str()?.trim();
    if run.is_empty() {
        return None;
    }
    let out = match &ctx.commands {
        Commands::Run { stdin } => cmd_cache::output(&Job {
            run,
            cwd: ctx.input.current_dir().map(Path::new),
            stdin,
            timeout: Duration::from_millis(
                number(opts, "timeout_ms").unwrap_or(DEFAULT_TIMEOUT_MS),
            ),
            ttl_ms: number(opts, "cache_s").unwrap_or(DEFAULT_CACHE_S) * 1_000,
        })?,
        Commands::Show => match opts.get("sample").and_then(|s| s.as_str()) {
            Some(sample) => sample.to_string(),
            None => format!("$ {}", crate::render::width::truncate(run, 24)),
        },
    };
    Some(Reading::default().set("out", out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Input;
    use crate::metrics::{find, read};
    use crate::models::Registry;
    use crate::style::GlyphTier;
    use chrono::FixedOffset;

    fn show(opts: &str) -> Option<String> {
        let input = Input::default();
        let reg = Registry::default();
        let ctx = Ctx::new(
            &input,
            &reg,
            0,
            FixedOffset::east_opt(0).unwrap(),
            GlyphTier::Unicode,
        );
        let opts: Options = toml::from_str(opts).unwrap();
        read(find("cmd").unwrap(), &ctx, &opts).map(|r| r.get("out").unwrap().text.clone())
    }

    #[test]
    fn previews_show_the_command_or_a_sample() {
        assert_eq!(
            show(r#"run = "kubectl config current-context""#).as_deref(),
            Some("$ kubectl config current-…")
        );
        assert_eq!(
            show("run = \"kubectl config current-context\"\nsample = \"prod-eu\"").as_deref(),
            Some("prod-eu")
        );
        assert_eq!(show(r#"run = "  ""#), None);
        assert_eq!(show(""), None);
    }
}
