use std::path::PathBuf;
use std::{env, fs};
use sqlite::{self, Connection, Value, State};
use failure::*;
use snippet::Snippet;
use error;

static DB_PATH: &'static str = ".local/rsm/data.db";

pub fn connect() -> Result<Connection, Error> {
    let db_file = get_db_path()
        .context("failed to get database path")?;

    // check if there is no database yet
    let is_init = db_file.exists();

    // init database if necessary
    let conn = if !is_init {
        init(&db_file)
            .context("failed to initialize database")?
    } else {
        sqlite::open(db_file.to_str().unwrap())
            .map_err::<Error, _>(|e| e.into())?
    };

    conn.execute("PRAGMA foreign_keys = ON")
        .context("failed to enable foreign_key support")?;

    Ok(conn)
}

pub fn search_snippets(conn: &Connection, name: Option<String>, tags: Option<Vec<&str>>) -> Result<Vec<Snippet>, Error> {
    let name_filter = name.clone().map_or(if tags.is_some() { "0" } else { "1" }, |_| "S.name LIKE ?");
    let tag_filter = tags.clone().map_or(if name.is_some() { "0" } else { "1" }.to_string(), |x| (0..x.len()).map(|_| "T.name LIKE ?").collect::<Vec<&str>>().as_slice().join(" AND "));

    let query = format!(
        "SELECT S.id, S.name, S.content FROM `snippets` AS S
        WHERE {}
        UNION
        SELECT S.id, S.name, S.content FROM `snippets` AS S
        INNER JOIN `snippet_tags` AS ST ON ST.snippet_id = S.id
        INNER JOIN `tags` AS T ON T.id = ST.tag_id
        WHERE {} GROUP BY S.id",
        name_filter,
        tag_filter);

    let mut statement = conn.prepare(query.as_str())
        .context("failed to prepare load statement")?;

    let mut bind_count = 1;

    if name.is_some() {
        statement.bind(bind_count, name.unwrap().as_str())
            .context("failed to bind name")?;
        bind_count += 1;
    }

    if tags.is_some() {
        for tag in tags.unwrap() {
            statement.bind(bind_count, tag)
                .context("failed to bind tag")?;
            bind_count += 1;
        }
    }

    let mut snippets = Vec::new();

    while let State::Row = statement.next().context("failed to execute sql statement")? {
        let snippet_id = statement.read::<i64>(0)
            .context("failed to read snippet id")?;
        let name = statement.read::<String>(1)
            .context("failed to read snippet name")?;
        let content = statement.read::<String>(2)
            .context("failed to read content")?;

        let tags = get_snippet_tags(conn, snippet_id)
            .context("failed to load snippet tags")?;

        let snippet = Snippet {
            id: snippet_id,
            name: name,
            content: content,
            tags: tags
        };

        snippets.push(snippet);
    }

    Ok(snippets)
}

pub fn get_snippet(conn: &Connection, snippet_id: i64) -> Result<Snippet, Error> {
    let mut statement = conn.prepare("SELECT name, content FROM `snippets` WHERE id = ?")
        .context("failed to prepare load statement")?;

    statement.bind(1, snippet_id)
        .context("failed to bind snippet id")?;

    let state = statement.next()
        .context("failed to execute sql statement")?;

    if state == State::Done {
        bail!(error::UnknownSnippetId);
    }

    let name = statement.read::<String>(0)
        .context("failed to read snippet name")?;
    let content = statement.read::<String>(1)
        .context("failed to read snippet content")?;

    let tags = get_snippet_tags(conn, snippet_id)
        .context("failed to load snippet tags")?;

    let snippet = Snippet {
        id: snippet_id,
        name: name,
        content: content,
        tags: tags
    };

    Ok(snippet)
}

pub fn delete_snippet(conn: &Connection, snippet_id: i64) -> Result<(), Error> {
    let mut statement = conn.prepare("DELETE FROM `snippets` WHERE id = ?")
        .context("failed to prepare load statement")?;

    statement.bind(1, snippet_id)
        .context("failed to bind snippet id")?;

    statement.next()
        .context("failed to execute sql statement")?;

    Ok(())
}

pub fn save_snippet(conn: &Connection, name: String, content: String, tags: Option<Vec<&str>>) -> Result<i64, Error> {
    let mut statement = conn.prepare("INSERT INTO `snippets` (name, content) VALUES (?, ?)")
        .context("failed to prepare save statement")?;

    // bind INSERT values
    statement.bind(1, name.as_str())
        .context("failed to bind name")?;
    statement.bind(2, content.as_str())
        .context("failed to bind content")?;

    if let Err(e) = statement.next() {
        // check if the snippet name is a duplicate
        if e.code.unwrap_or(0) == 19 {
            bail!(error::DupSnippetName);
        }

        bail!(e.context("failed to execute sql statement"))
    }

    let snippet_id = get_snippet_id(&conn, name.as_str())
        .context("failed to get snippet id")?;

    if let Some(tags) = tags {
        save_tags(&conn, snippet_id, tags)
            .context("failed to save snippet tags")?;
    }

    Ok(snippet_id)
}

