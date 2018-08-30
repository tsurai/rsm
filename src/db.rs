use std::path::PathBuf;
use std::{env, fs};
use sqlite::{self, Connection, Value, State};
use failure::*;
use snippet::Snippet;
use util;
use error;
#[cfg(feature = "sync")]
use sync;

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
    let name_filter = name.clone()
        .map_or(if tags.is_some() { "0" } else { "1" }, |_| "S.name LIKE ?");
    let tag_filter = tags.clone()
        .map_or(
            if name.is_some() { "0" } else { "1" }.to_string(),
            |x| (0..x.len()).map(|_| "T.name LIKE ?")
        .collect::<Vec<&str>>()
        .as_slice()
        .join(" AND "));

    let query = format!(
        "SELECT S.id, S.name, S.content FROM `snippets` AS S
        WHERE S.deleted = 0 AND {}
        UNION
        SELECT S.id, S.name, S.content FROM `snippets` AS S
        INNER JOIN `snippet_tags` AS ST ON ST.snippet_id = S.id
        INNER JOIN `tags` AS T ON T.id = ST.tag_id
        WHERE S.deleted = 0 AND {}
        GROUP BY S.id",
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
    let mut statement = conn.prepare(
        "SELECT name, content FROM `snippets`
        WHERE deleted = 0 AND id = ?")
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
    let mut statement = conn.prepare(
        "UPDATE `snippets` SET deleted = 1, last_updated = ?
        WHERE id = ?")
        .context("failed to prepare load statement")?;

    statement.bind(1, util::get_utc_now())
        .context("failed to bind time")?;
    statement.bind(2, snippet_id)
        .context("failed to bind snippet id")?;

    statement.next()
        .context("failed to execute sql statement")?;

    // remove all tags linked to this snippet
    remove_tags_by_snippet_id(conn, snippet_id)
        .context("failed to delete snippet tags")?;

    Ok(())
}

pub fn save_snippet(conn: &Connection, name: String, content: String, tags: Option<Vec<&str>>) -> Result<i64, Error> {
    let new_last_updated = util::get_utc_now();

    let mut statement = conn.prepare(
        "INSERT INTO `snippets` (name, content, last_updated)
        VALUES (?, ?, ?)
        ON CONFLICT(name) DO UPDATE
        SET content = ?, deleted = 0, last_updated = ?
        WHERE deleted = 1")
        .context("failed to prepare save statement")?;

    statement.bind(1, name.as_str())
        .context("failed to bind name")?;
    statement.bind(2, content.as_str())
        .context("failed to bind content")?;
    statement.bind(3, new_last_updated)
        .context("failed to bind time")?;
    statement.bind(4, content.as_str())
        .context("failed to bind content")?;
    statement.bind(5, new_last_updated)
        .context("failed to bind time")?;

    statement.next()
        .context("failed to execute sql statement")?;

    let (snippet_id, last_updated) = get_snippet_meta(&conn, name.as_str())
        .context("failed to get snippet id")?;

    if new_last_updated != last_updated {
        bail!(error::DupSnippetName);
    }

    if let Some(tags) = tags {
        save_tags(&conn, snippet_id, tags)
            .context("failed to save snippet tags")?;
    }

    Ok(snippet_id)
}

pub fn change_snippet_content(conn: &Connection, snippet_id: i64, content: String) -> Result<(), Error> {
    let mut statement = conn.prepare(
        "UPDATE `snippets` SET content = ?, last_updated = ? WHERE id = ?;")
        .context("failed to prepare content change statement")?;

    statement.bind(1, content.as_str())
        .context("failed to bind content")?;
    statement.bind(2, util::get_utc_now())
        .context("failed to bind time")?;
    statement.bind(3, snippet_id)
        .context("failed to bind id")?;

    statement.next()
        .context("failed to execute sql statement")?;

    Ok(())
}

pub fn rename_snippet(conn: &Connection, snippet_id: i64, name: String) -> Result<(), Error> {
    let mut statement = conn.prepare(
        "UPDATE `snippets` SET name = ?, last_updated = ? WHERE id = ?;")
        .context("failed to prepare snippet rename statement")?;

    statement.bind(1, name.as_str())
        .context("failed to bind name")?;
    statement.bind(2, util::get_utc_now())
        .context("failed to bind time")?;
    statement.bind(3, snippet_id)
        .context("failed to bind id")?;

    statement.next()
        .context("failed to execute sql statement")?;

    Ok(())
}

fn remove_tags_by_snippet_id(conn: &Connection, snippet_id: i64) -> Result<(), Error> {
    let mut statement = conn.prepare(
        "UPDATE `snippet_tags` SET deleted = 1, last_updated = ?
        WHERE snippet_id = ?")
        .context("failed to prepare tag removal statement")?;

    statement.bind(1, util::get_utc_now())
        .context("failed to bind time")?;
    statement.bind(2, snippet_id)
        .context("failed to bind id")?;

    statement.next()
        .context("failed to execute sql statement")?;

    Ok(())
}

pub fn remove_tags_by_name(conn: &Connection, snippet_id: i64, tags: Vec<&str>) -> Result<(), Error> {
    let tags_filter = tags.clone()
        .iter()
        .map(|_| "?")
        .collect::<Vec<&str>>()
        .as_slice()
        .join(", ");

    let query = format!(
        "UPDATE `snippet_tags` AS ST SET deleted = 1, last_updated = ?
        WHERE ST.snippet_id = ? AND ST.tag_id IN (
            SELECT id FROM tags
            WHERE tags.name IN ({})
        )", tags_filter);

    let mut statement = conn.prepare(query)
        .context("failed to prepare snippet rename statement")?;

    statement.bind(1, util::get_utc_now())
        .context("failed to bind time")?;
    statement.bind(2, snippet_id)
        .context("failed to bind id")?;

    for (i, &tag) in tags.iter().enumerate() {
        statement.bind(3+i, tag)
            .context("failed to bind name")?;
    }

    statement.next()
        .context("failed to execute sql statement")?;

    Ok(())
}

pub fn save_tags(conn: &Connection, snippet_id: i64, tags: Vec<&str>) -> Result<(), Error> {
    // insert tag if not exists
    let mut insert_tag = conn.prepare(
       "INSERT INTO `tags` (name, last_updated)
       VALUES (?, ?)")
       .context("failed to prepare tag save statement")?
       .cursor();

    // save relationship between snippet and tag
    let mut insert_snippet_tag = conn.prepare(
        "INSERT INTO `snippet_tags` (snippet_id, tag_id, deleted, last_updated)
        SELECT ?, id, 0, ?
        FROM `tags`
        WHERE name = ?
        ON CONFLICT(snippet_id, tag_id) DO UPDATE
        SET deleted = 0, last_updated = ?")
        .context("failed to prepare snippet_tag save statement")?
        .cursor();

    for tag in tags.clone() {
        insert_tag.bind(&[Value::String(tag.to_string()),
                          Value::Integer(util::get_utc_now())])
            .context("failed to bind name")?;

        insert_tag.next()
            .context("failed to execute sql statement")?;

        insert_snippet_tag.bind(&[Value::Integer(snippet_id),
                                  Value::Integer(util::get_utc_now()),
                                  Value::String(tag.to_string()),
                                  Value::Integer(util::get_utc_now())])
            .context("failed to bind values")?;

        insert_snippet_tag.next()
            .context("failed to execute sql statement")?;
    }

    Ok(())
}

fn get_snippet_meta(conn: &Connection, name: &str) -> Result<(i64, i64), Error> {
    let mut statement = conn.prepare(
        "SELECT id, last_updated FROM `snippets` WHERE
        deleted = 0 AND name = ?")
        .context("failed to prepare save statement")?;

    statement.bind(1, name)
        .context("failed to bind name")?;

    statement.next()
        .context("failed to execute sql statement")?;

    let snippet_id = statement.read::<i64>(0)
        .context("failed to read id col")?;
    let last_updated = statement.read::<i64>(1)
        .context("failed to read update time col")?;

    Ok((snippet_id, last_updated))
}

fn get_snippet_tags(conn: &Connection, snippet_id: i64) -> Result<Vec<String>, Error> {
    let mut statement = conn.prepare(
        "SELECT name FROM `tags` AS T
        INNER JOIN `snippet_tags` AS ST on T.id = ST.tag_id AND
        ST.deleted = 0 AND ST.snippet_id = ?")
        .context("failed to prepare load statement")?;

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

#[cfg(feature = "sync")]
fn parse_snippet_row(statement: &mut sqlite::Statement) -> Result<sync::SnippetRow, Error> {
    let id = statement.read::<i64>(0)
        .context("failed to read id col")?;
    let name = statement.read::<String>(1)
        .context("failed to read name col")?;
    let content = statement.read::<String>(2)
        .context("failed to read content col")?;
    let deleted = statement.read::<i64>(3)
        .context("failed to read deleted col")?;
    let last_updated = statement.read::<i64>(4)
        .context("failed to read last update col")?;

    let row = sync::SnippetRow {
        id: id,
        name: name,
        content: content,
        deleted: deleted,
        last_updated: last_updated,
    };

    Ok(row)
}

#[cfg(feature = "sync")]
fn parse_tag_row(statement: &mut sqlite::Statement) -> Result<sync::TagRow, Error> {
    let id = statement.read::<i64>(0)
        .context("failed to read id col")?;
    let name = statement.read::<String>(1)
        .context("failed to read name col")?;
    let deleted = statement.read::<i64>(2)
        .context("failed to read deleted col")?;
    let last_updated = statement.read::<i64>(3)
        .context("failed to read last update col")?;

    let row = sync::TagRow {
        id: id,
        name: name,
        deleted: deleted,
        last_updated: last_updated,
    };

    Ok(row)
}

#[cfg(feature = "sync")]
fn parse_snippet_tag_row(statement: &mut sqlite::Statement) -> Result<sync::SnippetTagRow, Error> {
    let id = statement.read::<i64>(0)
        .context("failed to read id col")?;
    let snippet_id = statement.read::<i64>(1)
        .context("failed to read snippet id col")?;
    let tag_id = statement.read::<i64>(2)
        .context("failed to read tag id col")?;
    let deleted = statement.read::<i64>(3)
        .context("failed to read deleted col")?;
    let last_updated = statement.read::<i64>(4)
        .context("failed to read last update col")?;

    let row = sync::SnippetTagRow {
        id: id,
        snippet_id: snippet_id,
        tag_id: tag_id,
        deleted: deleted,
        last_updated: last_updated,
    };

    Ok(row)
}

#[cfg(feature = "sync")]
pub type SyncData = (Vec<sync::SnippetRow>, Vec<sync::TagRow>, Vec<sync::SnippetTagRow>);

#[cfg(feature = "sync")]
pub fn get_sync_data(conn: &Connection, last_synced: i64) -> Result<SyncData, Error> {
    let mut snippet_data = Vec::new();
    let mut tag_data = Vec::new();
    let mut snippet_tag_data = Vec::new();

    let tables = vec!["snippets", "tags", "snippet_tags"];

    for (idx, table) in tables.iter().enumerate() {
        let query = format!("SELECT * FROM `{}` WHERE last_updated > ?", table);

        let mut statement = conn.prepare(query)
            .context("failed to prepare sync statement")?;

        statement.bind(1, last_synced)
            .context("failed to bind last synced time")?;

        while let State::Row = statement.next().context("failed to execute sql statement")? {
            match idx {
                0 => {
                    let row = parse_snippet_row(&mut statement)
                        .context("failed to parse snippet row")?;
                    snippet_data.push(row);
                },
                1 => {
                    let row = parse_tag_row(&mut statement)
                        .context("failed to parse tag row")?;
                    tag_data.push(row);
                },
                2 => {
                    let row = parse_snippet_tag_row(&mut statement)
                        .context("failed to parse snippet tag row")?;
                    snippet_tag_data.push(row);
                },
                _ => panic!("unexpected error")
            };
        }
    }

    Ok((snippet_data, tag_data, snippet_tag_data))
}

pub fn get_metadata_value(conn: &Connection, key: &str) -> Result<String, Error> {
    let mut statement = conn.prepare("SELECT value FROM metadata WHERE key = ?")
        .context("failed to prepare meta data statement")?;

    statement.bind(1, key)
        .context("failed to bind key")?;

    let state = statement.next()
        .context("failed to execute sql statement")?;

    if state == State::Done {
        bail!(error::UnknownMetaKey);
    }

    let value = statement.read::<String>(0)
        .context("failed to read value col")?;

    Ok(value)
}

pub fn set_metadata_value(conn: &Connection, key: &str, value: &str) -> Result<(), Error> {
    let mut statement = conn.prepare(
        "INSERT INTO `metadata` (key, value)
        VALUES (?, ?)
        ON CONFLICT(key) DO UPDATE
        SET value = ?
        WHERE key = ?")
        .context("failed to prepare metadata statement")?;

    statement.bind(1, key)
        .context("failed to bind key")?;
    statement.bind(2, value)
        .context("failed to bind value")?;
    statement.bind(3, value)
        .context("failed to bind value")?;
    statement.bind(4, key)
        .context("failed to bind key")?;

    statement.next()
        .context("failed to execute sql statement")?;

    Ok(())
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
            content TEXT,
            deleted INTEGER DEFAULT 0,
            last_updated INTEGER NOT NULL
        );
        CREATE TABLE tags(
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name VARCHAR(64),
            deleted INTEGER DEFAULT 0,
            last_updated INTEGER NOT NULL,
            UNIQUE(name) ON CONFLICT IGNORE
        );
        CREATE TABLE snippet_tags(
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            snippet_id INTEGER REFERENCES snippets(id),
            tag_id INTEGER REFERENCES tags(id),
            deleted INTEGER DEFAULT 0,
            last_updated INTEGER NOT NULL,
            UNIQUE(snippet_id, tag_id)
        );
        CREATE TABLE metadata(
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            key VARCHAR(32),
            value TEXT,
            UNIQUE(key) ON CONFLICT REPLACE
        );
        INSERT INTO `metadata` (key, value) VALUES ('last_synced', 0);"
        ).context("failed to create database tables")?;

    Ok(conn)
}
