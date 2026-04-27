use std::collections::HashMap;
use crate::models::SteamApp;

#[derive(Debug, Clone, PartialEq)]
pub enum VdfValue {
    String(String),
    Map(HashMap<String, VdfValue>),
}

impl VdfValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            VdfValue::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<&HashMap<String, VdfValue>> {
        match self {
            VdfValue::Map(m) => Some(m),
            _ => None,
        }
    }
}

struct Lexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            // Skip whitespace
            while self.pos < self.input.len()
                && self.input.as_bytes()[self.pos].is_ascii_whitespace()
            {
                self.pos += 1;
            }
            // Skip line comments
            if self.pos + 1 < self.input.len()
                && &self.input[self.pos..self.pos + 2] == "//"
            {
                while self.pos < self.input.len()
                    && self.input.as_bytes()[self.pos] != b'\n'
                {
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip_whitespace_and_comments();
        self.input.as_bytes().get(self.pos).copied()
    }

    fn next_token(&mut self) -> Option<Token> {
        self.skip_whitespace_and_comments();
        if self.pos >= self.input.len() {
            return None;
        }
        let b = self.input.as_bytes()[self.pos];
        match b {
            b'{' => {
                self.pos += 1;
                Some(Token::OpenBrace)
            }
            b'}' => {
                self.pos += 1;
                Some(Token::CloseBrace)
            }
            b'"' => {
                self.pos += 1; // skip opening quote
                let mut s = String::new();
                loop {
                    if self.pos >= self.input.len() {
                        break;
                    }
                    let c = self.input.as_bytes()[self.pos];
                    if c == b'\\' && self.pos + 1 < self.input.len() {
                        let next = self.input.as_bytes()[self.pos + 1];
                        match next {
                            b'"' => {
                                s.push('"');
                                self.pos += 2;
                            }
                            b'n' => {
                                s.push('\n');
                                self.pos += 2;
                            }
                            b't' => {
                                s.push('\t');
                                self.pos += 2;
                            }
                            b'\\' => {
                                s.push('\\');
                                self.pos += 2;
                            }
                            _ => {
                                s.push('\\');
                                self.pos += 1;
                            }
                        }
                    } else if c == b'"' {
                        self.pos += 1; // skip closing quote
                        break;
                    } else {
                        // Multi-byte UTF-8 safety: advance by char boundary
                        let ch = self.input[self.pos..]
                            .chars()
                            .next()
                            .unwrap_or('\0');
                        s.push(ch);
                        self.pos += ch.len_utf8();
                    }
                }
                Some(Token::Str(s))
            }
            _ => None,
        }
    }
}

#[derive(Debug)]
enum Token {
    Str(String),
    OpenBrace,
    CloseBrace,
}

fn parse_map(lexer: &mut Lexer) -> Result<HashMap<String, VdfValue>, String> {
    let mut map = HashMap::new();
    loop {
        match lexer.peek() {
            None | Some(b'}') => break,
            _ => {}
        }
        // Expect a key string
        let key = match lexer.next_token() {
            Some(Token::Str(s)) => s,
            Some(Token::CloseBrace) => break,
            other => {
                return Err(format!("Expected key string, got {:?}", other));
            }
        };
        // Expect either a value string or an open brace
        match lexer.peek() {
            Some(b'{') => {
                lexer.next_token(); // consume '{'
                let sub = parse_map(lexer)?;
                // consume '}'
                match lexer.next_token() {
                    Some(Token::CloseBrace) => {}
                    other => return Err(format!("Expected '}}', got {:?}", other)),
                }
                map.insert(key.to_lowercase(), VdfValue::Map(sub));
            }
            Some(b'"') => {
                let value = match lexer.next_token() {
                    Some(Token::Str(s)) => s,
                    other => return Err(format!("Expected value string, got {:?}", other)),
                };
                map.insert(key.to_lowercase(), VdfValue::String(value));
            }
            other => {
                return Err(format!(
                    "Unexpected token after key '{}': {:?}",
                    key, other
                ));
            }
        }
    }
    Ok(map)
}

pub fn parse_vdf(content: &str) -> Result<HashMap<String, VdfValue>, String> {
    let mut lexer = Lexer::new(content);
    // VDF files may have an outer key wrapping everything, or may be a raw map.
    // Handle both: if the first token is a string, it's a top-level key.
    match lexer.peek() {
        Some(b'"') => {
            // Read the top-level key
            let _key = match lexer.next_token() {
                Some(Token::Str(s)) => s,
                other => return Err(format!("Expected top-level key, got {:?}", other)),
            };
            // Read '{'
            match lexer.next_token() {
                Some(Token::OpenBrace) => {}
                other => return Err(format!("Expected '{{' after top-level key, got {:?}", other)),
            }
            let map = parse_map(&mut lexer)?;
            // Read closing '}'
            match lexer.next_token() {
                Some(Token::CloseBrace) | None => {}
                other => return Err(format!("Expected closing '}}', got {:?}", other)),
            }
            Ok(map)
        }
        Some(b'{') => {
            lexer.next_token(); // consume '{'
            let map = parse_map(&mut lexer)?;
            lexer.next_token(); // consume '}'
            Ok(map)
        }
        None => Ok(HashMap::new()),
        other => Err(format!("Unexpected start of VDF: {:?}", other)),
    }
}

