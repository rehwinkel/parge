use std::{collections::HashSet, io::Write};

use color_eyre::Result;
use smol_str::SmolStr;

use crate::lexer::Lexer;

pub fn gen_header_lexer<W: Write>(lexer: &Lexer, writer: &mut W) -> Result<()> {
    let tokens: HashSet<SmolStr> = lexer
        .get_states()
        .iter()
        .filter(|s| s.is_some())
        .map(|s| s.unwrap().clone())
        .collect();
    write!(
        writer,
        r#"#include <cstdint>
#include <string>

enum class Token
{{
    _EOF,
    {}
}};

class Lexer
{{
private:
    std::string contents;
    size_t pos;
    uint32_t next_chr();
    void rev_chr();

public:
    Lexer(std::string contents);
    Token next();
}};
"#,
        tokens
            .into_iter()
            .collect::<Vec<SmolStr>>()
            .join(",\r\n    ")
    )?;
    Ok(())
}

pub fn gen_body_lexer<W: Write>(lexer: &Lexer, writer: &mut W) -> Result<()> {
    write!(
        writer,
        r#"#include "lexer.h"
#include <system_error>

uint32_t Lexer::next_chr() {{
    if (this->pos < this->contents.size()) {{
        return (uint32_t)this->contents[this->pos++];
    }} else {{
        return -1;
    }}
}}

void Lexer::rev_chr() {{
    this->pos--;
}}

Lexer::Lexer(std::string content) : contents(content), pos(0) {{}}

Token Lexer::next()
{{
    size_t state = 0;
    while (1)
    {{
        uint32_t ch = this->next_chr();

        switch (state) {{
"#
    )?;
    let trap = lexer
        .get_states()
        .iter()
        .position(|s| match s {
            Some(s) if s == &"_TRAP" => true,
            _ => false,
        })
        .unwrap();
    for (i, acc) in lexer.get_states().iter().enumerate() {
        write!(writer, "            case {}:\r\n", i)?;
        if i == trap {
            write!(writer, "                return Token::_TRAP;\r\n")?;
        } else {
            write!(writer, "                switch (ch) {{\r\n")?;
            for (r0, r1, result) in lexer.get_connections(i) {
                if r0 == r1 {
                    write!(writer, "                    case {}:\r\n", r0)?;
                } else {
                    write!(writer, "                    case {} ... {}:\r\n", r0, r1)?;
                }
                if result == trap {
                    if let Some(acc) = acc {
                        write!(writer, "                        this->rev_chr();\r\n")?;
                        write!(writer, "                        return Token::{};\r\n", acc)?;
                    } else {
                        write!(writer, "                        return Token::_TRAP;\r\n")?;
                    }
                } else {
                    write!(writer, "                        state = {};\r\n", result)?;
                    write!(writer, "                        break;\r\n")?;
                }
            }
            if let Some(acc) = acc {
                write!(writer, "                    case (uint32_t) -1:\r\n")?;
                write!(writer, "                        return Token::{};\r\n", acc)?;
            }
            write!(writer, "                }}\r\n")?;
        }
        write!(writer, "                break;\r\n")?;
    }
    write!(
        writer,
        "        }}\r\n\r\n        if (ch == -1) return Token::_EOF;\r\n    }}\r\n}}\r\n"
    )?;
    Ok(())
}
