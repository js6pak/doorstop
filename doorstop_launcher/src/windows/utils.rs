pub(super) fn trim_start(text: &[u16]) -> &[u16] {
    for (i, c) in text.iter().enumerate() {
        if *c != ' ' as u16 {
            return &text[i..];
        }
    }

    &[0]
}

pub(super) fn strip_first_arg(command_line: &[u16]) -> &[u16] {
    let mut in_quotes = false;

    for (i, c) in command_line.iter().enumerate() {
        if *c == '"' as u16 {
            in_quotes = !in_quotes;
        } else if *c == ' ' as u16 && !in_quotes {
            return trim_start(&command_line[i..]);
        }
    }

    &[0]
}

#[test]
#[allow(clippy::needless_raw_string_hashes)]
fn test_strip_first_arg() {
    fn utf16(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>()
    }

    fn test(input: &str, expected: &str) {
        let input = utf16(input);
        let actual = strip_first_arg(input.as_slice());
        assert!(
            actual.iter().eq(utf16(expected).iter()),
            "expected: {}, actual: {}",
            expected,
            String::from_utf16_lossy(actual)
        );
    }

    test(r#""C:\doorstop_launcher.exe""#, r#""#);
    test(r#""C:\doorstop_launcher.exe"  "#, r#""#);
    test(r#""C:\doorstop_launcher.exe" arg"#, r#"arg"#);
    test(r#""C:\żółć\doorstop_launcher.exe" arg"#, r#"arg"#);
    test(r#"C:\doorstop_launcher.exe arg"#, r#"arg"#);
    test(r#""C:\Program Files\doorstop_launcher.exe" arg"#, r#"arg"#);
    test(r#"C:\"Program Files"\doorstop_launcher.exe arg"#, r#"arg"#);
}
