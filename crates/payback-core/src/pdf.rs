//! A small PDF 1.4 writer: text in the standard Helvetica faces (no font
//! embedding, so files stay tiny), filled rectangles and lines. Coordinates
//! passed in are measured from the top-left of the page, in points.

use crate::pdf_metrics::{HELVETICA, HELVETICA_BOLD};
use std::fmt::Write as _;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Font {
    Regular,
    Bold,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb(pub f64, pub f64, pub f64);

impl Rgb {
    /// Parse "#RRGGBB"; falls back to grey.
    pub fn hex(s: &str) -> Rgb {
        let p = |i: usize| u8::from_str_radix(s.get(i..i + 2).unwrap_or("80"), 16).unwrap_or(128) as f64 / 255.0;
        if s.len() == 7 && s.starts_with('#') {
            Rgb(p(1), p(3), p(5))
        } else {
            Rgb(0.5, 0.5, 0.5)
        }
    }
}

/// Map a character to its WinAnsiEncoding byte, if it has one.
pub fn win_ansi(c: char) -> Option<u8> {
    let u = c as u32;
    match u {
        0x20..=0x7E => Some(u as u8),
        0xA0..=0xFF => Some(u as u8),
        _ => Some(match c {
            '€' => 0x80,
            '‚' => 0x82,
            'ƒ' => 0x83,
            '„' => 0x84,
            '…' => 0x85,
            '†' => 0x86,
            '‡' => 0x87,
            'ˆ' => 0x88,
            '‰' => 0x89,
            'Š' => 0x8A,
            '‹' => 0x8B,
            'Œ' => 0x8C,
            'Ž' => 0x8E,
            '‘' => 0x91,
            '’' => 0x92,
            '“' => 0x93,
            '”' => 0x94,
            '•' => 0x95,
            '–' => 0x96,
            '—' => 0x97,
            '˜' => 0x98,
            '™' => 0x99,
            'š' => 0x9A,
            '›' => 0x9B,
            'œ' => 0x9C,
            'ž' => 0x9E,
            'Ÿ' => 0x9F,
            '\t' => b' ',
            _ => return None,
        }),
    }
}

pub fn can_encode(s: &str) -> bool {
    s.chars().all(|c| win_ansi(c).is_some())
}

fn encode(s: &str) -> Vec<u8> {
    s.chars().filter(|c| !c.is_control() || *c == '\t').map(|c| win_ansi(c).unwrap_or(b'?')).collect()
}

/// Width of `s` in points.
pub fn measure(s: &str, size: f64, font: Font) -> f64 {
    let table = match font {
        Font::Regular => &HELVETICA,
        Font::Bold => &HELVETICA_BOLD,
    };
    let units: u32 = encode(s).iter().map(|&b| table[b as usize] as u32).sum();
    units as f64 * size / 1000.0
}

/// Shorten `s` with an ellipsis so it fits in `max` points.
pub fn fit(s: &str, max: f64, size: f64, font: Font) -> String {
    if measure(s, size, font) <= max {
        return s.to_string();
    }
    let ell = measure("…", size, font);
    let mut out = String::new();
    let mut w = 0.0;
    for c in s.chars() {
        let cw = measure(&c.to_string(), size, font);
        if w + cw + ell > max {
            break;
        }
        out.push(c);
        w += cw;
    }
    let trimmed = out.trim_end();
    format!("{trimmed}…")
}

/// Break `s` into lines no wider than `max`, at most `max_lines` (the last is ellipsized).
pub fn wrap(s: &str, max: f64, size: f64, font: Font, max_lines: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in s.split_whitespace() {
        let candidate = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
        if measure(&candidate, size, font) <= max || cur.is_empty() {
            cur = candidate;
        } else {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_string();
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.len() > max_lines && max_lines > 0 {
        let rest = lines[max_lines - 1..].join(" ");
        lines.truncate(max_lines - 1);
        lines.push(rest);
    }
    lines.into_iter().map(|l| fit(&l, max, size, font)).collect()
}

fn num(v: f64) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".into()
    } else {
        s.to_string()
    }
}

fn pdf_string(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() + 2);
    out.push('(');
    for &b in bytes {
        match b {
            b'(' | b')' | b'\\' => {
                out.push('\\');
                out.push(b as char);
            }
            0x20..=0x7E => out.push(b as char),
            _ => {
                let _ = write!(out, "\\{b:03o}");
            }
        }
    }
    out.push(')');
    out
}

