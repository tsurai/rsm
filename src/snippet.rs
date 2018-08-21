use std::fmt;
use ansi_term;

pub struct Snippet {
    pub id: i64,
    pub name: String,
    pub tags: Vec<String>,
    pub content: String
}

impl fmt::Display for Snippet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}\n", ansi_term::Style::new().bold().paint("Name:"), self.name)?;

        if !self.tags.is_empty() {
            write!(f, "{} {}\n", ansi_term::Style::new().bold().paint("Tags:"), self.tags.as_slice().join(", "))?;
        }

        write!(f, "\n{}", self.content)
    }
}
