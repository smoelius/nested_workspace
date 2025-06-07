#![allow(unused)]

use anyhow::{Result, anyhow, ensure};
use cargo_metadata::MetadataCommand;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    env::current_dir,
    fs::{Metadata, read_to_string},
    path::{Path, PathBuf},
    sync::{
        LazyLock, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::SystemTime,
};
use walkdir::WalkDir;

#[derive(Debug)]
pub struct Value {
    parent_mtime: SystemTime,
    mtime: SystemTime,
    contents: String,
}

impl Value {
    fn new(path: &Path) -> Result<Self> {
        let parent = path.parent().ok_or_else(|| anyhow!("path has no parent"))?;
        let parent_mtime = parent.metadata().and_then(|metadata| metadata.modified())?;
        let mtime = path.metadata().and_then(|metadata| metadata.modified())?;
        let contents = read_to_string(path)?;
        Ok(Self {
            parent_mtime,
            mtime,
            contents,
        })
    }
}

#[derive(Debug)]
pub struct NowContents {
    map: HashMap<PathBuf, Value>,
    compared: AtomicBool,
}

impl NowContents {
    pub fn new(stdout: &str) -> Result<Self> {
        let map = Self::map_from_stdout(stdout)?;
        Ok(Self {
            map,
            compared: AtomicBool::new(false),
        })
    }

    fn map_from_stdout(stdout: &str) -> Result<HashMap<PathBuf, Value>> {
        let mut map = HashMap::new();
        for line in stdout
            .lines()
            .filter(|line| line.replace('\\', "/").ends_with("/out"))
        {
            let index = line.rfind('=').unwrap();
            let path = Path::new(&line[index + 1..]).join("now.txt");
            let value = Value::new(&path)?;
            map.insert(path, value);
        }
        Ok(map)
    }

    pub fn get(&self) -> &HashMap<PathBuf, Value> {
        &self.map
    }

    /// Produces a new instance with a map with the same keys
    pub fn rescan(&self) -> Result<Self> {
        let map = self
            .map
            .keys()
            .map(|path| {
                Value::new(path)
                    .map(|contents| (path.clone(), contents))
                    .map_err(Into::into)
            })
            .collect::<Result<HashMap<_, _>>>()?;
        Ok(Self {
            map,
            compared: AtomicBool::new(false),
        })
    }

    /// Verifies that:
    /// - keys match
    /// - key-value pairs do not match
    pub fn compare(&self, other: &Self) -> Result<()> {
        self.compared.store(true, Ordering::SeqCst);
        other.compared.store(true, Ordering::SeqCst);

        let keys_self = self.map.keys().collect::<HashSet<_>>();
        let keys_other = other.map.keys().collect::<HashSet<_>>();
        ensure!(keys_self == keys_other, "keys differ");

        for (path, value_self) in &self.map {
            let value_other = other.map.get(path).unwrap();
            ensure!(
                value_self.contents != value_other.contents,
                "`{}` contents are equal",
                path.display()
            );
        }

        Ok(())
    }
}

impl Drop for NowContents {
    fn drop(&mut self) {
        if !self.compared.load(Ordering::SeqCst) {
            let other = self.rescan().unwrap();
            self.compare(&other).unwrap();
        }
    }
}
