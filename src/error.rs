use failure::*;

#[derive(Fail, Debug, PartialEq)]
#[fail(display = "duplicate snippet name")]
pub struct DupSnippetName;
