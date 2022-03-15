use std::{fs::File, path::Path};

use color_eyre::eyre::{bail, Result};
use fern::colors::{Color, ColoredLevelConfig};
use lexer::Lexer;
use log::info;

mod codegen;
mod lexer;
mod rules;

fn main() -> Result<()> {
    color_eyre::install()?;
    let colors = ColoredLevelConfig::new()
        .info(Color::Green)
        .warn(Color::Yellow)
        .error(Color::Red)
        .debug(Color::Blue);
    fern::Dispatch::new()
        .level(log::LevelFilter::Debug)
        .chain(
            fern::Dispatch::new()
                .format(move |out, msg, record| {
                    out.finish(format_args!(
                        "[{}] [{}] [{}] {}",
                        chrono::Local::now().format("%H:%M:%S.%3f %d.%m.%Y"),
                        record.target(),
                        colors.color(record.level()),
                        msg
                    ))
                })
                .chain(std::io::stdout()),
        )
        .chain(
            fern::Dispatch::new()
                .format(|out, msg, record| {
                    out.finish(format_args!(
                        "[{}] [{}] [{}] {}",
                        chrono::Local::now().format("%H:%M:%S.%3f %d.%m.%Y"),
                        record.target(),
                        record.level(),
                        msg
                    ))
                })
                .chain(
                    std::fs::OpenOptions::new()
                        .write(true)
                        .create(true)
                        .open("parge.log")?,
                ),
        )
        .apply()?;
    let matches = clap::Command::new("parge")
        .arg(
            clap::Arg::new("rules")
                .required(true)
                .help("The path of the rules file"),
        )
        .arg(
            clap::Arg::new("output")
                .short('o')
                .help("The parser output directory")
                .takes_value(true),
        )
        .arg(
            clap::Arg::new("lang")
                .short('l')
                .help("The language to generate")
                .required(true)
                .takes_value(true)
                .possible_values(["cpp", "rust", "java"]),
        )
        .get_matches();
    let language = matches.value_of("lang").unwrap();
    let output = matches
        .value_of("output")
        .map(|p| Path::new(p))
        .unwrap_or(Path::new("."));
    let rules = Path::new(matches.value_of("rules").unwrap());

    let parsed_rules = rules::parse_file(rules)?;
    let lexer = lexer::Lexer::from_rules(&parsed_rules)?;

    match language {
        "cpp" => generate_cpp(&lexer, output)?,
        l => bail!("Language currently not supported: {}", l),
    }
    Ok(())
}

fn generate_cpp(lexer: &Lexer, output: &Path) -> Result<()> {
    if !output.is_dir() {
        std::fs::create_dir_all(output)?;
    }
    let cpp_config = codegen::cpp::CppConfig {
        support_cpp17: true,
    };
    codegen::cpp::gen_header_lexer(
        &lexer,
        &cpp_config,
        &mut File::create(output.join("lexer.h")).unwrap(),
    )?;
    codegen::cpp::gen_body_lexer(
        &lexer,
        &cpp_config,
        &mut File::create(output.join("lexer.cpp")).unwrap(),
    )?;
    Ok(())
}
