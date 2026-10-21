use ccometixline::cli::{Cli, Command, PreviewArgs};
use ccometixline::config::{self, Config, DepthSetting, LoadNote};
use ccometixline::env::{AutoCompact, ClaudeSettings};
use ccometixline::input::{Input, MAX_INPUT_BYTES};
use ccometixline::metrics::GitSource;
use ccometixline::models::Registry;
use ccometixline::render::ansi::{self, EmitOptions};
use ccometixline::render::span::{Span, Style};
use ccometixline::state::{self, StateView};
use ccometixline::statusline::{self, Frame};
use ccometixline::style::palette::Token;
use ccometixline::{presets, scenarios};
use chrono::{Local, Utc};
use std::io::{self, IsTerminal, Read};

fn main() {
    let cli = Cli::parse_args();

    if cli.patch.is_some() {
        println!("ccline v2 no longer patches Claude Code; --patch was removed.");
        return;
    }

    match cli.command {
        Some(Command::Preview(args)) => return preview(args, cli.theme.as_deref()),
        Some(Command::Update) => return update(),
        Some(Command::RefreshGit { dir }) => return state::git_cache::refresh(&dir),
        Some(Command::UpdateCheck) => {
            state::update::check(Utc::now().timestamp());
            return;
        }
        None => {}
    }

    if cli.config || io::stdin().is_terminal() {
        println!("ccline v2 is under development: the configuration TUI is not available yet.");
        println!("Try `ccline preview --all` to see your config against sample sessions.");
        return;
    }

    run(cli.theme.as_deref());
}

fn load_config(theme: Option<&str>) -> (Config, LoadNote) {
    let (config, note) = Config::load();
    match theme.and_then(|name| config::apply_theme(config.clone(), name)) {
        Some(themed) => (themed, note),
        None => (config, note),
    }
}

fn run(theme: Option<&str>) {
    let mut text = String::new();
    let _ = io::stdin()
        .take(MAX_INPUT_BYTES as u64)
        .read_to_string(&mut text);
    let input = match Input::parse(&text) {
        Ok(input) => input,
        Err(e) => {
            println!("\x1b[0mccline: could not read the status line input ({e})\x1b[0m");
            return;
        }
    };
    let (config, note) = load_config(theme);
    let registry = Registry::load();
    let settings = ClaudeSettings::for_input(&input);
    let (now, tz) = (Utc::now().timestamp(), *Local::now().offset());
    let mut view = state::observe(&input, &text, &registry, now, tz);
    view.latest_version = state::update::available(&config.update, now);
    let frame = Frame {
        now,
        tz,
        width: statusline::budget(&config, &settings),
        git: GitSource::Cached(config.git.cache_ms),
        state: view,
        compact: settings.auto_compact(),
    };
    let mut lines = statusline::lines(&input, &config, &registry, frame);
    if let LoadNote::Invalid(_) = note {
        let warn = statusline::theme(&config).palette.color(Token::Warn);
        let mark = Span::styled(
            " ! ccline config",
            Style {
                fg: warn,
                ..Style::default()
            },
        );
        match lines.first_mut() {
            Some(first) => first.push(mark),
            None => lines.push(vec![mark]),
        }
    }
    let opts = EmitOptions {
        depth: config.color_depth.resolve(),
        guards: true,
    };
    println!("{}", ansi::emit(&lines, opts));
}

fn update() {
    use state::update::{CURRENT, RELEASES, is_newer};
    match state::update::check(Utc::now().timestamp()) {
        Some(latest) if is_newer(&latest, CURRENT) => {
            println!("ccline {latest} is available (running {CURRENT}).");
            println!("Upgrade:       npm update -g @cometix/ccline");
            println!("Release notes: {RELEASES}/tag/v{latest}");
        }
        Some(latest) => println!("ccline {CURRENT} is up to date (latest published: {latest})."),
        None => {
            eprintln!("ccline: could not reach the npm registry or GitHub; try again later.");
            std::process::exit(1);
        }
    }
}

fn preview(args: PreviewArgs, theme: Option<&str>) {
    if args.list {
        println!("Scenarios:");
        for s in scenarios::SCENARIOS {
            println!("  {:<12} {}", s.name, s.summary);
        }
        println!("Presets (use with --theme):");
        for p in presets::PRESETS {
            println!("  {:<12} {}", p.name, p.summary);
        }
        return;
    }
    let (config, _) = load_config(theme);
    let depth = match args.color.as_str() {
        "truecolor" => DepthSetting::TrueColor,
        "256" => DepthSetting::Ansi256,
        "16" => DepthSetting::Ansi16,
        "none" => DepthSetting::None,
        _ => config.color_depth,
    }
    .resolve();
    let width = args
        .width
        .or_else(ccometixline::env::columns)
        .unwrap_or(100);
    let registry = Registry::load();
    let now = Utc::now().timestamp();
    let tz = *Local::now().offset();
    let opts = EmitOptions {
        depth,
        guards: true,
    };

    let render = |input: &Input, git: GitSource, state: StateView, compact: AutoCompact| {
        let frame = Frame {
            now,
            tz,
            width: Some(width),
            git,
            state,
            compact,
        };
        ansi::emit(&statusline::lines(input, &config, &registry, frame), opts)
    };
    let scenario = |s: &scenarios::Scenario| {
        render(
            &s.input(now),
            GitSource::Fixed(s.git()),
            s.state(now),
            AutoCompact::default(),
        )
    };

    if args.all {
        for s in scenarios::SCENARIOS {
            println!("\x1b[2m── {} · {}\x1b[0m", s.name, s.summary);
            println!("{}\n", scenario(s));
        }
        return;
    }
    if let Some(s) = scenarios::find(&args.scenario) {
        println!("{}", scenario(s));
        return;
    }
    match std::fs::read_to_string(&args.scenario).map(|t| Input::parse(&t)) {
        Ok(Ok(input)) => {
            // A real payload renders against the live environment, but
            // previewing must not record anything.
            let compact = ClaudeSettings::for_input(&input).auto_compact();
            println!(
                "{}",
                render(&input, GitSource::Live, StateView::default(), compact)
            )
        }
        Ok(Err(e)) => eprintln!("ccline: {} is not status line JSON: {e}", args.scenario),
        Err(_) => eprintln!(
            "ccline: no scenario or file named {:?}; see `ccline preview --list`",
            args.scenario
        ),
    }
}
