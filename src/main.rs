use std::fs::File;

use color_eyre::eyre::Result;

mod codegen;
mod lexer;
mod rules;

fn main() -> Result<()> {
    color_eyre::install()?;
    let rules = rules::parse_file("examples/test.pgrules")?;
    let lexer = lexer::Lexer::from_rules(&rules)?;
    codegen::cpp::gen_header_lexer(&lexer, &mut File::create("lexer.h").unwrap())?;
    codegen::cpp::gen_body_lexer(&lexer, &mut File::create("lexer.cpp").unwrap())?;
    Ok(())
}
