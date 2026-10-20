//! RFC 4180 CSV reading and writing.

/// Parse CSV text into rows of fields. Handles quoted fields with embedded
/// delimiters, quotes and newlines; CRLF or LF line endings; a UTF-8 BOM.
/// The delimiter is detected from the first line (comma, semicolon or tab).
pub fn parse(text: &str) -> Vec<Vec<String>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let delim = detect_delimiter(text);
    let mut rows = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();
    let mut field_started = false;

    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        match c {
            '"' if !field_started || field.is_empty() => {
                in_quotes = true;
                field_started = true;
            }
            c if c == delim => {
                row.push(std::mem::take(&mut field));
                field_started = false;
            }
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
                field_started = false;
            }
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
                field_started = false;
            }
            c => {
                field.push(c);
                field_started = true;
            }
        }
    }
    if field_started || !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    // Drop blank lines.
    rows.retain(|r| !(r.len() == 1 && r[0].trim().is_empty()));
    rows
}

fn detect_delimiter(text: &str) -> char {
    let first = text.lines().next().unwrap_or("");
    let count = |d: char| {
        let mut n = 0;
        let mut q = false;
        for c in first.chars() {
            if c == '"' {
                q = !q;
            } else if c == d && !q {
                n += 1;
            }
        }
        n
    };
    let (c, s, t) = (count(','), count(';'), count('\t'));
    if t > c && t > s {
        '\t'
    } else if s > c {
        ';'
    } else {
        ','
    }
}

/// Builds CSV text. Fields are quoted only when needed. Cells that a
/// spreadsheet would treat as a formula are prefixed with a quote to prevent
/// CSV injection.
pub struct Writer {
    out: String,
}

impl Default for Writer {
    fn default() -> Self {
        Self::new()
    }
}

impl Writer {
    pub fn new() -> Writer {
        // BOM so Excel opens UTF-8 correctly.
        Writer { out: String::from("\u{feff}") }
    }

    pub fn row<S: AsRef<str>>(&mut self, fields: &[S]) {
        for (i, f) in fields.iter().enumerate() {
            if i > 0 {
                self.out.push(',');
            }
            let f = f.as_ref();
            let guarded;
            let f = if f.starts_with(['=', '+', '-', '@', '\t', '\r']) && f.parse::<f64>().is_err() {
                guarded = format!("'{f}");
                guarded.as_str()
            } else {
                f
            };
            if f.contains([',', '"', '\n', '\r']) || f.starts_with(' ') || f.ends_with(' ') {
                self.out.push('"');
                self.out.push_str(&f.replace('"', "\"\""));
                self.out.push('"');
            } else {
                self.out.push_str(f);
            }
        }
        self.out.push_str("\r\n");
    }

    pub fn finish(self) -> String {
        self.out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quotes_newlines_and_bom() {
        let t = "\u{feff}a,b,c\r\n\"x, y\",\"he said \"\"hi\"\"\",\"multi\nline\"\n\n1,,3";
        let rows = parse(t);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0], vec!["a", "b", "c"]);
        assert_eq!(rows[1], vec!["x, y", "he said \"hi\"", "multi\nline"]);
        assert_eq!(rows[2], vec!["1", "", "3"]);
    }

    #[test]
    fn semicolons_and_trailing_empty() {
        let rows = parse("a;b;c\n1;2;\n");
        assert_eq!(rows, vec![vec!["a", "b", "c"], vec!["1", "2", ""]]);
        let rows = parse("a\tb\n1\t2");
        assert_eq!(rows[1], vec!["1", "2"]);
    }

    #[test]
    fn writer_roundtrip_and_injection_guard() {
        let mut w = Writer::new();
        w.row(&["plain", "with,comma", "quote\"d", "=SUM(A1)", "-12.5", " pad"]);
        let s = w.finish();
        let rows = parse(&s);
        assert_eq!(rows[0], vec!["plain", "with,comma", "quote\"d", "'=SUM(A1)", "-12.5", " pad"]);
    }
}
