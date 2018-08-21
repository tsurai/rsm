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

    let snippet_id = db::save_snippet(&conn, name, content, tags)
        .context("failed to save snippet")?;

    println!("Created snippet {}.", snippet_id);

    Ok(())
}

pub fn show_snippet(snippet_id: i64) -> Result<(), Error> {
    let conn = db::connect()
        .context("failed to connect to database")?;

    let snippet = db::get_snippet(&conn, snippet_id)
        .context("failed to load snippet")?;

    println!("{}", snippet);

    Ok(())
}
