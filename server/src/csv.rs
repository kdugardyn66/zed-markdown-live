//! CSV/TSV → HTML table (RFC 4180 quoting, delimiter auto-detection).
#![allow(dead_code)]

use crate::markdown::escape_html;

#[derive(Clone, Debug)]
pub struct CsvOptions {
    /// `None` = auto-detect among `,` `;` tab `|`.
    pub delimiter: Option<char>,
    pub header: bool,
    pub max_rows: usize,
    /// Optional caption shown above the table.
    pub caption: Option<String>,
}

impl Default for CsvOptions {
    fn default() -> Self {
        CsvOptions { delimiter: None, header: true, max_rows: 5000, caption: None }
    }
}

impl CsvOptions {
    /// Applies `key=value` options from a code-fence info string or an `@import` line.
    pub fn apply(&mut self, attrs: &[(String, String)]) {
        for (k, v) in attrs {
            match k.as_str() {
                "header" | "headers" => self.header = !matches!(v.as_str(), "false" | "0" | "no"),
                "delimiter" | "sep" | "separator" => {
                    self.delimiter = match v.as_str() {
                        "\\t" | "tab" | "\t" => Some('\t'),
                        s => s.chars().next(),
                    }
                }
                "caption" | "title" => self.caption = Some(v.clone()),
                "maxRows" | "max_rows" | "rows" => {
                    if let Ok(n) = v.parse() {
                        self.max_rows = n;
                    }
                }
                _ => {}
            }
        }
    }
}

fn detect_delimiter(text: &str) -> char {
    let sample: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).take(10).collect();
    let mut best = (',', 0usize, usize::MAX);
    for d in [',', ';', '\t', '|'] {
        let counts: Vec<usize> = sample.iter().map(|l| parse_line_count(l, d)).collect();
        if counts.is_empty() {
            continue;
        }
        let min = *counts.iter().min().unwrap();
        let max = *counts.iter().max().unwrap();
        // Prefer delimiters that give a consistent, larger number of columns.
        if min > 1 && (min > best.1 || (min == best.1 && max - min < best.2)) {
            best = (d, min, max - min);
        }
    }
    best.0
}

fn parse_line_count(line: &str, d: char) -> usize {
    let mut n = 1;
    let mut q = false;
    for c in line.chars() {
        if c == '"' {
            q = !q;
        } else if c == d && !q {
            n += 1;
        }
    }
    n
}

/// Parses CSV text into rows of cells.
pub fn parse(text: &str, delimiter: char) -> Vec<Vec<String>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    let mut any = false;
    while let Some(c) = chars.next() {
        any = true;
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    cell.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                cell.push(c);
            }
        } else if c == '"' && cell.is_empty() {
            quoted = true;
        } else if c == delimiter {
            row.push(std::mem::take(&mut cell));
        } else if c == '\r' {
        } else if c == '\n' {
            row.push(std::mem::take(&mut cell));
            if !(row.len() == 1 && row[0].is_empty()) {
                rows.push(std::mem::take(&mut row));
            } else {
                row.clear();
            }
            any = false;
        } else {
            cell.push(c);
        }
    }
    if any {
        row.push(cell);
        if !(row.len() == 1 && row[0].is_empty()) {
            rows.push(row);
        }
    }
    rows
}

fn is_number(s: &str) -> bool {
    let t = s.trim().trim_start_matches(['+', '-', '€', '$']).trim_end_matches(['%', '€']);
    !t.is_empty() && t.chars().all(|c| c.is_ascii_digit() || ".,  '_eE+-".contains(c)) && t.chars().any(|c| c.is_ascii_digit())
}

/// Renders CSV text as an HTML table.
pub fn to_html(text: &str, opts: &CsvOptions) -> String {
    let d = opts.delimiter.unwrap_or_else(|| detect_delimiter(text));
    let rows = parse(text, d);
    if rows.is_empty() {
        return "<p class=\"ml-note\">(empty CSV)</p>".into();
    }
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    // Numeric columns are right-aligned.
    let body_start = if opts.header { 1 } else { 0 };
    let numeric: Vec<bool> = (0..cols)
        .map(|c| {
            let vals: Vec<&String> = rows.iter().skip(body_start).filter_map(|r| r.get(c)).filter(|v| !v.trim().is_empty()).collect();
            !vals.is_empty() && vals.iter().all(|v| is_number(v))
        })
        .collect();
    let mut h = String::from("<div class=\"ml-csv\"><table>");
    if let Some(c) = &opts.caption {
        h.push_str(&format!("<caption>{}</caption>", escape_html(c)));
    }
    let cell = |tag: &str, v: &str, c: usize| {
        let cls = if numeric.get(c).copied().unwrap_or(false) { " class=\"num\"" } else { "" };
        format!("<{tag}{cls}>{}</{tag}>", escape_html(v))
    };
    if opts.header {
        h.push_str("<thead><tr>");
        for c in 0..cols {
            h.push_str(&cell("th", rows[0].get(c).map(|s| s.as_str()).unwrap_or(""), c));
        }
        h.push_str("</tr></thead>");
    }
    h.push_str("<tbody>");
    let body: Vec<&Vec<String>> = rows.iter().skip(body_start).collect();
    for r in body.iter().take(opts.max_rows) {
        h.push_str("<tr>");
        for c in 0..cols {
            h.push_str(&cell("td", r.get(c).map(|s| s.as_str()).unwrap_or(""), c));
        }
        h.push_str("</tr>");
    }
    h.push_str("</tbody></table>");
    if body.len() > opts.max_rows {
        h.push_str(&format!("<p class=\"ml-note\">… {} more rows not shown (csvMaxRows = {})</p>", body.len() - opts.max_rows, opts.max_rows));
    }
    h.push_str("</div>");
    h
}

