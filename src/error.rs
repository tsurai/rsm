use failure::*;

#[derive(Fail, Debug)]
#[fail(display = "duplicate snippet name")]
pub struct DupSnippetName;

#[derive(Fail, Debug)]
#[fail(display = "unknown snippet id")]
pub struct UnknownSnippetId;

#[derive(Fail, Debug)]
#[fail(display = "unknown metadata key")]
pub struct UnknownMetaKey;

#[derive(Fail, Debug)]
#[fail(display = "syncing is not enabled")]
#[cfg(not(feature = "sync"))]
pub struct SyncingNotEnabled;
