//! 표준 디렉터리: app / data / models / cache / runtimes / manifests / logs / projects / sdk / docs / exports.
//! 모든 경로는 상대. 절대경로는 런타임에 root와 결합해서만 사용.

use std::path::{Path, PathBuf};

pub struct Workspace {
    pub root: PathBuf,
}

impl Workspace {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }
    pub fn data(&self) -> PathBuf {
        self.root.join("data")
    }
    pub fn models(&self) -> PathBuf {
        self.root.join("models")
    }
    pub fn cache(&self) -> PathBuf {
        self.root.join("cache")
    }
    pub fn runtimes(&self) -> PathBuf {
        self.root.join("runtimes")
    }
    pub fn manifests(&self) -> PathBuf {
        self.root.join("manifests")
    }
    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }
    pub fn projects(&self) -> PathBuf {
        self.root.join("projects")
    }
    pub fn sdk(&self) -> PathBuf {
        self.root.join("sdk")
    }
    pub fn docs(&self) -> PathBuf {
        self.root.join("docs")
    }
    pub fn exports(&self) -> PathBuf {
        self.root.join("exports")
    }
    pub fn manifest_file(&self) -> PathBuf {
        self.root.join("manifest.json")
    }
}
