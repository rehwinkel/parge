use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
};

use color_eyre::Result;
use smol_str::SmolStr;

use crate::lexer::Lexer;

macro_rules! write_line {
    ($indent:expr,$writer:expr,$($arg:tt)*) => {
        for _ in 0..$indent {
            write!($writer, "    ")?;
        }
        write!($writer, $($arg)*)?;
    };
}

pub fn gen_lexer<W: Write>(lexer: &Lexer, writer: &mut W) -> Result<()> {
    let tokens: BTreeSet<SmolStr> = lexer
        .get_states()
        .iter()
        .filter(|s| s.is_some())
        .map(|s| s.unwrap().clone())
        .collect();

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
        r#"import java.io.InputStream;
import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.io.UnsupportedEncodingException;

public class Lexer {{

    private final BufferedReader reader;
    private final StringBuffer buf;

    public Lexer(InputStream is) {{
        BufferedReader reader = null;
        try {{
            reader = new BufferedReader(new InputStreamReader(is, "utf-8"));
        }} catch (UnsupportedEncodingException e) {{
        }}
        this.reader = reader;
        this.buf = new StringBuffer();
    }}

    private int toAlphabet(int ch) {{
        switch (ch) {{
"#
    )?;
    for (i, (r0, r1)) in lexer.get_alphabet().iter().enumerate() {
        if r0 == r1 {
            write_line!(3, writer, "case {}:\r\n", r0);
            write_line!(4, writer, "return {};\r\n", i);
        }
    }
    write_line!(2, writer, "}}\r\n");
    write!(writer, "        ")?;
    for (i, (r0, r1)) in lexer.get_alphabet().iter().enumerate() {
        if r0 != r1 {
            write!(writer, "if (ch >= {} && ch <= {}) {{\r\n", r0, r1)?;
            write_line!(3, writer, "return {};\r\n", i);
            write_line!(2, writer, "}} else ");
        }
    }
    write!(writer, "{{\r\n            return -1;\r\n        }}\r\n")?;
    write!(
        writer,
        r#"    }}

    private int read() throws IOException {{
        int ch = this.reader.read();
        return ch;
    }}

    public TextToken next() throws IOException {{
        Token found = Token._TRAP;
        int found_pos = 0;

        int pos = 0;
        int state = 0;
        while (true) {{
            if (state == {}) {{
                String s = this.buf.substring(0, found_pos);
                this.buf.delete(0, found_pos);
                return new TextToken(found, s);
            }}

            int ch;
            if (pos < this.buf.length()) {{
                ch = this.buf.charAt(pos);
            }} else {{
                ch = this.read();
                if (ch != -1) this.buf.appendCodePoint(ch);
            }}
            int ach = this.toAlphabet(ch);

            switch (state) {{
"#,
        trap
    )?;
    for (i, acc) in lexer.get_states().iter().enumerate() {
        if i != trap {
            write_line!(4, writer, "case {}:\r\n", i);
            write_line!(5, writer, "switch (ach) {{\r\n");
            let mut results: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
            for (r0, r1, result) in lexer.get_connections(i) {
                if let Some(result) = results.get_mut(&result) {
                    result.push(
                        lexer
                            .get_alphabet()
                            .iter()
                            .position(|a| a == &(r0, r1))
                            .unwrap(),
                    );
                } else {
                    results.insert(
                        result,
                        vec![lexer
                            .get_alphabet()
                            .iter()
                            .position(|a| a == &(r0, r1))
                            .unwrap()],
                    );
                }
            }
            for (result, ranges) in results {
                if result == trap {
                    write_line!(6, writer, "default:\r\n");
                } else {
                    for alphabet_id in ranges {
                        write_line!(6, writer, "case {}:\r\n", alphabet_id);
                    }
                }
                if let Some(acc) = acc {
                    write_line!(7, writer, "found_pos = pos;\r\n");
                    write_line!(7, writer, "found = Token.{};\r\n", acc);
                    write_line!(7, writer, "state = {};\r\n", result);
                    write_line!(7, writer, "break;\r\n");
                } else {
                    write_line!(7, writer, "state = {};\r\n", result);
                    write_line!(7, writer, "break;\r\n");
                }
            }
            write_line!(5, writer, "}}\r\n");
            write_line!(5, writer, "break;\r\n");
        }
    }
    write!(
        writer,
        r#"            }}

            if (ch == -1)
            {{
                if (found == Token._TRAP)
                {{
                    return new TextToken(Token._EOF, "");
                }}

                String s = this.buf.substring(0, found_pos);
                this.buf.delete(0, found_pos);
                return new TextToken(found, s);
            }}

            pos++;
        }}
    }}

    public static class TextToken {{
        private final Token token;
        private final String text;

        public TextToken(Token token, String text) {{
            this.token = token;
            this.text = text;
        }}

        public Token getToken() {{
            return this.token;
        }}

        public String getText() {{
            return this.text;
        }}
    }}

    public static enum Token {{
        _EOF,
        _ERR,
"#
    )?;

    for token in tokens {
        write!(writer, "        {},\r\n", token)?;
    }
    write!(
        writer,
        r#"        ;
    }}

}}"#
    )?;
    Ok(())
}
