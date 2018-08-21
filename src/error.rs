use failure::*;

#[derive(Fail, Debug)]
#[fail(display = "duplicate snippet name")]
pub struct DupSnippetName;

#[derive(Fail, Debug)]
#[fail(display = "unknown snippet id")]
pub struct UnknownSnippetId;
