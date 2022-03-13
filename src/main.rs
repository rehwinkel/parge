use color_eyre::eyre::Result;

mod rules;
mod lexer;

fn main() -> Result<()> {
    color_eyre::install()?;
    let rules = rules::parse_file("examples/test.pgrules")?;
    lexer::Lexer::from_rules(&rules);
    Ok(())
}
