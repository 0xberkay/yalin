//! Stage 2: grammar validation over structural offsets, building the tape.
//!
//! Iterative (explicit stack), so nesting depth is bounded only by
//! [`Limits::max_depth`], never by the native call stack.

use crate::scanner::{BlockScanner, Scanner, chunk_capacity};
use crate::tape::{Node, Tag};
use crate::{ErrorKind, JsonError, Limits, number, strings, utf8};

pub(crate) fn parse(input: &[u8], limits: &Limits) -> Result<Vec<Node>, JsonError> {
    if input.len() > limits.max_input_len.min(u32::MAX as usize) {
        return Err(JsonError::new(ErrorKind::InputTooLarge, 0));
    }
    // RFC 8259 §8.1: the whole text must be UTF-8. Checking it once up front lets
    // stage 2 skip string bytes entirely unless a string has escapes (control
    // bytes inside strings are found by the scanner's masks).
    utf8::validate(input).map_err(|at| JsonError::new(ErrorKind::InvalidUtf8, at))?;
    Parser {
        input,
        scanner: BlockScanner::default(),
        chunk: vec![0; chunk_capacity(input.len())].into_boxed_slice(),
        len: 0,
        pos: 0,
        escaped: 0,
        tape: Vec::with_capacity(tape_capacity(input.len())),
        stack: Vec::with_capacity(16),
        context: Context::Top,
        max_depth: limits.max_depth,
    }
    .run()
}

/// Initial tape capacity. Object-heavy JSON needs one node per 5–8 bytes (benchmark
/// product list: ~7.9, `testdata` product list: ~4.9); denser input grows the tape. Reserving
/// up front avoids repeated grow-and-copy, and pages of an mmap-backed
/// reservation that are never written cost no memory.
fn tape_capacity(input_len: usize) -> usize {
    input_len / 5 + 4
}

/// Grammar position between tokens. Transitions that are always followed by the
/// same next step (`,` then key or value, key then value) are taken directly
/// instead of going through this loop.
#[derive(Clone, Copy)]
enum State {
    /// Top-level value.
    Value,
    /// Just after `{`.
    ObjectFirst,
    /// Just after `[`.
    ArrayFirst,
    /// After a member value: `,` or `}`.
    ObjectNext,
    /// After an element: `,` or `]`.
    ArrayNext,
    /// After the top-level value: only the end of input may follow.
    End,
}

/// Kind of the innermost open container, cached so the hot path never reads the tape.
#[derive(Clone, Copy)]
enum Context {
    Top,
    Object,
    Array,
}

struct Parser<'a> {
    input: &'a [u8],
    scanner: BlockScanner,
    /// Current chunk of structural offsets: `chunk[pos..len]` are still to be read.
    chunk: Box<[u32]>,
    len: usize,
    pos: usize,
    /// Next unread entry of [`BlockScanner::escaped_closes`] for the current chunk.
    escaped: usize,
    tape: Vec<Node>,
    /// Open containers: tape index and the enclosing [`Context`] to restore on close.
    stack: Vec<(usize, Context)>,
    context: Context,
    max_depth: usize,
}

fn err<T>(kind: ErrorKind, offset: usize) -> Result<T, JsonError> {
    Err(JsonError::new(kind, offset))
}

/// Bytes that may legally follow a bare atom.
fn is_delimiter(b: Option<&u8>) -> bool {
    matches!(
        b,
        None | Some(b' ' | b'\t' | b'\n' | b'\r' | b',' | b':' | b'[' | b']' | b'{' | b'}' | b'"')
    )
}

