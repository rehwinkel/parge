use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
};

use color_eyre::Result;
use smol_str::SmolStr;

use crate::lexer::Lexer;

pub fn gen_header_lexer<W: Write>(lexer: &Lexer, writer: &mut W) -> Result<()> {
    let tokens: BTreeSet<SmolStr> = lexer
        .get_states()
        .iter()
        .filter(|s| s.is_some())
        .map(|s| s.unwrap().clone())
        .collect();
    write!(
        writer,
        r#"#include <cstdint>
#include <string>
#include <istream>
#include <sstream>

enum class Token
{{
    _EOF,
    _ERR,
    {}
}};

class Lexer
{{
private:
    std::stringstream buf;
    std::istream &contents;
    uint32_t next_chr(int *err, bool &use_buf);
    void read(bool &use_buf, char *dst, size_t n);

public:
    Lexer(std::istream &contents);
    std::string next(Token &token);
}};
"#,
        tokens
            .into_iter()
            .collect::<Vec<SmolStr>>()
            .join(",\r\n    ")
    )?;
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

pub fn gen_body_lexer<W: Write>(lexer: &Lexer, writer: &mut W) -> Result<()> {
    let trap = lexer
        .get_states()
        .iter()
        .position(|s| match s {
            Some(s) if s == &"_TRAP" => true,
            _ => false,
        })
        .unwrap();

    write!(
        writer,
        r#"#include "lexer.h"
#include <system_error>
#include <sstream>

void Lexer::read(bool &use_buf, char *dst, size_t n)
{{
    if (n == 0)
        return;
    if (use_buf)
    {{
        size_t read = this->buf.readsome(dst, n);
        if (read < n || !this->buf.rdbuf()->in_avail())
        {{
            use_buf = false;
            this->contents.read(dst + read, n - read);
        }}
    }}
    else
    {{
        this->contents.read(dst, n);
    }}
}}

// taken from: https://github.com/skeeto/branchless-utf8
uint32_t Lexer::next_chr(int *e, bool &use_buf)
{{
    uint32_t ch = 0;
    uint32_t *c = &ch;
    static const char lengths[] = {{
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
        0, 0, 0, 0, 0, 0, 0, 0, 2, 2, 2, 2, 3, 3, 4, 0}};
    static const int masks[] = {{0x00, 0x7f, 0x1f, 0x0f, 0x07}};
    static const uint32_t mins[] = {{4194304, 0, 128, 2048, 65536}};
    static const int shiftc[] = {{0, 18, 12, 6, 0}};
    static const int shifte[] = {{0, 6, 4, 2, 0}};

    char s[4] = {{0}};
    this->read(use_buf, s, 1);
    int len = lengths[s[0] >> 3];
    if (len)
        this->read(use_buf, s + 1, len - 1);

    /* Assume a four-byte character and load four bytes. Unused bits are
     * shifted out.
     */
    *c = (uint32_t)(s[0] & masks[len]) << 18;
    *c |= (uint32_t)(s[1] & 0x3f) << 12;
    *c |= (uint32_t)(s[2] & 0x3f) << 6;
    *c |= (uint32_t)(s[3] & 0x3f) << 0;
    *c >>= shiftc[len];

    /* Accumulate the various error conditions. */
    *e = (*c < mins[len]) << 6;      // non-canonical encoding
    *e |= ((*c >> 11) == 0x1b) << 7; // surrogate half?
    *e |= (*c > 0x10FFFF) << 8;      // out of range?
    *e |= (s[1] & 0xc0) >> 2;
    *e |= (s[2] & 0xc0) >> 4;
    *e |= (s[3]) >> 6;
    *e ^= 0x2a; // top two bits of each tail byte correct?
    *e >>= shifte[len];

    return ch;
}}

int push_utf8(std::ostream &s, uint32_t cp)
{{
    int len = 4 - ((cp < 0x80) + (cp < 0x800) + (cp < 0x10000));
    char chs[4] = {{0}};
    chs[(len - 3 >= 0) * len - 3] = (0b10000000 | ((cp >> 12) & 0x3f));
    chs[(len - 2 >= 0) * len - 2] = (0b10000000 | ((cp >> 6) & 0x3f));
    chs[len - 1] = (0b10000000 | (cp & 0x3f));
    chs[0] = cp * (len == 1) +
             (0b11000000 | ((cp >> 6) & 0x1f)) * (len == 2) +
             (0b11100000 | ((cp >> 12) & 0xf)) * (len == 3) +
             (0b11110000 | ((cp >> 18) & 0x7)) * (len == 4);
    s.write(chs, len);
    return len;
}}

Lexer::Lexer(std::istream &contents) : contents(contents) {{}}

std::string Lexer::next(Token &token)
{{
    Token found = Token::_TRAP;
    size_t found_pos = 0;

    size_t pos = 0;
    size_t state = 0;
    bool use_buf = this->buf.rdbuf()->in_avail();
    while (1)
    {{
        if (state == {}) {{
            std::string s(found_pos, '\0');
            this->buf.read(&s[0], found_pos);
            token = found;
            return s;
        }}

        int error = 0;
        uint32_t ch = this->next_chr(&error, use_buf);
        if (error) {{
            token = Token::_ERR;
            return "";
        }}
        int chlen = push_utf8(this->buf, ch);

        switch (state) {{
"#,
        trap
    )?;
    for (i, acc) in lexer.get_states().iter().enumerate() {
        write_line!(3, writer, "case {}:\r\n", i);
        if i == trap {
            write_line!(5, writer, "{{\r\n");
            write_line!(6, writer, "std::string s(found_pos, '\\0');\r\n");
            write_line!(6, writer, "this->buf.read(&s[0], found_pos);\r\n");
            write_line!(6, writer, "token = found;\r\n");
            write_line!(6, writer, "return s;\r\n");
            write_line!(5, writer, "}}\r\n");
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
                if let Some(acc) = acc {
                    write_line!(6, writer, "found_pos = pos;\r\n");
                    write_line!(6, writer, "found = Token::{};\r\n", acc);
                    write_line!(6, writer, "state = {};\r\n", result);
                    write_line!(6, writer, "break;\r\n");
                } else {
                    write_line!(6, writer, "state = {};\r\n", result);
                    write_line!(6, writer, "break;\r\n");
                }
            }
            write_line!(4, writer, "}}\r\n");
        }
        write_line!(4, writer, "break;\r\n");
    }
    write!(
        writer,
        r#"        }}

        if (ch == 0)
        {{
            if (found == Token::_TRAP)
            {{
                token = Token::_EOF;
                return "";
            }}

            std::string s(found_pos, '\0');
            this->buf.read(&s[0], found_pos);
            token = found;
            return s;
        }}

        pos += chlen;
    }}
}}"#
    )?;
    Ok(())
}
