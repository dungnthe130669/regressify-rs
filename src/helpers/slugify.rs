pub fn slugify(s: &str) -> String {
    let mut slug = String::with_capacity(s.len());
    // Starts with true to avoid leading -
    let mut prev_is_dash = true;
    {
        let mut push_char = |x: u8| {
            match x {
                b'a'..=b'z' | b'0'..=b'9' => {
                    prev_is_dash = false;
                    slug.push(x.into());
                }
                b'A'..=b'Z' => {
                    prev_is_dash = false;
                    // Manual lowercasing as Rust to_lowercase() is unicode
                    // aware and therefore much slower
                    slug.push((x - b'A' + b'a').into());
                }
                _ => {
                    if !prev_is_dash {
                        slug.push('-');
                        prev_is_dash = true;
                    }
                }
            }
        };

        for c in s.chars() {
            if c.is_ascii() {
                (push_char)(c as u8);
            } else {
                for &cx in deunicode::deunicode_char(c).unwrap_or("-").as_bytes() {
                    (push_char)(cx);
                }
            }
        }
    }

    if slug.ends_with('-') {
        slug.pop();
    }
    // We likely reserved more space than needed.
    slug.shrink_to_fit();
    slug
}
