use std::collections::BTreeMap;
use thiserror::Error;

//TODO: put into a embedded ron file

static KEYWORD_ID: [(char, &str); 31] = [
    ('\u{C101}', "ORANGE"),
    ('\u{C102}', "PINK"),
    ('\u{C103}', "RED"),
    ('\u{C104}', "GREEN"),
    ('\u{C105}', "LIGHTBLUE"),
    ('\u{C106}', "YELLOW"),
    ('\u{C107}', "WHITE"),
    ('\u{C108}', "GRAY"),
    ('\u{C109}', "PINK"),
    ('\u{C10A}', "RED"),
    ('\u{C10B}', "BLACK"),
    ('\u{C10C}', "DARKGRAY"),
    ('\u{C10D}', "DARKGREEN"),
    ('\u{C10E}', "BLUE"),
    ('\u{C10F}', "COLOREND"),
    ('\u{C200}', "CENTER"),
    ('\u{D100}', "PLAYERNAME"),
    ('\u{D200}', "PARTNERNAME"),
    ('\u{D301}', "PLAYERPOKEMON"),
    ('\u{D302}', "PARTNERPOKEMON"),
    ('\u{A072}', "POKE"),
    ('\u{A09B}', "BUTTONA"),
    ('\u{A09C}', "BUTTONB"),
    ('\u{A09D}', "BUTTONX"),
    ('\u{A09E}', "BUTTONY"),
    ('\u{A09F}', "BUTTONL"),
    ('\u{A0A0}', "BUTTONR"),
    ('\u{B200}', "SPEAKERNORMAL"),
    ('\u{B201}', "SPEAKERHAPPY"),
    ('\u{B202}', "SPEAKERPAINED"),
    ('\u{EB00}', "PAUSE"),
];

#[derive(Error, Debug, PartialEq)]
pub enum MessageKeywordEncodeError {
    #[error("The final character is an escape character ('\\'). If you want to use the \\ character, use \\\\.")]
    NoCharAfterEscape,
    #[error("The character {0} is escaped (preceded by \\). This is useless, and thus reported as an error to prevent human error. If you want to use \\, write \\\\.")]
    UselessEscape(char),
    #[error("The final character is part of a bracketed text. Either close the bracker (add ']' at end of text), or escape the first one if you don't want to replace it with a special character (by writing '\\' before '[').")]
    NoCharInBracket,
    #[error("The escape sequence {0:?} isn't reconized. If you didn't wanted to use an escape sequence, you can use '\\[' instead of '['")]
    UnknownEscape(String)
}

pub struct MessageKeyword {
    id_by_string: BTreeMap<String, char>,
    string_by_id: BTreeMap<char, String>,
}

impl MessageKeyword {
    pub fn new_empty() -> Self {
        MessageKeyword {
            id_by_string: BTreeMap::new(),
            string_by_id: BTreeMap::new(),
        }
    }

    pub fn new_default() -> Self {
        let mut keywords = Self::new_empty();
        for (id, text) in &KEYWORD_ID {
            keywords.add_keyword(*id, text.to_string());
        }
        keywords
    }

    pub fn add_keyword(&mut self, id: char, text: String) {
        self.string_by_id.insert(id, text.clone());
        self.id_by_string.insert(text, id);
    }

    pub fn decode(&self, input: &str) -> String {
        let mut result = String::with_capacity(input.len() + 30);
        for chara in input.chars() {
            if let Some(element) = self.string_by_id.get(&chara) {
                result.push('[');
                result.push_str(&element);
                result.push(']');
            } else if chara == '[' {
                result.push_str("\\[");
            } else if chara == '\\' {
                result.push_str("\\\\");
            } else {
                result.push(chara);
            };
        }
        result
    }

    pub fn encode(&self, decoded: &str) -> Result<String, MessageKeywordEncodeError> {
        //TODO: error
        let mut result = String::with_capacity(decoded.len());
        let mut char_iter = decoded.chars();
        loop {
            let chara = if let Some(v) = char_iter.next() {
                v
            } else {
                break;
            };
            match chara {
                '\\' => {
                    let escaped_char = char_iter
                        .next()
                        .map_or_else(|| Err(MessageKeywordEncodeError::NoCharAfterEscape), Ok)?;
                    match escaped_char {
                        '\\' | '[' => result.push(escaped_char),
                        other => return Err(MessageKeywordEncodeError::UselessEscape(other)),
                    }
                }
                '[' => {
                    let mut bracket_buffer = String::with_capacity(20);
                    loop {
                        let in_bracket_chara = char_iter
                            .next()
                            .map_or_else(|| Err(MessageKeywordEncodeError::NoCharInBracket), Ok)?;
                        if in_bracket_chara == ']' {
                            break;
                        } else {
                            bracket_buffer.push(in_bracket_chara)
                        }
                    }
                    let special_chara = *self.id_by_string.get(&bracket_buffer).map_or_else(|| Err(MessageKeywordEncodeError::UnknownEscape(bracket_buffer)), Ok)?;
                    result.push(special_chara);
                }
                c => result.push(c),
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
mod test {
    use crate::{MessageKeyword, MessageKeywordEncodeError};

    #[test]
    fn test_message_keyword() {
        let keywords = MessageKeyword::new_default();
        
        let source = "\\[ [RED] \\\\";
        assert_eq!(&keywords.decode(&keywords.encode(source).unwrap()), source);
        assert_eq!(keywords.encode("\\"), Err(MessageKeywordEncodeError::NoCharAfterEscape));
        assert_eq!(keywords.encode("\\a"), Err(MessageKeywordEncodeError::UselessEscape('a')));
        assert_eq!(keywords.encode("[RED"), Err(MessageKeywordEncodeError::NoCharInBracket));
        match keywords.encode("[SOMETHING_UNKNOWN]") {
            Err(MessageKeywordEncodeError::UnknownEscape(_)) => (),
            x => panic!("{:?}", x)
        };
    }
}