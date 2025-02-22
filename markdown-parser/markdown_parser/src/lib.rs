use std::io::Write;

pub fn markdown_to_html(s: &str, heading_offset: usize) -> Result<String, Box<dyn std::error::Error>> {
    let mut parser = MarkdownParser::new(s, heading_offset);
    let mut output = Vec::new();
    loop {
        let (text, markdown_elem) = parser.next_elem();
        let _ = output.write(text)?;
        use MarkdownElem::*;
        match markdown_elem {
            Some(AtxHeadingStart(level)) => {
                let _ = output.write(format!("<h{level}>").as_bytes())?;
            },
            Some(AtxHeadingEnd(level)) => {
                let _ = output.write(format!("</h{level}>\n").as_bytes());
            },
            Some(ListStart) => {
                let _ = output.write(b"<ul>\n<li>")?;
            },
            Some(ListElem) => {
                let _ = output.write(b"</li>\n<li>")?;
            },
            Some(ListEnd) => {
                let _ = output.write(b"</li>\n</ul>\n")?;
            },
            Some(EmphasisStart) => {
                let _ = output.write(b"<em>")?;
            },
            Some(EmphasisEnd) => {
                let _ = output.write(b"</em>")?;
            },
            Some(StrongStart) => {
                let _ = output.write(b"<strong>")?;
            },
            Some(StrongEnd) => {
                let _ = output.write(b"</strong>")?;
            },
            Some(Break) => {
                let _ = output.write(b"\n<br>\n")?;
            },
            None => break,
        }
    }

    Ok(String::from_utf8(output)?)
}

macro_rules! parser_next_chars {
    ($self: expr, $values:pat) => { {
        let offs = $self.buf[$self.idx..].iter().position(|x| {
            matches!(x, $values)
        });

        let Some(offs) = offs else {
            $self.idx = $self.buf.len();
            return None;
        };

        $self.idx + offs
    }}

}

pub struct MarkdownParser<'a>  {
    buf: &'a [u8],
    idx: usize,
    state_stack: Vec<ParserState>,
    heading_offset: usize,
}