pub struct Page {
    height: f64,
    ops: String,
}

impl Page {
    fn y(&self, y: f64) -> f64 {
        self.height - y
    }

    /// Draw text with its baseline at `y` (from the top).
    pub fn text(&mut self, x: f64, y: f64, size: f64, font: Font, color: Rgb, s: &str) {
        let f = match font {
            Font::Regular => "F1",
            Font::Bold => "F2",
        };
        let _ = writeln!(
            self.ops,
            "{} {} {} rg BT /{f} {} Tf {} {} Td {} Tj ET",
            num(color.0),
            num(color.1),
            num(color.2),
            num(size),
            num(x),
            num(self.y(y)),
            pdf_string(&encode(s))
        );
    }

    /// Right-aligned text ending at `x`.
    pub fn text_right(&mut self, x: f64, y: f64, size: f64, font: Font, color: Rgb, s: &str) {
        let w = measure(s, size, font);
        self.text(x - w, y, size, font, color, s);
    }

    /// Filled rectangle; `y` is the top edge.
    pub fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, color: Rgb) {
        let _ = writeln!(
            self.ops,
            "{} {} {} rg {} {} {} {} re f",
            num(color.0),
            num(color.1),
            num(color.2),
            num(x),
            num(self.y(y + h)),
            num(w),
            num(h)
        );
    }

    pub fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, width: f64, color: Rgb) {
        let _ = writeln!(
            self.ops,
            "{} w {} {} {} RG {} {} m {} {} l S",
            num(width),
            num(color.0),
            num(color.1),
            num(color.2),
            num(x1),
            num(self.y(y1)),
            num(x2),
            num(self.y(y2))
        );
    }

    /// A filled circle (approximated with Bézier curves).
    pub fn dot(&mut self, cx: f64, cy: f64, r: f64, color: Rgb) {
        let k = 0.5523 * r;
        let y = self.y(cy);
        let _ = writeln!(
            self.ops,
            "{} {} {} rg {} {} m {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c f",
            num(color.0),
            num(color.1),
            num(color.2),
            num(cx + r),
            num(y),
            num(cx + r),
            num(y + k),
            num(cx + k),
            num(y + r),
            num(cx),
            num(y + r),
            num(cx - k),
            num(y + r),
            num(cx - r),
            num(y + k),
            num(cx - r),
            num(y),
            num(cx - r),
            num(y - k),
            num(cx - k),
            num(y - r),
            num(cx),
            num(y - r),
            num(cx + k),
            num(y - r),
            num(cx + r),
            num(y - k),
            num(cx + r),
            num(y),
        );
    }
}

pub struct Document {
    pub width: f64,
    pub height: f64,
    title: String,
    pages: Vec<Page>,
}

impl Document {
    pub fn new(width: f64, height: f64, title: &str) -> Document {
        Document { width, height, title: title.to_string(), pages: Vec::new() }
    }

    pub fn letter(title: &str) -> Document {
        Document::new(612.0, 792.0, title)
    }

    pub fn a4(title: &str) -> Document {
        Document::new(595.28, 841.89, title)
    }

    pub fn add_page(&mut self) -> usize {
        self.pages.push(Page { height: self.height, ops: String::new() });
        self.pages.len() - 1
    }

