use std::{
	borrow::Cow,
	collections::BTreeMap,
	path::{Path, PathBuf},
};

pub enum FsOrEmbeddedDirectory {
	Fs(FsDirectory),
	Embedded(EmbeddedDirectory),
}

pub enum FsOrEmbeddedFile {
	Fs(FsFile),
	Embedded(EmbeddedFile),
}

impl FsOrEmbeddedDirectory {
	pub fn read(&self, path: &Path) -> Option<FsOrEmbeddedFile> {
		match self {
			FsOrEmbeddedDirectory::Fs(s) => s.read(path),
			FsOrEmbeddedDirectory::Embedded(s) => s.read(path),
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
