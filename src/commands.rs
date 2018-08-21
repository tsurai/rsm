use failure::*;
use ansi_term::{Style, Colour};
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

pub fn list_snippets(name: Option<String>, tags: Option<Vec<&str>>) -> Result<(), Error> {
    let conn = db::connect()
        .context("failed to connect to database")?;

    let snippets = db::search_snippets(&conn, name, tags)
        .context("failed to search snippets")?;

    // get the max width for each list column
    let (id_padding, tag_padding, name_padding) = util::get_list_col_widths(&snippets);

    // print list header
    let style = Style::new().underline();
    println!("{} {} {}",
             style.paint(format!("{:1$}", "Id", id_padding)),
             style.paint(format!("{:1$}", "Tags", tag_padding)),
             style.paint(format!("{:1$}", "Name", name_padding)));

    for (i, snippet) in snippets.iter().enumerate() {
        let style = if i % 2 == 0 {
            Style::new()
        } else {
            Style::new().on(Colour::Fixed(235))
        };

        let snippet_line = format!("{:3$} {:4$} {:5$}",
                                   snippet.id,
                                   snippet.tags.as_slice().join(", "),
                                   snippet.name, id_padding,
                                   tag_padding,
                                   name_padding);

        // print line with backgrounbd color
        println!("{}", style.paint(snippet_line));
    }

    Ok(())
}