impl MarkdownParser<'_> {
    pub fn new(buf: &str, heading_offset: usize) -> MarkdownParser<'_> {
        MarkdownParser {
            buf: buf.as_bytes(),
            idx: 0,
            state_stack: Vec::new(),
            heading_offset,
        }
    }

    pub fn next_elem(&mut self) -> (&'_[u8], Option<MarkdownElem>) {
        let start_idx = self.idx;
        loop {
            if self.idx >= self.buf.len() {
                return (&self.buf[start_idx..], None);
            }

            let elem = match self.state_stack.last() {
                Some(ParserState::Header(level)) => {
                    self.parse_in_header(*level)
                },
                Some(ParserState::List) => {
                    self.parse_in_list()
                },
                Some(ParserState::Emphasis) => {
                    self.parse_in_emphasis()
                },
                Some(ParserState::Strong) => {
                    self.parse_in_strong()
                },
                None => {
                    self.parse_default_state()
                },
            };

            let Some((end_idx, elem)) = elem else {
                continue;
            };

            return (&self.buf[start_idx..end_idx], Some(elem))
        }
    }

    fn parse_default_state(&mut self) -> Option<(usize, MarkdownElem)> {
        let pos = parser_next_chars!(self, b'#' | b'*' | b'-' | b'\n');

        match self.buf[pos] {
            b'#' => {
                if let Some(num_hashes) = self.parse_atx_heading_start(pos) {
                    return Some((self.idx - num_hashes - 1, MarkdownElem::AtxHeadingStart(num_hashes)));
                }
            },
            b'*' => {
                if is_line_start_with_trailing_ws(self.buf, pos) {
                    self.state_stack.push(ParserState::List);
                    self.idx = pos + 1;
                    return Some((self.idx - 1, MarkdownElem::ListStart));
                }

                return self.parse_strong_emphasis(pos);
            },
            b'-' => {
                if is_line_start_with_trailing_ws(self.buf, pos) {
                    self.state_stack.push(ParserState::List);
                    self.idx = pos + 1;
                    return Some((self.idx - 1, MarkdownElem::ListStart));
                }
            },
            b'\n' => {
                if let Some(b'\n') = self.buf.get(pos + 1) {
                    self.idx = pos + 2;
                    return Some((self.idx - 2, MarkdownElem::Break));
                }
            },
            _ => unreachable!(),
        }

        self.idx = pos + 1;
        None
    }

    fn parse_in_header(&mut self, level: usize) -> Option<(usize, MarkdownElem)> {
        let pos = parser_next_chars!(self, b'\n');

        self.idx = pos + 1;
        self.state_stack.pop();
        Some((self.idx, MarkdownElem::AtxHeadingEnd(level)))
    }

    fn parse_in_emphasis(&mut self) -> Option<(usize, MarkdownElem)> {
        let pos = parser_next_chars!(self, b'*');
        self.idx = pos + 1;
        self.state_stack.pop();
        Some((self.idx - 1, MarkdownElem::EmphasisEnd))
    }

    fn parse_in_strong(&mut self) -> Option<(usize, MarkdownElem)> {
        let pos = parser_next_chars!(self, b'*');
        self.idx = pos + 2;

        if let Some(b'*') = self.buf.get(pos + 1) {
            self.state_stack.pop();
            return Some((self.idx - 2, MarkdownElem::StrongEnd));
        }

        None
    }

    fn parse_in_list(&mut self) -> Option<(usize, MarkdownElem)> {
        let pos = parser_next_chars!(self, b'\n' | b'*' | b'-');

        self.idx = pos + 1;

        match self.buf[pos] {
            b'*' => {
                if is_line_start_with_trailing_ws(self.buf, pos) {
                    return Some((self.idx - 1, MarkdownElem::ListElem));
                }

                return self.parse_strong_emphasis(pos);
            },
            b'-' => {
                if is_line_start_with_trailing_ws(self.buf, pos) {
                    return Some((self.idx - 1, MarkdownElem::ListElem));
                }
            },
            b'\n' => {
                if let Some(b'\n') = self.buf.get(pos + 1) {
                    self.idx = pos + 2;
                    self.state_stack.pop();
                    return Some((self.idx, MarkdownElem::ListEnd));
                }
            }
            _ => unreachable!(),
        }

        None
    }

    fn parse_atx_heading_start(&mut self, first_hash: usize) -> Option<usize> {
        if !is_line_start(self.buf, first_hash) { return None };

        let mut num_hashes = self.heading_offset;
        let mut byte;

        let mut it = self.buf[first_hash..].iter().cloned();
        let mut it = CountingIt::new(&mut it);
        loop {
            byte = it.next()?;

            if byte != b'#' || num_hashes == 6 {
                break;
            }
            num_hashes += 1;
        }

        if num_hashes == 0 {
            return None;
        }

        if !byte.is_ascii_whitespace() {
            return None;
        }

        self.state_stack.push(ParserState::Header(num_hashes));
        self.idx += it.count;
        Some(num_hashes)
    }

    fn parse_strong_emphasis(&mut self, pos: usize) -> Option<(usize, MarkdownElem)> {
        let next_char = self.buf.get(pos + 1);


        match next_char {
            Some(b'*') => {
                self.idx = pos + 2;
                self.state_stack.push(ParserState::Strong);
                Some((self.idx - 2, MarkdownElem::StrongStart))
            },
            _ => {
                self.idx = pos + 1;
                self.state_stack.push(ParserState::Emphasis);
                Some((self.idx - 1, MarkdownElem::EmphasisStart))
            },
        }

    }
}

fn has_trailing_whitespace(buf: &[u8], pos: usize) -> bool {
    pos + 1 < buf.len() && buf[pos + 1].is_ascii_whitespace()
}

fn is_line_start(buf: &[u8], pos: usize) -> bool {
    pos == 0 || buf[pos - 1] == b'\n'
}

fn is_line_start_with_trailing_ws(buf: &[u8], pos: usize) -> bool {
    is_line_start(buf, pos) && has_trailing_whitespace(buf, pos)
}

#[derive(Debug)]
pub enum MarkdownElem {
    AtxHeadingStart(usize),
    AtxHeadingEnd(usize),
    ListStart,
    ListElem,
    ListEnd,
    EmphasisStart,
    EmphasisEnd,
    StrongStart,
    StrongEnd,
    Break,
}

enum ParserState {
    Header(usize),
    Emphasis,
    Strong,
    List,
}

struct CountingIt<'a> {
    count: usize,
    inner: &'a mut dyn std::iter::Iterator<Item=u8>,
}

impl CountingIt<'_> {
    fn new(inner: &mut dyn std::iter::Iterator<Item=u8>) -> CountingIt<'_> {
        CountingIt {
            count: 0,
            inner,
        }
    }
}

impl std::iter::Iterator for CountingIt<'_> {
    type Item = u8;
    fn next(&mut self) -> Option<u8> {
        let ret = self.inner.next()?;
        self.count += 1;
        Some(ret)
    }
}
