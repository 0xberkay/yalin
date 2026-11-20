//! Corpus, differential (vs `serde_json`) and edge-case tests.

use std::fs;
use std::path::PathBuf;

use serde_json::Value as Serde;

use crate::tape::Tag;
use crate::*;

fn corpus(dir: &str) -> Vec<(String, Vec<u8>)> {
    // YALIN_TESTDATA overrides the corpus location (e.g. when running on a device).
    let root = std::env::var_os("YALIN_TESTDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata"))
        .join(dir);
    let mut files: Vec<_> = fs::read_dir(&root)
        .unwrap_or_else(|e| panic!("{}: {e}", root.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .map(|p| (p.file_name().unwrap().to_string_lossy().into_owned(), fs::read(&p).unwrap()))
        .collect();
    files.sort();
    files
}

/// Converts a parsed value into `serde_json::Value` by walking the tape.
fn to_serde(v: Value<'_>) -> Serde {
    let node = v.node();
    match node.tag {
        Tag::Null => Serde::Null,
        Tag::True => Serde::Bool(true),
        Tag::False => Serde::Bool(false),
        Tag::Number => serde_json::from_str(v.as_number_str().unwrap()).unwrap(),
        Tag::String | Tag::EscapedString => Serde::String(v.as_str().unwrap().into_owned()),
        Tag::Array => {
            let mut out = Vec::new();
            let mut i = v.idx + 1;
            while i < node.end as usize {
                out.push(to_serde(v.at(i)));
                i = v.tape[i].skip(i);
            }
            Serde::Array(out)
        }
        Tag::Object => {
            let mut out = serde_json::Map::new();
            let mut i = v.idx + 1;
            while i < node.end as usize {
                let key = v.at(i).as_str().unwrap().into_owned();
                out.insert(key, to_serde(v.at(i + 1)));
                i = v.tape[i + 1].skip(i + 1);
            }
            Serde::Object(out)
        }
    }
}

/// Builds a `serde_json::Value` from [`Value::walk`] events.
#[derive(Default)]
struct SerdeBuilder {
    /// Open containers, with the pending key for objects.
    stack: Vec<(Serde, Option<String>)>,
    key: Option<String>,
    done: Option<Serde>,
    lens: Vec<usize>,
}

impl SerdeBuilder {
    fn put(&mut self, v: Serde) {
        match self.stack.last_mut() {
            Some((Serde::Array(a), _)) => a.push(v),
            Some((Serde::Object(o), _)) => drop(o.insert(self.key.take().unwrap(), v)),
            Some(_) => unreachable!(),
            None => self.done = Some(v),
        }
    }
    fn end(&mut self, len: usize) {
        let (v, key) = self.stack.pop().unwrap();
        let actual = match &v {
            Serde::Array(a) => a.len(),
            Serde::Object(o) => o.len(),
            _ => unreachable!(),
        };
        assert_eq!(actual, len);
        self.lens.push(len);
        self.key = key;
        self.put(v);
    }
}

impl Visitor for SerdeBuilder {
    fn null(&mut self) {
        self.put(Serde::Null)
    }
    fn bool(&mut self, value: bool) {
        self.put(Serde::Bool(value))
    }
    fn number(&mut self, text: &str) {
        self.put(serde_json::from_str(text).unwrap())
    }
    fn string(&mut self, value: &str) {
        self.put(Serde::String(value.to_owned()))
    }
    fn key(&mut self, key: &str) {
        self.key = Some(key.to_owned())
    }
    fn begin_array(&mut self) {
        self.stack.push((Serde::Array(Vec::new()), self.key.take()))
    }
    fn end_array(&mut self, len: usize) {
        self.end(len)
    }
    fn begin_object(&mut self) {
        self.stack.push((Serde::Object(Default::default()), self.key.take()))
    }
    fn end_object(&mut self, len: usize) {
        self.end(len)
    }
}

fn walk_to_serde(v: Value<'_>) -> Result<Serde, JsonError> {
    let mut b = SerdeBuilder::default();
    v.walk(&mut b)?;
    assert!(b.stack.is_empty());
    Ok(b.done.unwrap())
}

#[test]
fn walk_rejects_duplicate_keys() {
    let dup = |s: &str| {
        let doc = Document::parse(s.as_bytes()).unwrap();
        walk_to_serde(doc.root()).map(drop).map_err(|e| (e.kind(), e.offset()))
    };
    assert_eq!(dup(r#"{"a":1,"a":2}"#), Err((ErrorKind::DuplicateKey, 7)));
    assert_eq!(dup(r#"{"a":1,"\u0061":2}"#), Err((ErrorKind::DuplicateKey, 7)));
    assert_eq!(dup(r#"[{"a":{"a":1}},{"a":[{"a":0}]}]"#), Ok(()));
    assert_eq!(dup(r#"{"a":{"b":1},"b":2,"c":{"b":1,"c":2}}"#), Ok(()));
    // Past the linear scan: every key distinct, then one repeat of an early key.
    let many: Vec<String> = (0..100).map(|i| format!(r#""k{i}":{i}"#)).collect();
    assert_eq!(dup(&format!("{{{}}}", many.join(","))), Ok(()));
    let repeat = format!(r#"{{{},"k3":0}}"#, many.join(","));
    assert_eq!(dup(&repeat), Err((ErrorKind::DuplicateKey, repeat.len() - 7)));
    let repeat_late = format!(r#"{{{},"k99":0}}"#, many.join(","));
    assert!(dup(&repeat_late).is_err());
}

#[test]
fn walk_reports_lengths_and_subtrees() {
    let doc = Document::parse(br#"{"a":[1,[],{}],"b":{"c":"x\ny"},"d":[]}"#.as_slice()).unwrap();
    let mut b = SerdeBuilder::default();
    doc.root().walk(&mut b).unwrap();
    assert_eq!(b.lens, [0, 0, 3, 1, 0, 3]);
    assert_eq!(walk_to_serde(doc.get("b").unwrap()).unwrap(), serde_json::json!({"c": "x\ny"}));
    assert_eq!(walk_to_serde(doc.get("a[0]").unwrap()).unwrap(), serde_json::json!(1));
}

/// Compares accept/reject and decoded values with serde_json.
/// Returns false when the difference is a documented, intended one.
fn differential(input: &[u8]) -> bool {
    let ours = Document::parse(input);
    let theirs = serde_json::from_slice::<Serde>(input);
    match (ours, theirs) {
        (Ok(doc), Ok(expected)) => {
            assert_eq!(to_serde(doc.root()), expected, "value mismatch: {:?}", String::from_utf8_lossy(input));
            match walk_to_serde(doc.root()) {
                Ok(walked) => assert_eq!(walked, expected, "walk mismatch: {:?}", String::from_utf8_lossy(input)),
                Err(e) => assert_eq!(e.kind(), ErrorKind::DuplicateKey),
            }
            true
        }
        (Err(_), Err(_)) => true,
        // serde_json rejects valid JSON numbers outside f64 range and nests > 128.
        (Ok(_), Err(e)) if e.to_string().contains("out of range") || e.to_string().contains("recursion") => false,
        (ours, theirs) => panic!(
            "accept/reject mismatch on {:?}: yalin={:?} serde={:?}",
            String::from_utf8_lossy(input),
            ours.err(),
            theirs.err()
        ),
    }
}

#[test]
fn valid_corpus_parses_and_matches_serde() {
    let files = corpus("valid");
    assert!(files.len() >= 20);
    for (name, bytes) in files {
        Document::parse(bytes.as_slice()).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(differential(&bytes), "{name}");
    }
}

#[test]
fn invalid_corpus_is_rejected() {
    let files = corpus("invalid");
    assert!(files.len() >= 20);
    for (name, bytes) in files {
        assert!(validate(&bytes).is_err(), "{name} accepted");
        assert!(serde_json::from_slice::<Serde>(&bytes).is_err(), "{name}: corpus entry is valid JSON");
    }
}

/// Deterministic mutation fuzzing of the corpus against serde_json. Catches
/// panics, accept/reject disagreements and decoding differences.
#[test]
fn mutated_corpus_agrees_with_serde() {
    const ALPHABET: &[u8] = b"{}[]:,\"\\ \t\n0123456789-+.eEtrufalsn/bu\x00\x1f\x7f\xc3\xa9\xed\xa0\xff";
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut rand = move |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n as u64) as usize
    };
    let seeds: Vec<_> = corpus("valid").into_iter().chain(corpus("invalid")).map(|(_, b)| b).collect();
    let mut compared = 0;
    for round in 0..40_000 {
        let mut m = seeds[round % seeds.len()].clone();
        for _ in 0..1 + rand(3) {
            let at = if m.is_empty() { 0 } else { rand(m.len()) };
            let byte = ALPHABET[rand(ALPHABET.len())];
            match rand(4) {
                0 if !m.is_empty() => m[at] = byte,
                1 if !m.is_empty() => drop(m.remove(at)),
                2 => m.insert(at, byte),
                _ => m.truncate(at),
            }
        }
        compared += differential(&m) as usize;
    }
    assert!(compared > 39_000, "too many skipped comparisons: {compared}");
}

fn err_kind(input: &[u8]) -> ErrorKind {
    validate(input).unwrap_err().kind()
}

#[test]
fn edge_cases_from_spec() {
    assert_eq!(err_kind(b""), ErrorKind::UnexpectedEof);
    assert_eq!(err_kind(b"   "), ErrorKind::UnexpectedEof);
    assert_eq!(err_kind(br#"["\uD800"]"#), ErrorKind::InvalidSurrogate);
    assert!(validate(br#"["\uD83D\uDE00"]"#).is_ok());
    assert!(validate(br#"["\"", "\\", "\/", "a\\\"b"]"#).is_ok());
    assert_eq!(err_kind(b"[1,]"), ErrorKind::UnexpectedCharacter);
    assert_eq!(err_kind(b"{\"a\":1,}"), ErrorKind::UnexpectedCharacter);
    assert_eq!(err_kind(b"[\"\x00\"]"), ErrorKind::ControlCharacterInString);
    assert_eq!(err_kind(b"[\"\xc3\x28\"]"), ErrorKind::InvalidUtf8);
    assert_eq!(err_kind(b"\xEF\xBB\xBF{}"), ErrorKind::UnexpectedCharacter); // BOM rejected
    assert_eq!(err_kind(b"[1 2]"), ErrorKind::UnexpectedCharacter);
    assert_eq!(err_kind(b"[1true]"), ErrorKind::InvalidNumber);
    assert_eq!(err_kind(b"truex"), ErrorKind::InvalidLiteral);
    assert_eq!(err_kind(b"NaN"), ErrorKind::UnexpectedCharacter);
    assert_eq!(err_kind(b"[.5]"), ErrorKind::UnexpectedCharacter);
    assert_eq!(err_kind(b"{} x"), ErrorKind::TrailingCharacters);
    assert_eq!(err_kind(b"\"a\"\"b\""), ErrorKind::TrailingCharacters);
    assert_eq!(err_kind(b"{\"a\"}"), ErrorKind::UnexpectedCharacter);
    assert_eq!(err_kind(b"[}"), ErrorKind::UnexpectedCharacter);
    assert_eq!(err_kind(b"{]"), ErrorKind::UnexpectedCharacter);
    // Huge numbers are valid JSON; representability is a query concern.
    assert!(validate(b"1e400").is_ok());
    assert!(validate(&[b'9'; 10_000]).is_ok());
}

#[test]
fn error_offsets() {
    assert_eq!(validate(b"[1,  ]").unwrap_err().offset(), 5);
    assert_eq!(validate(b"{\"a\":tru}").unwrap_err().offset(), 5);
    assert_eq!(validate(b"[\"ok\", \"\\q\"]").unwrap_err().offset(), 8);
}

#[test]
fn depth_limit() {
    let nested = |d: usize| [vec![b'['; d], vec![b']'; d]].concat();
    assert!(validate(&nested(1000)).is_ok());
    assert!(validate(&nested(1024)).is_ok());
    assert_eq!(err_kind(&nested(1025)), ErrorKind::DepthLimitExceeded);
    // Far beyond the limit must still fail cleanly (no recursion, no stack overflow).
    assert_eq!(err_kind(&nested(1_000_000)), ErrorKind::DepthLimitExceeded);
    let limits = Limits { max_depth: 2, ..Limits::default() };
    assert!(Document::parse_with_limits(b"[[1]]".as_slice(), &limits).is_ok());
    assert!(Document::parse_with_limits(b"[[[1]]]".as_slice(), &limits).is_err());
}

#[test]
fn input_size_limit() {
    let limits = Limits { max_input_len: 4, ..Limits::default() };
    assert!(Document::parse_with_limits(b"[1]".as_slice(), &limits).is_ok());
    let e = Document::parse_with_limits(b"[1,2]".as_slice(), &limits).err().unwrap();
    assert_eq!(e.kind(), ErrorKind::InputTooLarge);
}

const API: &[u8] = br#"{
  "user": {"id": 42, "name": "Ada", "email": null, "active": true, "score": 9.5,
           "nick": "\u0041da\n", "big": 123456789012345678901},
  "items": [{"price": 1}, {"price": 2}, {"price": 3}, {"price": 4.25, "tags": ["x", [7]]}],
  "dup": 1, "dup": 2,
  "a\u002eb": "escaped key"
}"#;

#[test]
fn query_values() {
    let doc = Document::parse(API).unwrap();
    assert_eq!(doc.get("user.name").unwrap().as_str().unwrap(), "Ada");
    assert!(matches!(doc.get("user.name").unwrap().as_str().unwrap(), std::borrow::Cow::Borrowed(_)));
    assert_eq!(doc.get("user.nick").unwrap().as_str().unwrap(), "Ada\n");
    assert_eq!(doc.get("user.id").unwrap().as_i64().unwrap(), 42);
    assert_eq!(doc.get("user.score").unwrap().as_f64().unwrap(), 9.5);
    assert!(doc.get("user.active").unwrap().as_bool().unwrap());
    assert_eq!(doc.get("items[3].price").unwrap().as_f64().unwrap(), 4.25);
    assert_eq!(doc.get("items[3].tags[1][0]").unwrap().as_i64().unwrap(), 7);
    assert_eq!(doc.get("items[0]").unwrap().get("price").unwrap().as_i64().unwrap(), 1);
    assert_eq!(doc.get("").unwrap().kind(), ValueKind::Object);
    assert_eq!(doc.get("dup").unwrap().as_i64().unwrap(), 1, "first duplicate wins");
}

#[test]
fn query_distinguishes_missing_null_and_type() {
    let doc = Document::parse(API).unwrap();
    let email = doc.get("user.email").unwrap();
    assert!(email.is_null());
    assert_eq!(
        email.as_str().unwrap_err(),
        QueryError::TypeMismatch { expected: ValueKind::String, found: ValueKind::Null }
    );
    assert_eq!(doc.get("user.phone").unwrap_err(), QueryError::Missing);
    assert_eq!(doc.get("items[4]").unwrap_err(), QueryError::Missing);
    assert_eq!(doc.get("nope.deeper").unwrap_err(), QueryError::Missing);
    assert_eq!(
        doc.get("user.id").unwrap().as_str().unwrap_err(),
        QueryError::TypeMismatch { expected: ValueKind::String, found: ValueKind::Number }
    );
    assert_eq!(
        doc.get("items.price").unwrap_err(),
        QueryError::TypeMismatch { expected: ValueKind::Object, found: ValueKind::Array }
    );
    assert_eq!(
        doc.get("user[0]").unwrap_err(),
        QueryError::TypeMismatch { expected: ValueKind::Array, found: ValueKind::Object }
    );
    assert_eq!(doc.get("user.score").unwrap().as_i64().unwrap_err(), QueryError::NumberNotRepresentable);
    assert_eq!(doc.get("user.big").unwrap().as_i64().unwrap_err(), QueryError::NumberNotRepresentable);
    assert_eq!(doc.get("user.big").unwrap().as_number_str().unwrap(), "123456789012345678901");
    let huge = Document::parse(b"1e400".as_slice()).unwrap();
    assert_eq!(huge.root().as_f64().unwrap_err(), QueryError::NumberNotRepresentable);
}

#[test]
fn query_matches_decoded_keys() {
    let doc = Document::parse(br#"{"\u0061b":1,"q\"uote":2}"#.as_slice()).unwrap();
    assert_eq!(doc.get("ab").unwrap().as_i64().unwrap(), 1);
    assert_eq!(doc.get("q\"uote").unwrap().as_i64().unwrap(), 2);
}

#[test]
fn invalid_paths() {
    let doc = Document::parse(API).unwrap();
    for (path, position) in
        [(".a", 0), ("a.", 2), ("a..b", 2), ("a[", 2), ("a[x]", 2), ("a[01]", 4), ("a[0]b", 4), ("a]", 1), ("[]", 1)]
    {
        assert_eq!(doc.get(path).unwrap_err(), QueryError::InvalidPath { position }, "{path}");
    }
    assert!(matches!(doc.get("a[99999999999999999999999]"), Err(QueryError::InvalidPath { .. })));
}

#[test]
fn projection() {
    let doc = Document::parse(API).unwrap();
    let got = doc.project(&["user.id", "user.name", "user.active", "user.phone"]);
    assert_eq!(got[0].unwrap().as_i64().unwrap(), 42);
    assert_eq!(got[1].unwrap().as_str().unwrap(), "Ada");
    assert!(got[2].unwrap().as_bool().unwrap());
    assert_eq!(got[3].unwrap_err(), QueryError::Missing);
}

#[test]
fn owned_document_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>(_: &T) {}
    let doc = Document::parse(API.to_vec()).unwrap();
    assert_send_sync(&doc);
    let name = std::thread::scope(|s| s.spawn(|| doc.get("user.name").unwrap().as_str().unwrap().into_owned()).join());
    assert_eq!(name.unwrap(), "Ada");
}

/// UTF-8 is checked over the whole input: every 1–4 byte sequence over
/// boundary-heavy byte values, inside a string at every 8-byte alignment, must
/// agree with `std::str::from_utf8` (validity and error offset).
#[test]
fn utf8_matches_std() {
    const BYTES: [u8; 22] = [
        0x20, 0x41, 0x7F, 0x80, 0x8F, 0x90, 0x9F, 0xA0, 0xBF, 0xC0, 0xC1, 0xC2, 0xDF, 0xE0, 0xE1, 0xED,
        0xEE, 0xF0, 0xF1, 0xF4, 0xF5, 0xFF,
    ];
    let mut seqs: Vec<Vec<u8>> = vec![vec![]];
    for _ in 0..4 {
        let longest = seqs.last().unwrap().len();
        let longer: Vec<Vec<u8>> = seqs
            .iter()
            .filter(|s| s.len() == longest)
            .flat_map(|s| BYTES.iter().map(move |&b| [s.as_slice(), &[b]].concat()))
            .collect();
        seqs.extend(longer);
    }
    for seq in &seqs {
        for pad in 0..9 {
            let input = [&b"[\""[..], &vec![b'a'; pad], seq, b"zzz\"]"].concat();
            let expected = std::str::from_utf8(&input).map(drop).map_err(|e| (ErrorKind::InvalidUtf8, e.valid_up_to()));
            assert_eq!(validate(&input).map_err(|e| (e.kind(), e.offset())), expected, "{input:02X?}");
        }
    }
}

/// Control bytes are rejected anywhere inside a string (including right after a
/// backslash and across 64-byte scanner blocks) and allowed as whitespace outside.
#[test]
fn control_bytes_in_strings() {
    for pos in 0..150 {
        for control in [0x00u8, 0x01, b'\t', b'\n', b'\r', 0x1F] {
            let mut input = b"[\"".to_vec();
            input.extend(std::iter::repeat_n(b'x', 150));
            input.extend_from_slice(b"\"]");
            input[2 + pos] = control;
            let e = validate(&input).unwrap_err();
            assert_eq!((e.kind(), e.offset()), (ErrorKind::ControlCharacterInString, 2 + pos));
            if pos > 0 {
                input[1 + pos] = b'\\';
                let e = validate(&input).unwrap_err();
                assert_eq!((e.kind(), e.offset()), (ErrorKind::ControlCharacterInString, 2 + pos));
            }
        }
    }
    assert!(validate(b"\t[\r\n1,\t\"a\" ]\n").is_ok());
    assert!(validate(b"\"\x7F\"").is_ok()); // DEL is not a control character in JSON
}
