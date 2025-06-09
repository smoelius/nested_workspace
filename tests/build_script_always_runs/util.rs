use anyhow::{Result, anyhow, ensure};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::SystemTime,
};

#[derive(Debug)]
pub struct Value {
    parent_mtime: SystemTime,
    mtime: SystemTime,
}

impl Value {
    fn new(path: &Path) -> Result<Self> {
        let parent = path.parent().ok_or_else(|| anyhow!("path has no parent"))?;
        let parent_mtime = parent.metadata().and_then(|metadata| metadata.modified())?;
        let mtime = path.metadata().and_then(|metadata| metadata.modified())?;
        Ok(Self {
            parent_mtime,
            mtime,
        })
    }
}

#[derive(Debug)]
pub struct Timestamps {
    map: HashMap<PathBuf, Value>,
    compared: AtomicBool,
}

impl Timestamps {
    pub fn new(stdout: &str) -> Result<Self> {
        let map = Self::map_from_stdout(stdout)?;
        Ok(Self {
            map,
            compared: AtomicBool::new(false),
        })
    }

    fn map_from_stdout(stdout: &str) -> Result<HashMap<PathBuf, Value>> {
        let mut map = HashMap::new();
        for line in stdout.lines().filter(|line| {
            line.replace('\\', "/")
                .ends_with("/nested_workspace.timestamp")
        }) {
            let index = line.rfind('=').unwrap();
            let path = PathBuf::from(&line[index + 1..]);
            let value = Value::new(&path)?;
            map.insert(path, value);
        }
        Ok(map)
    }

    pub fn get(&self) -> &HashMap<PathBuf, Value> {
        &self.map
    }

    /// Produce a new instance with a map with the same keys
    pub fn rescan(&self) -> Result<Self> {
        let map = self
            .map
            .keys()
            .map(|path| Value::new(path).map(|contents| (path.clone(), contents)))
            .collect::<Result<HashMap<_, _>>>()?;
        Ok(Self {
            map,
            compared: AtomicBool::new(false),
        })
    }

    pub fn compare(&self, other: &Self) -> Result<()> {
        self.compared.store(true, Ordering::SeqCst);
        other.compared.store(true, Ordering::SeqCst);

        let keys_self = self.map.keys().collect::<HashSet<_>>();
        let keys_other = other.map.keys().collect::<HashSet<_>>();
        ensure!(keys_self == keys_other, "keys differ");

        for (path, value_self) in &self.map {
            let value_other = other.map.get(path).unwrap();
            ensure!(value_self.parent_mtime <= value_other.parent_mtime);
            ensure!(value_self.mtime < value_other.mtime);
        }

        Ok(())
    }
}

impl Drop for Timestamps {
    fn drop(&mut self) {
        if !self.compared.load(Ordering::SeqCst) {
            let other = self.rescan().unwrap();
            self.compare(&other).unwrap();
        }
    }
}
