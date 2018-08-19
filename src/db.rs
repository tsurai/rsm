use std::path::PathBuf;
use std::{env, fs};
use failure::*;
use sqlite::{self, Connection, Value};

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

    Ok(conn)
}

pub fn save_snippet(conn: &Connection, name: String, content: String, tags: Option<Vec<&str>>) -> Result<(), Error> {
    let mut statement = conn.prepare("INSERT INTO `snippets` (name, content) VALUES (?, ?)")
        .context("failed to prepare save statement")?;

    // bind INSERT values
    statement.bind(1, name.as_str())
        .context("failed to bind name")?;
    statement.bind(2, content.as_str())
        .context("failed to bind content")?;

    // check if the snippet name is a duplicate and  exit early gracefully
    if let Err(e) = statement.next() {
        if e.code.unwrap_or(0) == 19 {
            println!("Error: duplicate snippet name");
            return Ok(());
        }

        bail!(e.context("failed to execute sql statement"))
    }

    if let Some(tags) = tags {
        save_tags(&conn, name.as_str(), tags)
            .context("failed to save snippet tags")?;
    }

    Ok(())
}

fn save_tags(conn: &Connection, name: &str, tags: Vec<&str>) -> Result<(), Error> {
    let snippet_id = get_snippet_id(conn, name)
        .context("failed to get snippet id")?;

    // insert tag if not exists
    let mut insert_tag = conn.prepare(
       "INSERT INTO `tags` (name)
       SELECT ?
       WHERE NOT EXISTS (SELECT 1 FROM `tags` WHERE name = ?);"
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
        insert_tag.bind(&[Value::String(tag.to_string()), Value::String(tag.to_string())])
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
            name VARCHAR(64) UNIQUE
        );
        CREATE TABLE snippet_tags(
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            snippet_id INTEGER,
            tag_id INTEGER
        );"
        ).context("failed to create database tables")?;

    Ok(conn)
}
