use std::cell::RefCell;
use std::path::Path;

use agora_core::{Subreddit, SubredditName};
use rusqlite::{Connection, params};

/// `$XDG_DATA_HOME/agora/agora.db`: the saved subscription list (SPEC §14).
/// Drafts join it in slice 4. Small writes on the main context.
pub struct LocalData {
    db: RefCell<Connection>,
}

impl LocalData {
    /// Opens or creates the database at `path`.
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        Self::with(Connection::open(path)?)
    }

    fn with(db: Connection) -> rusqlite::Result<Self> {
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS subscriptions (
                name TEXT PRIMARY KEY COLLATE NOCASE,
                id TEXT NOT NULL,
                title TEXT,
                icon_url TEXT
            );",
        )?;
        Ok(Self { db: RefCell::new(db) })
    }

    /// The list saved at the last refresh, by name.
    pub fn subscriptions(&self) -> rusqlite::Result<Vec<Subreddit>> {
        let db = self.db.borrow();
        let mut query = db.prepare("SELECT name, id, title, icon_url FROM subscriptions ORDER BY name")?;
        query
            .query_map([], |row| {
                Ok(Subreddit {
                    id: row.get(1)?,
                    title: row.get(2)?,
                    icon_url: row.get(3)?,
                    subscribed: true,
                    ..Subreddit::from_name(SubredditName::new(row.get::<_, String>(0)?))
                })
            })?
            .collect()
    }

    /// Replaces the saved list.
    pub fn save_subscriptions(&self, subs: &[Subreddit]) -> rusqlite::Result<()> {
        let mut db = self.db.borrow_mut();
        let tx = db.transaction()?;
        tx.execute("DELETE FROM subscriptions", [])?;
        for sub in subs {
            tx.execute(
                "INSERT OR REPLACE INTO subscriptions (name, id, title, icon_url) VALUES (?1, ?2, ?3, ?4)",
                params![sub.name.as_str(), sub.id, sub.title, sub.icon_url],
            )?;
        }
        tx.commit()
    }

    /// Deletes everything (sign-out, SPEC §14).
    pub fn clear(&self) -> rusqlite::Result<()> {
        self.db.borrow().execute("DELETE FROM subscriptions", []).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sub(name: &str) -> Subreddit {
        Subreddit {
            id: format!("t5_{name}"),
            subscribed: true,
            ..Subreddit::from_name(SubredditName::new(name))
        }
    }

    fn memory() -> LocalData {
        LocalData::with(Connection::open_in_memory().unwrap()).unwrap()
    }

    #[test]
    fn saving_replaces_the_list() {
        let data = memory();
        data.save_subscriptions(&[sub("gnome"), sub("rust")]).unwrap();
        data.save_subscriptions(&[sub("rust")]).unwrap();
        assert_eq!(data.subscriptions().unwrap(), vec![sub("rust")]);
    }

    #[test]
    fn clearing_leaves_nothing() {
        let data = memory();
        data.save_subscriptions(&[sub("gnome")]).unwrap();
        data.clear().unwrap();
        assert!(data.subscriptions().unwrap().is_empty());
    }
}