pub fn parse_acf(content: &str) -> Result<SteamApp, String> {
    let map = parse_vdf(content)?;

    let appid = map
        .get("appid")
        .and_then(|v| v.as_str())
        .ok_or("Missing appid")?
        .to_string();

    let name = map
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or("Missing name")?
        .to_string();

    let install_dir = map
        .get("installdir")
        .and_then(|v| v.as_str())
        .ok_or("Missing installdir")?
        .to_string();

    let size_on_disk = map
        .get("sizeondisk")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<u64>().ok());

    Ok(SteamApp {
        appid,
        name,
        install_dir,
        size_on_disk,
    })
}

pub fn parse_library_folders(content: &str) -> Result<Vec<String>, String> {
    let map = parse_vdf(content)?;

    let mut paths = Vec::new();

    for value in map.values() {
        if let VdfValue::Map(entry) = value {
            if let Some(VdfValue::String(path)) = entry.get("path") {
                paths.push(path.clone());
            }
        }
    }

    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_vdf() {
        let vdf = r#"
"AppState"
{
    "appid"     "12345"
    "name"      "My Game"
    "installdir" "MyGame"
}
"#;
        let result = parse_vdf(vdf).unwrap();
        assert_eq!(result.get("appid").and_then(|v| v.as_str()), Some("12345"));
        assert_eq!(result.get("name").and_then(|v| v.as_str()), Some("My Game"));
        assert_eq!(
            result.get("installdir").and_then(|v| v.as_str()),
            Some("MyGame")
        );
    }

    #[test]
    fn test_parse_acf() {
        let acf = r#"
"AppState"
{
    "appid"     "1245620"
    "name"      "Elden Ring"
    "installdir" "ELDEN RING"
    "SizeOnDisk" "50000000000"
}
"#;
        let app = parse_acf(acf).unwrap();
        assert_eq!(app.appid, "1245620");
        assert_eq!(app.name, "Elden Ring");
        assert_eq!(app.install_dir, "ELDEN RING");
        assert_eq!(app.size_on_disk, Some(50_000_000_000u64));
    }

    #[test]
    fn test_parse_acf_missing_size() {
        let acf = r#"
"AppState"
{
    "appid"     "730"
    "name"      "Counter-Strike 2"
    "installdir" "Counter-Strike Global Offensive"
}
"#;
        let app = parse_acf(acf).unwrap();
        assert_eq!(app.appid, "730");
        assert_eq!(app.name, "Counter-Strike 2");
        assert_eq!(app.size_on_disk, None);
    }

    #[test]
    fn test_parse_library_folders() {
        let vdf = r#"
"libraryfolders"
{
    "0"
    {
        "path"      "/Users/user/Library/Application Support/Steam"
        "label"     ""
        "contentid" "123456"
    }
    "1"
    {
        "path"      "/Volumes/ExternalDrive/SteamLibrary"
        "label"     "External"
        "contentid" "789012"
    }
}
"#;
        let paths = parse_library_folders(vdf).unwrap();
        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&"/Users/user/Library/Application Support/Steam".to_string()));
        assert!(paths.contains(&"/Volumes/ExternalDrive/SteamLibrary".to_string()));
    }

    #[test]
    fn test_parse_vdf_with_comments() {
        let vdf = r#"
// This is a top-level comment
"AppState"
{
    // appid comment
    "appid"     "99999"
    "name"      "Test Game" // inline style comment won't appear in key position
    "installdir" "TestGame"
}
"#;
        let result = parse_vdf(vdf).unwrap();
        assert_eq!(result.get("appid").and_then(|v| v.as_str()), Some("99999"));
        assert_eq!(
            result.get("name").and_then(|v| v.as_str()),
            Some("Test Game")
        );
    }

    #[test]
    fn test_parse_vdf_escaped_strings() {
        let vdf = r#"
"AppState"
{
    "appid"     "11111"
    "name"      "Game with \"Quotes\" inside"
    "installdir" "GameDir"
}
"#;
        let result = parse_vdf(vdf).unwrap();
        assert_eq!(
            result.get("name").and_then(|v| v.as_str()),
            Some("Game with \"Quotes\" inside")
        );
    }
}