impl Parser<'_> {
    fn run(mut self) -> Result<Vec<Node>, JsonError> {
        let mut state = State::Value;
        loop {
            state = match state {
                State::Value => {
                    let at = self.next()?;
                    self.value(at)?
                }
                State::ObjectFirst => {
                    let at = self.next()?;
                    match self.input[at] {
                        b'}' => self.close(),
                        b'"' => self.member(at)?,
                        _ => return err(ErrorKind::UnexpectedCharacter, at),
                    }
                }
                State::ArrayFirst => {
                    let at = self.next()?;
                    match self.input[at] {
                        b']' => self.close(),
                        _ => self.value(at)?,
                    }
                }
                State::ObjectNext => {
                    let at = self.next()?;
                    match self.input[at] {
                        b',' => {
                            let key = self.next()?;
                            if self.input[key] != b'"' {
                                return err(ErrorKind::UnexpectedCharacter, key);
                            }
                            self.member(key)?
                        }
                        b'}' => self.close(),
                        _ => return err(ErrorKind::UnexpectedCharacter, at),
                    }
                }
                State::ArrayNext => {
                    let at = self.next()?;
                    match self.input[at] {
                        b',' => {
                            let at = self.next()?;
                            self.value(at)?
                        }
                        b']' => self.close(),
                        _ => return err(ErrorKind::UnexpectedCharacter, at),
                    }
                }
                State::End => {
                    return match self.peek()? {
                        Some(at) => err(ErrorKind::TrailingCharacters, at),
                        None => Ok(self.tape),
                    };
                }
            };
        }
    }

    /// Consumes the next structural offset.
    #[inline(always)]
    fn next(&mut self) -> Result<usize, JsonError> {
        if self.pos < self.len {
            let at = self.chunk[self.pos];
            self.pos += 1;
            return Ok(at as usize);
        }
        self.next_slow()
    }

    #[cold]
    #[inline(never)]
    fn next_slow(&mut self) -> Result<usize, JsonError> {
        match self.peek()? {
            Some(at) => {
                self.pos += 1;
                Ok(at)
            }
            None => err(ErrorKind::UnexpectedEof, self.input.len()),
        }
    }

    /// Next structural offset without consuming it; `None` at end of input.
    fn peek(&mut self) -> Result<Option<usize>, JsonError> {
        if self.pos == self.len {
            self.pos = 0;
            self.escaped = 0;
            self.len = self.scanner.fill(self.input, &mut self.chunk)?;
        }
        Ok((self.pos < self.len).then(|| self.chunk[self.pos] as usize))
    }

    fn push(&mut self, tag: Tag, start: usize, end: usize) {
        // Offsets fit: input length was checked against u32::MAX.
        self.tape.push(Node { tag, start: start as u32, end: end as u32 });
    }

    /// Where the grammar goes after a complete value.
    #[inline(always)]
    fn after_value(&self) -> State {
        match self.context {
            Context::Top => State::End,
            Context::Object => State::ObjectNext,
            Context::Array => State::ArrayNext,
        }
    }

    fn value(&mut self, at: usize) -> Result<State, JsonError> {
        match self.input[at] {
            b'{' => return self.open(Tag::Object, at),
            b'[' => return self.open(Tag::Array, at),
            b'"' => self.string(at)?,
            b't' => self.literal(at, b"true", Tag::True)?,
            b'f' => self.literal(at, b"false", Tag::False)?,
            b'n' => self.literal(at, b"null", Tag::Null)?,
            b'-' | b'0'..=b'9' => {
                let end = number::validate(self.input, at)?;
                if !is_delimiter(self.input.get(end)) {
                    return err(ErrorKind::InvalidNumber, end);
                }
                self.push(Tag::Number, at, end);
            }
            _ => return err(ErrorKind::UnexpectedCharacter, at),
        }
        Ok(self.after_value())
    }

    fn open(&mut self, tag: Tag, at: usize) -> Result<State, JsonError> {
        if self.stack.len() >= self.max_depth {
            return err(ErrorKind::DepthLimitExceeded, at);
        }
        self.stack.push((self.tape.len(), self.context));
        self.push(tag, at, 0);
        Ok(if tag == Tag::Object {
            self.context = Context::Object;
            State::ObjectFirst
        } else {
            self.context = Context::Array;
            State::ArrayFirst
        })
    }

    /// Only called inside a container (the grammar states guarantee it).
    fn close(&mut self) -> State {
        if let Some((top, parent)) = self.stack.pop() {
            self.tape[top].end = self.tape.len() as u32;
            self.context = parent;
        }
        self.after_value()
    }

    /// Object member whose key's opening quote is at `at`: key, `:`, value.
    fn member(&mut self, at: usize) -> Result<State, JsonError> {
        self.string(at)?;
        let colon = self.next()?;
        if self.input[colon] != b':' {
            return err(ErrorKind::UnexpectedCharacter, colon);
        }
        let at = self.next()?;
        self.value(at)
    }

    /// String at opening quote `at`; the scanner guarantees the closing quote is next.
    fn string(&mut self, at: usize) -> Result<(), JsonError> {
        let close = self.next()?;
        debug_assert_eq!(self.input[close], b'"');
        // Every closing quote passes through here in order, so the scanner's list of
        // escaped strings is consumed in step with the offsets.
        let has_escape = self.scanner.escaped_closes().get(self.escaped) == Some(&(close as u32));
        if has_escape {
            self.escaped += 1;
            let found = strings::validate_escapes(&self.input[at + 1..close], at + 1)?;
            debug_assert!(found);
        }
        let tag = if has_escape { Tag::EscapedString } else { Tag::String };
        self.push(tag, at + 1, close);
        Ok(())
    }

    fn literal(&mut self, at: usize, word: &[u8], tag: Tag) -> Result<(), JsonError> {
        let end = at + word.len();
        if self.input.get(at..end) != Some(word) || !is_delimiter(self.input.get(end)) {
            return err(ErrorKind::InvalidLiteral, at);
        }
        self.push(tag, at, end);
        Ok(())
    }
}