/// A Chart.js config from CSV: first column = labels, numeric columns = datasets.
pub fn to_chart_json(text: &str, opts: &CsvOptions, chart_type: &str, title: Option<&str>) -> String {
    let d = opts.delimiter.unwrap_or_else(|| detect_delimiter(text));
    let rows = parse(text, d);
    let num = |s: &str| -> Option<f64> {
        let t = s.trim().trim_end_matches('%');
        let t = if t.contains(',') && !t.contains('.') { t.replace(',', ".") } else { t.replace(',', "") };
        t.parse::<f64>().ok()
    };
    let (head, body): (Vec<String>, &[Vec<String>]) = if opts.header && !rows.is_empty() { (rows[0].clone(), &rows[1..]) } else { (vec![], &rows[..]) };
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let labels: Vec<String> = body.iter().map(|r| r.first().cloned().unwrap_or_default()).collect();
    let mut datasets = Vec::new();
    for c in 1..cols {
        let vals: Vec<Option<f64>> = body.iter().map(|r| r.get(c).and_then(|v| num(v))).collect();
        if vals.iter().all(|v| v.is_none()) {
            continue;
        }
        let label = head.get(c).cloned().unwrap_or_else(|| format!("Series {c}"));
        datasets.push(serde_json::json!({"label": label, "data": vals}));
    }
    let mut cfg = serde_json::json!({
        "type": chart_type,
        "data": {"labels": labels, "datasets": datasets},
        "options": {"responsive": true, "plugins": {"legend": {"display": true}}}
    });
    if let Some(t) = title {
        cfg["options"]["plugins"]["title"] = serde_json::json!({"display": true, "text": t});
    }
    cfg.to_string()
}

/// The table as a GitHub pipe table (used by document exports).
pub fn to_markdown(text: &str, opts: &CsvOptions) -> String {
    let d = opts.delimiter.unwrap_or_else(|| detect_delimiter(text));
    let rows = parse(text, d);
    if rows.is_empty() {
        return String::new();
    }
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let esc = |s: &str| s.replace('|', "\\|").replace('\n', " ");
    let line = |r: &Vec<String>| format!("| {} |", (0..cols).map(|c| esc(r.get(c).map(|s| s.as_str()).unwrap_or(""))).collect::<Vec<_>>().join(" | "));
    let mut out = Vec::new();
    let (head, body) = if opts.header { (rows[0].clone(), &rows[1..]) } else { ((1..=cols).map(|i| format!("Column {i}")).collect(), &rows[..]) };
    out.push(line(&head));
    out.push(format!("|{}|", vec!["---"; cols].join("|")));
    for r in body.iter().take(opts.max_rows) {
        out.push(line(r));
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quoting_and_detection() {
        let t = "name;value;note\n\"Smith; John\";12,5;\"he said \"\"hi\"\"\"\nDoe;3;\n";
        assert_eq!(detect_delimiter(t), ';');
        let rows = parse(t, ';');
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1][0], "Smith; John");
        assert_eq!(rows[1][2], "he said \"hi\"");
        let h = to_html(t, &CsvOptions::default());
        assert!(h.contains("<th>name</th>"));
        assert!(h.contains("<td class=\"num\">12,5</td>"));
        assert!(h.contains("Smith; John"));
    }
    #[test]
    fn multiline_cell_and_no_header() {
        let t = "a,\"b\nc\"\n1,2";
        let rows = parse(t, ',');
        assert_eq!(rows, vec![vec!["a".to_string(), "b\nc".into()], vec!["1".into(), "2".into()]]);
        let mut o = CsvOptions::default();
        o.apply(&[("header".into(), "false".into())]);
        assert!(!to_html(t, &o).contains("<th>"));
        assert!(to_markdown("x,y\n1,2", &CsvOptions::default()).starts_with("| x | y |\n|---|---|"));
    }
}
