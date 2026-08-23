mod environment;
mod fixture;
mod model;
mod process;
mod report;
mod runner;
mod suite;

use std::{env, path::PathBuf};

use runner::RunOptions;
use suite::Suite;

fn usage() -> &'static str {
    "Usage:\n  pana-performance-benchmark run [--suite smoke|standard|soak] [--project-root PATH] [--fixture-root PATH] [--output-root PATH] [--app-binary PATH] [--keep-fixtures]\n  pana-performance-benchmark report --raw PATH --json PATH --markdown PATH [--baseline REPORT.json]"
}

fn required_value(arguments: &[String], index: &mut usize, flag: &str) -> Result<String, String> {
    *index += 1;
    arguments
        .get(*index)
        .cloned()
        .ok_or_else(|| format!("Lipsește valoarea pentru {flag}."))
}

fn default_fixture_root(project_root: &std::path::Path) -> PathBuf {
    project_root
        .join("tests")
        .join("fixtures")
        .join("projects")
        .join("index-zero")
}

fn run_command(arguments: &[String]) -> Result<(), String> {
    let current = env::current_dir().map_err(|error| error.to_string())?;
    let mut suite = Suite::Smoke;
    let mut project_root = current.clone();
    let mut fixture_root: Option<PathBuf> = None;
    let mut output_root = current.join("benchmark-results");
    let mut keep_fixtures = false;
    let mut app_binary = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--suite" => suite = Suite::parse(&required_value(arguments, &mut index, "--suite")?)?,
            "--project-root" => {
                project_root =
                    PathBuf::from(required_value(arguments, &mut index, "--project-root")?)
            }
            "--fixture-root" => {
                fixture_root = Some(PathBuf::from(required_value(
                    arguments,
                    &mut index,
                    "--fixture-root",
                )?))
            }
            "--output-root" => {
                output_root = PathBuf::from(required_value(arguments, &mut index, "--output-root")?)
            }
            "--keep-fixtures" => keep_fixtures = true,
            "--app-binary" => {
                app_binary = Some(PathBuf::from(required_value(
                    arguments,
                    &mut index,
                    "--app-binary",
                )?))
            }
            unknown => return Err(format!("Argument necunoscut: {unknown}\n{}", usage())),
        }
        index += 1;
    }
    project_root = project_root
        .canonicalize()
        .map_err(|error| format!("Rădăcină repo invalidă: {error}"))?;
    let fixture_root = fixture_root
        .unwrap_or_else(|| default_fixture_root(&project_root))
        .canonicalize()
        .map_err(|error| format!("Rădăcină INDEX ZERO invalidă: {error}"))?;
    let run_root = runner::run(RunOptions {
        project_root,
        canonical_fixture_root: fixture_root,
        output_root,
        suite,
        keep_fixtures,
        app_binary,
    })?;
    println!("[pana-benchmark] rezultat={}", run_root.display());
    Ok(())
}

fn report_command(arguments: &[String]) -> Result<(), String> {
    let mut raw = None;
    let mut json = None;
    let mut markdown = None;
    let mut baseline = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--raw" => {
                raw = Some(PathBuf::from(required_value(
                    arguments, &mut index, "--raw",
                )?))
            }
            "--json" => {
                json = Some(PathBuf::from(required_value(
                    arguments, &mut index, "--json",
                )?))
            }
            "--markdown" => {
                markdown = Some(PathBuf::from(required_value(
                    arguments,
                    &mut index,
                    "--markdown",
                )?))
            }
            "--baseline" => {
                baseline = Some(PathBuf::from(required_value(
                    arguments,
                    &mut index,
                    "--baseline",
                )?))
            }
            unknown => return Err(format!("Argument necunoscut: {unknown}\n{}", usage())),
        }
        index += 1;
    }
    report::write_reports_with_baseline(
        raw.as_deref().ok_or("Lipsește --raw.")?,
        json.as_deref().ok_or("Lipsește --json.")?,
        markdown.as_deref().ok_or("Lipsește --markdown.")?,
        baseline.as_deref(),
    )
}

fn main() {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    let result = match arguments.first().map(String::as_str) {
        Some("run") => run_command(&arguments[1..]),
        Some("report") => report_command(&arguments[1..]),
        Some("--help" | "-h") | None => {
            println!("{}", usage());
            Ok(())
        }
        Some(command) => Err(format!("Comandă necunoscută: {command}\n{}", usage())),
    };
    if let Err(error) = result {
        eprintln!("[pana-benchmark] {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_ul_implicit_este_in_catalogul_central() {
        let root = PathBuf::from("/repo/pana-studio");
        assert_eq!(
            default_fixture_root(&root),
            root.join("tests/fixtures/projects/index-zero")
        );
    }
}
