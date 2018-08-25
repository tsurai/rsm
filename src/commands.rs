use std::io::prelude::*;
use std::io;
use failure::*;
use ansi_term::{Style, Colour};
use content;
use util;
use db;

pub enum ModifyOperation<'a> {
    Name(String),
    Add(Vec<&'a str>),
    Remove(Vec<&'a str>),
    Content,
}

pub fn add_snippet(name: String, tags: Option<Vec<&str>>) -> Result<(), Error> {
    let content = (if util::is_a_tty() {
        content::get_from_editor(None)
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

pub fn modify_snippet(snippet_id: i64, op: ModifyOperation) -> Result<(), Error> {
    let conn = db::connect()
        .context("failed to connect to database")?;

    let snippet = db::get_snippet(&conn, snippet_id)
        .context("failed to load snippet")?;

    match op {
        ModifyOperation::Name(name) => {
            db::rename_snippet(&conn, snippet_id, name)
                .context("failed to rename snippet")?;
        },
        ModifyOperation::Add(tags) => {
            db::save_tags(&conn, snippet_id, tags)
                .context("failed to add tags to snippet")?;
        },
        ModifyOperation::Remove(tags) => {
            db::remove_tags(&conn, snippet_id, tags)
                .context("failed to remove tags to snippet")?;
        },
        ModifyOperation::Content => {
            let content = content::get_from_editor(Some(snippet.content))
                .context("failed to get new content from editor")?;

            db::change_snippet_content(&conn, snippet_id, content)
                .context("failed to change snippet content")?;
        },
    }

    Ok(())
}

pub fn delete_snippet(snippet_id: i64, confirmation: bool) -> Result<(), Error> {
    let conn = db::connect()
        .context("failed to connect to database")?;

    let snippet = db::get_snippet(&conn, snippet_id)
        .context("failed to load snippet")?;

    if !confirmation {
        let stdin = io::stdin();
        let mut handle = stdin.lock();

        loop {
            print!("Delete snippet {} '{}' (yes/no) ", snippet_id, snippet.name);
            io::stdout().flush()
                .context("failed to flush stdout")?;

            let mut buffer = String::new();
            handle.read_line(&mut buffer)
                .context("failed to read user input")?;

            if buffer != "\n" {
                let input = &buffer.as_str()[..buffer.len()-1];

                if "yes".starts_with(input) {
                    break;
                }

                if "no".starts_with(input) {
                    println!("Snippet not deleted");
                    return Ok(());
                }
            }
        }
    }

    db::delete_snippet(&conn, snippet_id)
        .context("failed to delete snippet")?;
    println!("Deleted snippet {} '{}'", snippet_id, snippet.name);

    Ok(())
}

pub fn list_snippets(name: Option<String>, tags: Option<Vec<&str>>) -> Result<(), Error> {
    let conn = db::connect()
        .context("failed to connect to database")?;

    let snippets = db::search_snippets(&conn, name, tags)
        .context("failed to search snippets")?;

    if snippets.is_empty() {
        println!("No snippets found");
        return Ok(());
    }

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
