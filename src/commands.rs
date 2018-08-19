use failure::*;
use content;
use util;
use db;

pub fn add_snippet(name: String, tags: Option<Vec<&str>>) -> Result<(), Error> {
    let content = (if util::is_a_tty() {
        content::get_from_editor()
            .context("failed to get content from editor")
    } else {
        content::get_from_stdin()
            .context("failed to get content from stdin")
    })
    .context("failed to get snippet content")?;

    let conn = db::connect()
        .context("failed to connect to the database")?;

    db::save_snippet(&conn, name, content, tags)
        .context("failed to save snippet")?;

    Ok(())
}
