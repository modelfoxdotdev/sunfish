use std::path::{Path, PathBuf};

use crate::embed::{EmbeddedFile, FsFile, FsOrEmbeddedDirectory, FsOrEmbeddedFile};

pub struct IncludeDir(pub FsOrEmbeddedDirectory);

impl IncludeDir {
	pub fn read(&self, path: &Path) -> Option<FsOrEmbeddedFile> {
		self.0.read(path)
	}
}

impl IntoIterator for IncludeDir {
	type Item = (PathBuf, FsOrEmbeddedFile);
	type IntoIter = FsOrEmbeddedIntoIter;
	fn into_iter(self) -> Self::IntoIter {
		match self.0 {
			FsOrEmbeddedDirectory::Fs(fs) => FsOrEmbeddedIntoIter::Fs(
				walkdir::WalkDir::new(fs.0).sort_by_file_name().into_iter(),
			),
			FsOrEmbeddedDirectory::Embedded(embedded) => {
				FsOrEmbeddedIntoIter::Embedded(embedded.0.into_iter())
			}
		}
	}
}

pub enum FsOrEmbeddedIntoIter {
	Fs(walkdir::IntoIter),
	Embedded(std::collections::btree_map::IntoIter<&'static Path, EmbeddedFile>),
}

impl Iterator for FsOrEmbeddedIntoIter {
	type Item = (PathBuf, FsOrEmbeddedFile);
	fn next(&mut self) -> Option<Self::Item> {
		match self {
			FsOrEmbeddedIntoIter::Fs(walkdir) => loop {
				let entry = match walkdir.next() {
					None => return None,
					Some(Err(e)) => panic!("{}", e),
					Some(Ok(entry)) => entry,
				};
				if entry.file_type().is_file() {
					let path = entry.path().to_owned();
					return Some((path.clone(), FsOrEmbeddedFile::Fs(FsFile(path))));
				} else {
					continue;
				}
			},
			FsOrEmbeddedIntoIter::Embedded(map) => map
				.next()
				.map(|(path, file)| (path.to_owned(), FsOrEmbeddedFile::Embedded(file))),
		}
	}
}
