//! Notification text for the device: markup parsed into line breaks and
//! emphasis marks. Listening for notifications is per platform
//! (platform::spawn_notification_listener).

// Windows has no notification listener yet (platform/windows).
#![cfg_attr(windows, allow(dead_code))]

/// Start and end of emphasised text (`<b>`, `<i>`, `<u>`) in a body for the
/// device, which draws it in the accent colour (DESIGN.md Section 11.10).
pub const EMPH_ON: char = '\u{2}';
pub const EMPH_OFF: char = '\u{3}';

/// A notification body as the device shows it, from the markup the
/// notification spec allows: line breaks kept (`<br>`, paragraphs, newlines),
/// emphasis marked with EMPH_ON/EMPH_OFF, links as their text, images as their
/// alt text, entities decoded, other tags dropped. A `<` that doesn't start a
/// tag is text, for apps that send plain text.
pub fn parse_markup(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut depth = 0u32; // nested emphasis
    let mut rest = input;
    while let Some(c) = rest.chars().next() {
        if c == '<' {
            if let Some((tag, after)) = split_tag(rest) {
                let name: String = tag
                    .trim_start_matches('/')
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric())
                    .collect::<String>()
                    .to_ascii_lowercase();
                let closing = tag.starts_with('/');
                match name.as_str() {
                    "b" | "strong" | "i" | "em" | "u" => {
                        if closing {
                            if depth > 0 {
                                depth -= 1;
                                if depth == 0 {
                                    out.push(EMPH_OFF);
                                }
                            }
                        } else {
                            if depth == 0 {
                                out.push(EMPH_ON);
                            }
                            depth += 1;
                        }
                    }
                    "br" => out.push('\n'),
                    "p" | "div" | "li" | "tr" if closing => out.push('\n'),
                    "img" => {
                        if let Some(alt) = attr(tag, "alt") {
                            out.push_str(&decode_entities(&alt));
                        }
                    }
                    _ => {}
                }
                rest = after;
                continue;
            }
        }
        if c == '&' {
            if let Some(end) = rest[..rest.len().min(12)].find(';') {
                let decoded = decode_entities(&rest[..=end]);
                if decoded != rest[..=end] {
                    out.push_str(&decoded);
                    rest = &rest[end + 1..];
                    continue;
                }
            }
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    if depth > 0 {
        out.push(EMPH_OFF);
    }
    tidy_lines(&out)
}

/// One line, no markers: for the title, the app name and the log.
pub fn flatten(text: &str) -> String {
    text.replace([EMPH_ON, EMPH_OFF], "").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `<tag ...>` at the start of `s`: the tag's inside and what follows. Only
/// something that looks like a tag counts.
fn split_tag(s: &str) -> Option<(&str, &str)> {
    let inner = &s[1..];
    let first = inner.chars().next()?;
    if !(first.is_ascii_alphabetic() || first == '/') {
        return None;
    }
    let end = inner.find('>')?;
    Some((&inner[..end], &inner[end + 1..]))
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let at = tag.to_ascii_lowercase().find(&format!("{name}="))? + name.len() + 1;
    let value = &tag[at..];
    let quote = value.chars().next().filter(|q| *q == '"' || *q == '\'')?;
    let value = &value[1..];
    Some(value[..value.find(quote)?].to_string())
}

fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(end) = rest.find(';').filter(|e| *e <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let name = &rest[1..end];
        let ch = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ => name
                .strip_prefix("#x")
                .or_else(|| name.strip_prefix("#X"))
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| name.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Spaces collapsed within lines, blank lines dropped, ends trimmed.
fn tidy_lines(text: &str) -> String {
    text.split('\n')
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.replace([EMPH_ON, EMPH_OFF], "").is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markup_becomes_device_text() {
        assert_eq!(
            parse_markup("Meeting moved to <b>3 PM</b><br/>Bring the <i>slides</i> &amp; notes"),
            "Meeting moved to \u{2}3 PM\u{3}\nBring the \u{2}slides\u{3} & notes"
        );
        assert_eq!(parse_markup("<a href=\"https://x\">Open</a> <img src=\"a\" alt=\"[pic]\"/>"), "Open [pic]");
        assert_eq!(parse_markup("<p>one</p><p>two</p>\n\n  three  "), "one\ntwo\nthree");
        // Plain text that only looks a bit like markup.
        assert_eq!(parse_markup("a < b && c > d; 5 <3"), "a < b && c > d; 5 <3");
        assert_eq!(parse_markup("&#x1F600; &#233; &bogus;"), "\u{1F600} é &bogus;");
        // Unclosed emphasis is closed; nested emphasis is one run.
        assert_eq!(parse_markup("<b>x <i>y</i> z"), "\u{2}x y z\u{3}");
        assert_eq!(flatten("\u{2}Hi\u{3}\nthere"), "Hi there");
    }
}
