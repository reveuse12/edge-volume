use std::{fs::File, path::Path};
// The OS releases this lock on exit/crash. Never unlink a live lock file.
pub fn acquire(path: &Path) -> std::io::Result<Option<File>> {
    let file = File::options()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(error)) => Err(error),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exclusive_lock_and_release_without_deleting_file() {
        let path =
            std::env::temp_dir().join(format!("edgevolume-instance-test-{}", std::process::id()));
        let first = acquire(&path).unwrap().unwrap();
        assert!(acquire(&path).unwrap().is_none());
        drop(first);
        assert!(acquire(&path).unwrap().is_some());
        std::fs::remove_file(path).unwrap();
    }
}
