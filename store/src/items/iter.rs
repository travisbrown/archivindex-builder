use std::path::{Path, PathBuf};

pub enum DirectoryIter {
    Running {
        level0: Vec<PathBuf>,
        level1: Option<Vec<PathBuf>>,
    },
    Failed(Option<std::io::Error>),
}

impl DirectoryIter {
    pub(crate) fn new(base: &Path) -> Self {
        match dir_contents(base) {
            Ok(paths) => Self::Running {
                level0: paths,
                level1: None,
            },
            Err(error) => Self::Failed(Some(error)),
        }
    }
}

impl Iterator for DirectoryIter {
    type Item = Result<PathBuf, std::io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Running {
                ref mut level0,
                ref mut level1,
            } => match level1.take() {
                Some(mut paths) => match paths.pop() {
                    Some(next) => {
                        let _ = level1.insert(paths);
                        Some(Ok(next))
                    }
                    None => self.next(),
                },
                None => match level0.pop() {
                    Some(level0_next) => match dir_contents(&level0_next) {
                        Ok(paths) => {
                            let _ = level1.insert(paths);
                            self.next()
                        }
                        Err(error) => Some(Err(error)),
                    },
                    None => None,
                },
            },
            Self::Failed(error) => error.take().map(Err),
        }
    }
}

pub struct FileIter {
    directories: DirectoryIter,
    current: Option<Vec<PathBuf>>,
}

impl FileIter {
    pub(crate) fn new(directories: DirectoryIter) -> Self {
        Self {
            directories,
            current: None,
        }
    }
}

impl Iterator for FileIter {
    type Item = Result<PathBuf, std::io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let Self {
            ref mut directories,
            ref mut current,
        } = self;
        match current.take() {
            Some(mut paths) => match paths.pop() {
                Some(next) => {
                    let _ = current.insert(paths);
                    Some(Ok(next))
                }
                None => self.next(),
            },
            None => match directories.next() {
                Some(Ok(current_next)) => match dir_contents(&current_next) {
                    Ok(paths) => {
                        let _ = current.insert(paths);
                        self.next()
                    }
                    Err(error) => Some(Err(error)),
                },
                Some(Err(error)) => Some(Err(error)),
                None => None,
            },
        }
    }
}

fn dir_contents(path: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut paths = std::fs::read_dir(path).and_then(|entries| {
        entries
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()
    })?;

    paths.sort();
    paths.reverse();

    Ok(paths)
}