    pub fn page(&mut self, i: usize) -> &mut Page {
        &mut self.pages[i]
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    pub fn finish(mut self) -> Vec<u8> {
        if self.pages.is_empty() {
            self.add_page();
        }
        let mut out: Vec<u8> = Vec::new();
        let mut offsets: Vec<usize> = Vec::new();
        out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");

        // Object numbering: 1 catalog, 2 pages, 3 F1, 4 F2, 5 info, then
        // page/content pairs.
        let n_pages = self.pages.len();
        let first_page_obj = 6;
        let obj = |out: &mut Vec<u8>, offsets: &mut Vec<usize>, body: &[u8]| {
            offsets.push(out.len());
            let n = offsets.len();
            out.extend_from_slice(format!("{n} 0 obj\n").as_bytes());
            out.extend_from_slice(body);
            out.extend_from_slice(b"\nendobj\n");
        };

        obj(&mut out, &mut offsets, b"<< /Type /Catalog /Pages 2 0 R >>");
        let kids: Vec<String> = (0..n_pages).map(|i| format!("{} 0 R", first_page_obj + i * 2)).collect();
        obj(
            &mut out,
            &mut offsets,
            format!("<< /Type /Pages /Kids [{}] /Count {n_pages} >>", kids.join(" ")).as_bytes(),
        );
        obj(
            &mut out,
            &mut offsets,
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
        );
        obj(
            &mut out,
            &mut offsets,
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>",
        );
        let mut title_hex = String::from("<FEFF");
        for u in self.title.encode_utf16() {
            let _ = write!(title_hex, "{u:04X}");
        }
        title_hex.push('>');
        obj(
            &mut out,
            &mut offsets,
            format!("<< /Title {title_hex} /Producer (Payback) /Creator (Payback) >>").as_bytes(),
        );

        let (w, h) = (num(self.width), num(self.height));
        for (i, page) in self.pages.iter().enumerate() {
            let content_obj = first_page_obj + i * 2 + 1;
            obj(
                &mut out,
                &mut offsets,
                format!(
                    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w} {h}] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {content_obj} 0 R >>"
                )
                .as_bytes(),
            );
            let mut body = format!("<< /Length {} >>\nstream\n", page.ops.len()).into_bytes();
            body.extend_from_slice(page.ops.as_bytes());
            body.extend_from_slice(b"\nendstream");
            obj(&mut out, &mut offsets, &body);
        }

        let xref_at = out.len();
        let total = offsets.len() + 1;
        out.extend_from_slice(format!("xref\n0 {total}\n0000000000 65535 f \n").as_bytes());
        for off in &offsets {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!("trailer\n<< /Size {total} /Root 1 0 R /Info 5 0 R >>\nstartxref\n{xref_at}\n%%EOF\n").as_bytes(),
        );
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_like_helvetica() {
        // "Hello" in Helvetica 12pt: H722 e556 l222 l222 o556 = 2278 units.
        assert!((measure("Hello", 12.0, Font::Regular) - 27.336).abs() < 0.01);
        assert!(measure("Hello", 12.0, Font::Bold) > measure("Hello", 12.0, Font::Regular));
        let f = fit("A very long project description indeed", 80.0, 10.0, Font::Regular);
        assert!(f.ends_with('…'));
        assert!(measure(&f, 10.0, Font::Regular) <= 80.0);
        let lines = wrap("one two three four five six seven eight nine ten", 60.0, 10.0, Font::Regular, 2);
        assert_eq!(lines.len(), 2);
        assert!(lines[1].ends_with('…'));
    }

    #[test]
    fn encodes_and_writes_valid_structure() {
        assert_eq!(win_ansi('€'), Some(0x80));
        assert_eq!(win_ansi('é'), Some(0xE9));
        assert_eq!(win_ansi('漢'), None);
        assert!(can_encode("Café – “quotes”"));

        let mut d = Document::letter("Test ✓ report");
        let p = d.add_page();
        d.page(p).text(50.0, 50.0, 12.0, Font::Bold, Rgb(0.0, 0.0, 0.0), "Hello (world) \\ é €");
        d.page(p).rect(50.0, 60.0, 100.0, 10.0, Rgb::hex("#2563EB"));
        d.page(p).line(50.0, 80.0, 150.0, 80.0, 0.5, Rgb(0.5, 0.5, 0.5));
        d.page(p).dot(60.0, 100.0, 3.0, Rgb::hex("#DC2626"));
        let bytes = d.finish();
        let s = String::from_utf8_lossy(&bytes);
        assert!(s.starts_with("%PDF-1.4"));
        assert!(s.contains("(Hello \\(world\\) \\\\ \\351 \\200)"));
        assert!(s.trim_end().ends_with("%%EOF"));
        // Every xref offset must point at its "N 0 obj" header (byte offsets).
        let find = |hay: &[u8], pat: &[u8]| hay.windows(pat.len()).rposition(|w| w == pat).unwrap();
        let xref = find(&bytes, b"xref\n");
        let table = std::str::from_utf8(&bytes[xref..]).unwrap();
        for (n, line) in table.lines().skip(3).take_while(|l| l.ends_with(" n ")).enumerate() {
            let off: usize = line[..10].parse().unwrap();
            assert!(bytes[off..].starts_with(format!("{} 0 obj", n + 1).as_bytes()), "object {}", n + 1);
        }
    }
}
