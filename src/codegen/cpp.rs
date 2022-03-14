use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
};

use color_eyre::Result;
use smol_str::SmolStr;

use crate::lexer::Lexer;

pub struct CppConfig {
    pub support_cpp17: bool,
}

pub fn gen_header_lexer<W: Write>(lexer: &Lexer, config: &CppConfig, writer: &mut W) -> Result<()> {
    let tokens: BTreeSet<SmolStr> = lexer
        .get_states()
        .iter()
        .filter(|s| s.is_some())
        .map(|s| s.unwrap().clone())
        .collect();
    write!(writer, "#include <cstdint>\r\n#include <string>\r\n")?;
    if config.support_cpp17 {
        write!(writer, "#include <string_view>\r\n")?;
    }
    write!(
        writer,
        r#"
enum class Token
{{
    _EOF,
    {}
}};

class Lexer
{{
private:
    std::string contents;
    size_t start_pos;
    size_t pos;
    uint32_t next_chr();
    void rev_chr();

public:
    Lexer(std::string contents);
    Token next();"#,
        tokens
            .into_iter()
            .collect::<Vec<SmolStr>>()
            .join(",\r\n    ")
    )?;
    if config.support_cpp17 {
        write!(writer, "\r\n    std::string_view str();\r\n")?;
    } else {
        write!(writer, "\r\n    std::string str();\r\n")?;
    }
    write!(writer, "}};\r\n")?;
    Ok(())
}

macro_rules! write_line {
    ($indent:expr,$writer:expr,$($arg:tt)*) => {
        for _ in 0..$indent {
            write!($writer, "    ")?;
        }
        write!($writer, $($arg)*)?;
    };
}

pub fn gen_body_lexer<W: Write>(lexer: &Lexer, config: &CppConfig, writer: &mut W) -> Result<()> {
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

"#
    )?;
    if config.support_cpp17 {
        write!(writer, "std::string_view Lexer::str() {{\r\n")?;
        write!(
            writer,
            "    return std::string_view(this->contents.data() + this->start_pos, this->pos - this->start_pos);\r\n"
        )?;
    } else {
        write!(writer, "std::string Lexer::str() {{\r\n")?;
        write!(
            writer,
            "    return this->contents.substr(this->start_pos, this->pos - this->start_pos);\r\n"
        )?;
    }
    write!(writer, "}}\r\n")?;
    write!(
        writer,
        r#"
Token Lexer::next()
{{
    this->start_pos = this->pos;
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
        write_line!(3, writer, "case {}:\r\n", i);
        if i == trap {
            write_line!(4, writer, "return Token::_TRAP;\r\n");
        } else {
            write_line!(4, writer, "switch (ch) {{\r\n");
            let mut results: BTreeMap<usize, Vec<(u32, u32)>> = BTreeMap::new();
            for (r0, r1, result) in lexer.get_connections(i) {
                if let Some(result) = results.get_mut(&result) {
                    result.push((r0, r1));
                } else {
                    results.insert(result, vec![(r0, r1)]);
                }
            }
            for (result, ranges) in results {
                for (r0, r1) in ranges {
                    if r0 == r1 {
                        write_line!(5, writer, "case {}:\r\n", r0);
                    } else {
                        write_line!(5, writer, "case {} ... {}:\r\n", r0, r1);
                    }
                }
                if result == trap {
                    if let Some(acc) = acc {
                        write_line!(6, writer, "this->rev_chr();\r\n");
                        write_line!(6, writer, "return Token::{};\r\n", acc);
                    } else {
                        write_line!(6, writer, "return Token::_TRAP;\r\n");
                    }
                } else {
                    write_line!(6, writer, "state = {};\r\n", result);
                    write_line!(6, writer, "break;\r\n");
                }
            }
            if let Some(acc) = acc {
                write_line!(5, writer, "case (uint32_t) -1:\r\n");
                write_line!(6, writer, "return Token::{};\r\n", acc);
            }
            write_line!(4, writer, "}}\r\n");
        }
        write_line!(4, writer, "break;\r\n");
    }
    write!(writer, "        }}\r\n\r\n")?;
    write!(writer, "        if (ch == -1) return Token::_EOF;\r\n")?;
    write!(writer, "    }}\r\n}}\r\n")?;
    Ok(())
}