pub fn change_snippet_content(conn: &Connection, snippet_id: i64, content: String) -> Result<(), Error> {
    let mut statement = conn.prepare("UPDATE `snippets` SET content = ? WHERE id = ?;")
        .context("failed to prepare content change statement")?;

    statement.bind(1, content.as_str())
        .context("failed to bind content")?;
    statement.bind(2, snippet_id)
        .context("failed to bind id")?;

    statement.next()
        .context("failed to execute sql statement")?;

    Ok(())
}

pub fn rename_snippet(conn: &Connection, snippet_id: i64, name: String) -> Result<(), Error> {
    let mut statement = conn.prepare("UPDATE `snippets` SET name = ? WHERE id = ?;")
        .context("failed to prepare snippet rename statement")?;

    statement.bind(1, name.as_str())
        .context("failed to bind name")?;
    statement.bind(2, snippet_id)
        .context("failed to bind id")?;

    statement.next()
        .context("failed to execute sql statement")?;

    Ok(())
}

pub fn remove_tags(conn: &Connection, snippet_id: i64, tags: Vec<&str>) -> Result<(), Error> {
    let tags_filter = tags.clone()
        .iter()
        .map(|_| "?")
        .collect::<Vec<&str>>()
        .as_slice()
        .join(", ");

    let query = format!(
        "DELETE FROM `snippet_tags` AS ST
        WHERE ST.snippet_id = ? AND ST.tag_id IN (
            SELECT id FROM tags
            WHERE tags.name IN ({})
        )", tags_filter);

    let mut statement = conn.prepare(query)
        .context("failed to prepare snippet rename statement")?;

    statement.bind(1, snippet_id)
        .context("failed to bind id")?;

    for (i, &tag) in tags.iter().enumerate() {
        statement.bind(2+i, tag)
            .context("failed to bind name")?;
    }

    statement.next()
        .context("failed to execute sql statement")?;

    Ok(())
}

pub fn save_tags(conn: &Connection, snippet_id: i64, tags: Vec<&str>) -> Result<(), Error> {
    // insert tag if not exists
    let mut insert_tag = conn.prepare(
       "INSERT INTO `tags` (name)
       VALUES (?)"
       ).context("failed to prepare tag save statement")?
       .cursor();

    // save relationship between snippet and tag
    let mut insert_snippet_tag = conn.prepare(
        "INSERT INTO `snippet_tags` (snippet_id, tag_id)
        SELECT ?, id
        FROM `tags`
        WHERE name = ?"
        ).context("failed to prepare snippet_tag save statement")?
        .cursor();

    for tag in tags.clone() {
        insert_tag.bind(&[Value::String(tag.to_string())])
            .context("failed to bind name")?;

        insert_tag.next()
            .context("failed to execute sql statement")?;

        insert_snippet_tag.bind(&[Value::Integer(snippet_id), Value::String(tag.to_string())])
            .context("failed to bind values")?;

        insert_snippet_tag.next()
            .context("failed to execute sql statement")?;
    }

    Ok(())
}

fn get_snippet_id(conn: &Connection, name: &str) -> Result<i64, Error> {
    let mut statement = conn.prepare("SELECT id FROM `snippets` WHERE name = ?")
        .context("failed to prepare save statement")?;

    statement.bind(1, name)
        .context("failed to bind name")?;

    statement.next()
        .context("failed to execute sql statement")?;

    let snippet_id = statement.read::<i64>(0)
        .context("failed to select snippet")?;

    Ok(snippet_id)
}

fn get_snippet_tags(conn: &Connection, snippet_id: i64) -> Result<Vec<String>, Error> {
    let mut statement = conn.prepare(
        "SELECT name FROM `tags` AS T
        INNER JOIN `snippet_tags` AS ST on T.id = ST.tag_id
        AND ST.snippet_id = ?"
        ).context("failed to prepare load statement")?;

    statement.bind(1, snippet_id)
        .context("failed to bind snippet id")?;

    let mut tags = Vec::new();

    while let State::Row = statement.next().context("failed to execute sql statement")? {
       let tag = statement.read::<String>(0)
           .context("failed to read tag name")?;

       tags.push(tag);
    }

    Ok(tags)
}

fn get_db_path() -> Result<PathBuf, Error> {
    let mut path = PathBuf::from(env::var("HOME")
        .context("failed to get HOME directory")?);

    path.push(DB_PATH);

    Ok(path)
}

fn init(db_file: &PathBuf) -> Result<Connection, Error> {
    let path = db_file.parent().unwrap();

    fs::create_dir(path)
        .context(format!("failed to create database directory: {:?}", path))?;

    let conn = sqlite::open(db_file.to_str().unwrap())
        .map_err::<Error, _>(|e| e.into())?;

    // initialize database tables
    conn.execute(
        "CREATE TABLE snippets(
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name VARCHAR(64) UNIQUE,
            content TEXT
        );
        CREATE TABLE tags(
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name VARCHAR(64),
            UNIQUE(name) ON CONFLICT IGNORE
        );
        CREATE TABLE snippet_tags(
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            snippet_id INTEGER REFERENCES snippets(id) ON DELETE CASCADE,
            tag_id INTEGER REFERENCES tags(id) ON DELETE CASCADE,
            UNIQUE(snippet_id, tag_id) ON CONFLICT IGNORE
        );"
        ).context("failed to create database tables")?;

    Ok(conn)
}
