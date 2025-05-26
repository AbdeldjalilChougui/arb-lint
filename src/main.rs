use anyhow::{bail, Context, Result};
use arb_lint::lint::{self, Options, Project, Severity};
use arb_lint::{arb_files, discover, fix, pick_template, report};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Text,
    Json,
    Sarif,
}

#[derive(Parser)]
#[command(name = "arb-lint", version, about = "Lint and format Flutter ARB localization files")]
struct Cli {
    /// Flutter project root (uses l10n.yaml / lib/l10n) or a directory of .arb files
    #[arg(default_value = ".")]
    path: PathBuf,
    /// Template file name (default: from l10n.yaml, else app_en.arb)
    #[arg(long)]
    template: Option<String>,
    /// Directory of Dart sources to scan for unused keys (default: <project>/lib if found)
    #[arg(long)]
    dart_root: Option<PathBuf>,
    /// Do not look for unused keys
    #[arg(long)]
    no_unused: bool,
    /// Report template messages without @key.description
    #[arg(long)]
    require_descriptions: bool,
    #[arg(long, value_enum, default_value = "text")]
    format: Format,
    /// Rewrite files in canonical form (template key order, @key after its message, @@locale)
    #[arg(long)]
    fix: bool,
    /// Exit with status 1 on warnings too
    #[arg(long)]
    deny_warnings: bool,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("arb-lint: {e:#}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode> {
    let cli = Cli::parse();
    let layout = discover(&cli.path);
    let paths = arb_files(&layout.arb_dir).with_context(|| format!("reading {}", layout.arb_dir.display()))?;
    if paths.is_empty() {
        bail!("no .arb files in {}", layout.arb_dir.display());
    }
    let mut diags = Vec::new();
    let files = lint::load(&paths, &mut diags);
    let configured = cli.template.clone().or(layout.template.clone());
    let Some(template) = pick_template(&files, configured.as_deref()) else {
        bail!("template {} not found (or not valid JSON) in {}", configured.unwrap_or_default(), layout.arb_dir.display());
    };
    let mut project = Project { files, template };

    if cli.fix {
        let order: Vec<String> = project.files[template].messages().map(|(k, _)| k.clone()).collect();
        let mut changed = 0;
        for f in &project.files {
            let formatted = fix::format(f, &order);
            if formatted != f.text {
                std::fs::write(&f.path, &formatted).with_context(|| format!("writing {}", f.path.display()))?;
                changed += 1;
            }
        }
        eprintln!("arb-lint: formatted {changed} file(s)");
        // Lint the rewritten files so reported positions match.
        diags.clear();
        project.files = lint::load(&paths, &mut diags);
    }

    let opts = Options {
        require_descriptions: cli.require_descriptions,
        dart_root: if cli.no_unused { None } else { cli.dart_root.clone().or(layout.dart_root.clone()) },
    };
    lint::lint(&project, &opts, &mut diags);
    let out = match cli.format {
        Format::Text => report::text(&diags),
        Format::Json => report::json(&diags),
        Format::Sarif => report::sarif(&diags),
    };
    print!("{out}");
    let worst = diags.iter().map(|d| d.severity).max();
    let fail = match worst {
        Some(Severity::Error) => true,
        Some(Severity::Warning) => cli.deny_warnings,
        _ => false,
    };
    Ok(if fail { ExitCode::from(1) } else { ExitCode::SUCCESS })
}
