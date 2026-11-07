//! Fills a text template and breaks it into lines that fit
//! a text box.

/// Splits on a literal separator like [`str::split`], except that trailing empty pieces are
/// dropped, unless the separator does not occur at all.
fn split_dropping_trailing_empty<'a>(s: &'a str, separator: &str) -> Vec<&'a str> {
    let mut parts: Vec<&str> = s.split(separator).collect();
    if parts.len() > 1 {
        while parts.last().is_some_and(|p| p.is_empty()) {
            parts.pop();
        }
    }
    parts
}

/// Control codes of one game's text: line break, extra line (scrolls the box) and new paragraph (new box).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Breaks<'a> {
    pub(crate) newline: &'a str,
    pub(crate) extraline: &'a str,
    pub(crate) newpara: &'a str,
}

/// Replaces each `(target, value)` in `text`, then rewraps every paragraph: a line ends when the next word would
/// pass `max_line_length` (measured with `length`, which should count encoded bytes), or after a word ending in
/// `.`, `?`, `!` or `,` once the line has half that length (at least 10). The first line break of a paragraph is
/// `newline`, later ones `extraline`.
pub(crate) fn format_with_replacements(
    text: &str,
    replacements: &[(&str, &str)],
    breaks: Breaks<'_>,
    max_line_length: usize,
    length: impl Fn(&str) -> usize,
) -> String {
    let Breaks {
        newline,
        extraline,
        newpara,
    } = breaks;
    let (text, ends_with_para) = match text.strip_suffix(newpara) {
        Some(stripped) => (stripped, true),
        None => (text, false),
    };
    let mut text = text.replace(newline, " ").replace(extraline, " ");
    // Two stages, so that one replacement's value is never replaced by another.
    for (i, (target, _)) in replacements.iter().enumerate() {
        text = text.replace(target, &format!("<tmpreplace{}>", i + 1));
    }
    for (i, (_, value)) in replacements.iter().enumerate() {
        text = text.replace(&format!("<tmpreplace{}>", i + 1), value);
    }

    let sentence_line_size = 10.max(max_line_length / 2);
    let mut result = String::new();
    for (para, paragraph) in split_dropping_trailing_empty(&text, newpara).into_iter().enumerate() {
        let mut full = String::new();
        let mut line = String::new();
        let mut words_in_line = 0;
        let mut line_length = 0;
        let mut lines_written = 0;
        let mut last_char = None;
        for word in split_dropping_trailing_empty(paragraph, " ") {
            let mut required = length(word);
            if words_in_line > 0 {
                required += 1;
            }
            let sentence_end = matches!(last_char, Some('.' | '?' | '!' | ','));
            if line_length + required > max_line_length || (line_length >= sentence_line_size && sentence_end) {
                if words_in_line > 0 {
                    push_line(&mut full, &line, lines_written, breaks);
                    lines_written += 1;
                    line.clear();
                }
                line.push_str(word);
                words_in_line = 1;
                line_length = length(word);
            } else {
                if words_in_line > 0 {
                    line.push(' ');
                }
                line.push_str(word);
                words_in_line += 1;
                line_length += required;
            }
            last_char = word.chars().last();
        }
        if words_in_line > 0 {
            push_line(&mut full, &line, lines_written, breaks);
        }
        if para > 0 {
            result.push_str(newpara);
        }
        result.push_str(&full);
    }
    if ends_with_para {
        result.push_str(newpara);
    }
    result
}

fn push_line(full: &mut String, line: &str, lines_written: usize, breaks: Breaks<'_>) {
    match lines_written {
        0 => {}
        1 => full.push_str(breaks.newline),
        _ => full.push_str(breaks.extraline),
    }
    full.push_str(line);
}

/// The line break of Gen 3 and Gen 4 text.
const NEWLINE: &str = "\\n";

