use std::{
	borrow::Cow,
	collections::BTreeMap,
	path::{Path, PathBuf},
};

pub enum IncludeDir {
	Fs(FsDirectory),
	Embedded(EmbeddedDirectory),
}

pub enum FsOrEmbeddedFile {
	Fs(FsFile),
	Embedded(EmbeddedFile),
}

impl IncludeDir {
	pub fn read(&self, path: &Path) -> Option<FsOrEmbeddedFile> {
		match self {
			IncludeDir::Fs(s) => s.read(path),
			IncludeDir::Embedded(s) => s.read(path),
		}
	}
}

impl IntoIterator for IncludeDir {
	type Item = (PathBuf, FsOrEmbeddedFile);
	type IntoIter = FsOrEmbeddedIntoIter;
	fn into_iter(self) -> Self::IntoIter {
		match self {
			IncludeDir::Fs(fs) => FsOrEmbeddedIntoIter::Fs(
				walkdir::WalkDir::new(fs.0).sort_by_file_name().into_iter(),
			),
			IncludeDir::Embedded(embedded) => {
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

impl FsOrEmbeddedFile {
	pub fn data(&self) -> Cow<'static, [u8]> {
		match self {
			FsOrEmbeddedFile::Fs(s) => s.data(),
			FsOrEmbeddedFile::Embedded(s) => s.data(),
		}
	}

	pub fn hash(&self) -> Option<&'static str> {
		match self {
			FsOrEmbeddedFile::Fs(s) => s.hash(),
			FsOrEmbeddedFile::Embedded(s) => s.hash(),
		}
	}
}

pub struct FsDirectory(pub PathBuf);

impl FsDirectory {
	pub fn read(&self, path: &Path) -> Option<FsOrEmbeddedFile> {
		let path = self.0.join(path);
		if path.exists() {
			Some(FsOrEmbeddedFile::Fs(FsFile(path)))
		} else {
			None
		}
	}
}

pub struct FsFile(pub PathBuf);

impl FsFile {
	pub fn data(&self) -> Cow<'static, [u8]> {
		Cow::Owned(std::fs::read(&self.0).unwrap())
	}

	pub fn hash(&self) -> Option<&'static str> {
		None
	}
}

#[derive(Debug)]
pub struct EmbeddedDirectory(pub BTreeMap<&'static Path, EmbeddedFile>);

#[derive(Clone, Debug)]
pub struct EmbeddedFile {
	pub data: &'static [u8],
	pub hash: &'static str,
}

impl EmbeddedDirectory {
	pub fn read(&self, path: &Path) -> Option<FsOrEmbeddedFile> {
		self.0
			.get(path)
			.map(|file| FsOrEmbeddedFile::Embedded(file.clone()))
	}
}

impl EmbeddedFile {
	pub fn data(&self) -> Cow<'static, [u8]> {
		Cow::Borrowed(self.data)
	}

	pub fn hash(&self) -> Option<&'static str> {
		Some(self.hash)
	}
}
