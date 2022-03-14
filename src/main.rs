use std::fs::File;

use color_eyre::eyre::Result;

mod codegen;
mod lexer;
mod rules;

fn main() -> Result<()> {
    color_eyre::install()?;
    let rules = rules::parse_file("examples/test.pgrules")?;
    let lexer = lexer::Lexer::from_rules(&rules)?;
    let cpp_config = codegen::cpp::CppConfig {
        support_cpp17: true,
    };
    codegen::cpp::gen_header_lexer(&lexer, &cpp_config, &mut File::create("lexer.h").unwrap())?;
    codegen::cpp::gen_body_lexer(&lexer, &cpp_config, &mut File::create("lexer.cpp").unwrap())?;
    Ok(())
}
