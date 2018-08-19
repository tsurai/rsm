use failure::*;
use content;
use util;

pub fn add_snippet(name: String, tags: Option<Vec<&str>>) -> Result<(), Error> {
    let content = (if util::is_a_tty() {
        content::get_from_editor()
            .context("failed to get content from editor")
    } else {
        content::get_from_stdin()
            .context("failed to get content from stdin")
    })
    .context("failed to get snippet content")?;

    println!("{}", content);

    Ok(())
}