/// The words of `description` (lines broken by `\\n`) on lines of at most `line_size`, by `size` (bytes in the
/// game's encoding); "Sp. Atk" and "Sp. Def" stay on one line.
pub(crate) fn reflow(description: &str, line_size: usize, size: impl Fn(&str) -> usize) -> String {
    let joined = description
        .replace(&format!("-{NEWLINE}"), "")
        .replace(NEWLINE, " ")
        .replace("Sp. Atk", "Sp__Atk")
        .replace("Sp. Def", "Sp__Def")
        .replace("SP. ATK", "SP__ATK")
        .replace("SP. DEF", "SP__DEF");
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    let mut words_in_line = 0;
    let mut chars_in_line = 0;
    // Empty words at the end are dropped.
    let mut words: Vec<&str> = joined.split(' ').collect();
    if joined.contains(' ') {
        while words.last() == Some(&"") {
            words.pop();
        }
    }
    for word in words {
        let word = word.replace("SP__", "SP. ").replace("Sp__", "Sp. ");
        let needed = size(&word) + usize::from(words_in_line > 0);
        if chars_in_line + needed <= line_size {
            if words_in_line > 0 {
                line.push(' ');
            }
            line.push_str(&word);
            words_in_line += 1;
            chars_in_line += needed;
        } else {
            if words_in_line > 0 {
                lines.push(std::mem::take(&mut line));
            }
            chars_in_line = size(&word);
            line = word;
            words_in_line = 1;
        }
    }
    if words_in_line > 0 {
        lines.push(line);
    }
    lines.join(NEWLINE)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GEN3: Breaks<'static> = Breaks {
        newline: "\\n",
        extraline: "\\l",
        newpara: "\\p",
    };

    fn format(text: &str, replacements: &[(&str, &str)], max: usize) -> String {
        format_with_replacements(text, replacements, GEN3, max, str::len)
    }

    #[test]
    fn split_drops_trailing_empty_pieces() {
        assert_eq!(split_dropping_trailing_empty("a b  ", " "), ["a", "b"]);
        assert_eq!(split_dropping_trailing_empty(" a", " "), ["", "a"]);
        assert_eq!(split_dropping_trailing_empty("", " "), [""]);
        assert_eq!(split_dropping_trailing_empty("  ", " "), Vec::<&str>::new());
    }

    #[test]
    fn wraps_lines_and_keeps_paragraphs() {
        // Reference output, with a plain length.
        assert_eq!(
            format(
                "I see! [s] is your choice.\\pSo, you want [s]?",
                &[("[s]", "BULBASAUR")],
                20
            ),
            "I see! BULBASAUR is\\nyour choice.\\pSo, you want\\nBULBASAUR?"
        );
        assert_eq!(
            format("one two three four five six seven eight nine ten", &[], 10),
            "one two\\nthree four\\lfive six\\lseven\\leight nine\\lten"
        );
    }

    #[test]
    fn existing_breaks_are_redone_and_a_final_paragraph_kept() {
        assert_eq!(format("a\\nb\\lc\\p", &[], 36), "a b c\\p");
    }

    #[test]
    fn replacements_do_not_replace_each_other() {
        assert_eq!(format("[a] [b]", &[("[a]", "[b]"), ("[b]", "X")], 36), "[b] X");
    }

    #[test]
    fn a_long_line_breaks_after_a_sentence() {
        // Past half the line, a word ending a sentence ends the line.
        assert_eq!(
            format("Hello there friend. How are you?", &[], 36),
            "Hello there friend.\\nHow are you?"
        );
    }

    #[test]
    fn descriptions_reflow_keeping_stat_names_together() {
        let size = str::len;
        assert_eq!(
            reflow("A powerful attack\\nthat may lower Sp. Def.", 18, size),
            "A powerful attack\\nthat may lower\\nSp. Def."
        );
        assert_eq!(reflow("Hyper-\\nbeam", 18, size), "Hyperbeam");
        assert_eq!(reflow("Averyveryverylongword x", 5, size), "Averyveryverylongword\\nx");
    }
}
