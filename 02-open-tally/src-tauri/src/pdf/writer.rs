use std::fmt::Write as FmtWrite;

/// A lightweight, deterministic, pure-Rust PDF 1.4 generator.
/// Generates self-contained A4 documents without external dependencies or headless browsers.
pub struct PdfWriter {
    stream: String,
}

impl PdfWriter {
    pub const PAGE_WIDTH: f64 = 595.28;
    pub const PAGE_HEIGHT: f64 = 841.89;

    pub fn new() -> Self {
        Self {
            stream: String::with_capacity(8192),
        }
    }

    /// Escapes special characters in PDF literal text strings.
    fn escape_pdf_string(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '\\' => out.push_str("\\\\"),
                '(' => out.push_str("\\("),
                ')' => out.push_str("\\)"),
                '\r' => out.push_str("\\r"),
                '\n' => out.push_str("\\n"),
                '₹' => out.push_str("INR "),
                c if c.is_ascii() => out.push(c),
                _ => out.push('?'),
            }
        }
        out
    }

    /// Draws a stroked rectangle.
    pub fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, line_width: f64) {
        let _ = writeln!(
            self.stream,
            "q {:.2} w {:.2} {:.2} {:.2} {:.2} re s Q",
            line_width, x, y, w, h
        );
    }

    /// Draws a filled and stroked rectangle (with optional background shading).
    pub fn filled_rect(&mut self, x: f64, y: f64, w: f64, h: f64, fill_gray: f64, line_width: f64) {
        let _ = writeln!(
            self.stream,
            "q {:.2} g {:.2} w {:.2} {:.2} {:.2} {:.2} re B Q",
            fill_gray, line_width, x, y, w, h
        );
    }

    /// Draws a straight line between two points.
    pub fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, line_width: f64) {
        let _ = writeln!(
            self.stream,
            "q {:.2} w {:.2} {:.2} m {:.2} {:.2} l s Q",
            line_width, x1, y1, x2, y2
        );
    }

    /// Draws text at (x, y). font_id is "/F1" (regular) or "/F2" (bold).
    pub fn text(&mut self, text: &str, x: f64, y: f64, font_id: &str, size: f64) {
        let escaped = Self::escape_pdf_string(text);
        let _ = writeln!(
            self.stream,
            "BT {} {:.2} Tf {:.2} {:.2} Td ({}) Tj ET",
            font_id, size, x, y, escaped
        );
    }

    /// Draws right-aligned text ending at right_x.
    /// Uses approximate Helvetica character metrics.
    pub fn text_right(&mut self, text: &str, right_x: f64, y: f64, font_id: &str, size: f64) {
        let approx_width = Self::estimate_text_width(text, size, font_id == "/F2");
        let x = right_x - approx_width;
        self.text(text, x.max(0.0), y, font_id, size);
    }

    /// Draws center-aligned text centered around center_x.
    pub fn text_center(&mut self, text: &str, center_x: f64, y: f64, font_id: &str, size: f64) {
        let approx_width = Self::estimate_text_width(text, size, font_id == "/F2");
        let x = center_x - (approx_width / 2.0);
        self.text(text, x.max(0.0), y, font_id, size);
    }

    /// Simple heuristic width estimator for standard Helvetica.
    fn estimate_text_width(text: &str, size: f64, is_bold: bool) -> f64 {
        let scale = size / 1000.0;
        let bold_factor = if is_bold { 1.08 } else { 1.0 };
        let mut total = 0.0;
        for c in text.chars() {
            let w = match c {
                ' ' => 278.0,
                'i' | 'l' | '.' | ',' | ':' | ';' | '!' => 222.0,
                'f' | 't' | 'j' | 'I' => 278.0,
                'r' => 333.0,
                's' | 'c' | 'z' => 444.0,
                'a' | 'b' | 'd' | 'e' | 'g' | 'h' | 'k' | 'n' | 'o' | 'p' | 'q' | 'u' | 'v'
                | 'x' | 'y' => 556.0,
                'w' => 722.0,
                'm' => 833.0,
                'A' | 'B' | 'C' | 'D' | 'E' | 'F' | 'G' | 'H' | 'J' | 'K' | 'L' | 'N' | 'O'
                | 'P' | 'Q' | 'R' | 'S' | 'T' | 'U' | 'V' | 'X' | 'Y' | 'Z' => 667.0,
                'M' => 778.0,
                'W' => 944.0,
                '0'..='9' => 556.0,
                '-' | '/' => 333.0,
                _ => 500.0,
            };
            total += w * scale * bold_factor;
        }
        total
    }

    /// Assembles the complete PDF 1.4 document into a binary byte vector.
    pub fn build(self) -> Vec<u8> {
        let mut doc = Vec::with_capacity(self.stream.len() + 1024);
        let header = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n";
        doc.extend_from_slice(header);

        let mut offsets: Vec<usize> = Vec::new();

        // Object 1: Catalog
        offsets.push(doc.len());
        doc.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

        // Object 2: Pages
        offsets.push(doc.len());
        doc.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");

        // Object 3: Page
        offsets.push(doc.len());
        doc.extend_from_slice(
            format!(
                "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.2} {:.2}] /Contents 4 0 R /Resources << /Font << /F1 5 0 R /F2 6 0 R >> >> >>\nendobj\n",
                Self::PAGE_WIDTH,
                Self::PAGE_HEIGHT
            )
            .as_bytes(),
        );

        // Object 4: Contents Stream
        offsets.push(doc.len());
        let stream_bytes = self.stream.as_bytes();
        let stream_header = format!("4 0 obj\n<< /Length {} >>\nstream\n", stream_bytes.len());
        doc.extend_from_slice(stream_header.as_bytes());
        doc.extend_from_slice(stream_bytes);
        doc.extend_from_slice(b"\nendstream\nendobj\n");

        // Object 5: Font F1 (Helvetica)
        offsets.push(doc.len());
        doc.extend_from_slice(
            b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n",
        );

        // Object 6: Font F2 (Helvetica-Bold)
        offsets.push(doc.len());
        doc.extend_from_slice(
            b"6 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold >>\nendobj\n",
        );

        // XRef Table
        let xref_offset = doc.len();
        let total_objects = offsets.len() + 1; // +1 for 0 0 obj
        let mut xref = format!("xref\n0 {}\n0000000000 65535 f \n", total_objects);
        for off in offsets {
            let _ = writeln!(xref, "{:010} 00000 n ", off);
        }
        doc.extend_from_slice(xref.as_bytes());

        // Trailer
        let trailer = format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            total_objects, xref_offset
        );
        doc.extend_from_slice(trailer.as_bytes());

        doc
    }
}

impl Default for PdfWriter {
    fn default() -> Self {
        Self::new()
    }
}
