use std::{
	borrow::Cow,
	collections::BTreeMap,
	path::{Path, PathBuf},
};

pub enum IncludeDir {
	Fs(FsDirectory),
	Included(IncludedDirectory),
}

pub enum FsOrIncludedFile {
	Fs(FsFile),
	Included(IncludedFile),
}

impl IncludeDir {
	pub fn read(&self, path: &Path) -> Option<FsOrIncludedFile> {
		match self {
			IncludeDir::Fs(s) => s.read(path),
			IncludeDir::Included(s) => s.read(path),
		}
	}
}

impl IntoIterator for IncludeDir {
	type Item = (PathBuf, FsOrIncludedFile);
	type IntoIter = FsOrIncludedIntoIter;
	fn into_iter(self) -> Self::IntoIter {
		match self {
			IncludeDir::Fs(fs) => FsOrIncludedIntoIter::Fs(
				walkdir::WalkDir::new(fs.0).sort_by_file_name().into_iter(),
			),
			IncludeDir::Included(embedded) => {
				FsOrIncludedIntoIter::Included(embedded.0.into_iter())
			}
		}
	}
}

pub enum FsOrIncludedIntoIter {
	Fs(walkdir::IntoIter),
	Included(std::collections::btree_map::IntoIter<&'static Path, IncludedFile>),
}

impl Iterator for FsOrIncludedIntoIter {
	type Item = (PathBuf, FsOrIncludedFile);
	fn next(&mut self) -> Option<Self::Item> {
		match self {
			FsOrIncludedIntoIter::Fs(walkdir) => loop {
				let entry = match walkdir.next() {
					None => return None,
					Some(Err(e)) => panic!("{}", e),
					Some(Ok(entry)) => entry,
				};
				if entry.file_type().is_file() {
					let path = entry.path().to_owned();
					return Some((path.clone(), FsOrIncludedFile::Fs(FsFile(path))));
				} else {
					continue;
				}
			},
			FsOrIncludedIntoIter::Included(map) => map
				.next()
				.map(|(path, file)| (path.to_owned(), FsOrIncludedFile::Included(file))),
		}
	}
}

impl FsOrIncludedFile {
	pub fn data(&self) -> Cow<'static, [u8]> {
		match self {
			FsOrIncludedFile::Fs(s) => s.data(),
			FsOrIncludedFile::Included(s) => s.data(),
		}
	}

	pub fn hash(&self) -> Option<&'static str> {
		match self {
			FsOrIncludedFile::Fs(s) => s.hash(),
			FsOrIncludedFile::Included(s) => s.hash(),
		}
	}
}

pub struct FsDirectory(pub PathBuf);

impl FsDirectory {
	pub fn read(&self, path: &Path) -> Option<FsOrIncludedFile> {
		let path = self.0.join(path);
		if path.exists() {
			Some(FsOrIncludedFile::Fs(FsFile(path)))
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
pub struct IncludedDirectory(pub BTreeMap<&'static Path, IncludedFile>);

#[derive(Clone, Debug)]
pub struct IncludedFile {
	pub data: &'static [u8],
	pub hash: &'static str,
}

impl IncludedDirectory {
	pub fn read(&self, path: &Path) -> Option<FsOrIncludedFile> {
		self.0
			.get(path)
			.map(|file| FsOrIncludedFile::Included(file.clone()))
	}
}

impl IncludedFile {
	pub fn data(&self) -> Cow<'static, [u8]> {
		Cow::Borrowed(self.data)
	}

	pub fn hash(&self) -> Option<&'static str> {
		Some(self.hash)
	}
}
