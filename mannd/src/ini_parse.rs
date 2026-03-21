use std::{
    collections::BTreeMap,
    fmt::Debug,
    fs::{File, read_to_string},
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
};

use crate::{HOME, error::ManndError};
use tempfile::NamedTempFile;
use tracing::instrument;

#[derive(Debug)]
pub struct IniConfig {
    pub file_path: PathBuf,
    // use btreemap to keep ordering
    pub sections: BTreeMap<String, BTreeMap<String, String>>,
}

impl IniConfig {
    #[instrument(err)]
    pub fn new(file_path: PathBuf) -> Result<Self, ManndError> {
        let sections: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        let mut conf = Self {
            file_path,
            sections,
        };
        Self::parse_file(&mut conf)?;
        Ok(conf)
    }

    #[instrument(err, skip(self))]
    fn parse_file(&mut self) -> Result<(), ManndError> {
        let file = read_to_string(self.file_path.clone())
            .map_err(|_| ManndError::FileNotFound("File not found".to_string()))?;
        let lines = file.lines();

        let mut current_section: String = String::new();
        for line in lines {
            let line = line.trim().to_string();
            // skip whitespace, incorrect indentation and empty lines
            if line.starts_with(['#']) || line.is_empty() {
                continue;
            }

            if line.starts_with('[') && line.ends_with(']') {
                current_section = line[1..line.len() - 1].to_string();
                self.sections
                    .entry(current_section.to_string())
                    .or_default();
            } else if let Some((k, v)) = line.split_once('=') {
                self.sections
                    .entry(current_section.to_string())
                    .or_default()
                    .insert(
                        k.trim().to_string(),
                        Self::parse_field(v)?.trim().to_string(),
                    );
            }
        }

        Ok(())
    }

    pub fn add_to_section<T: Into<String>>(
        &mut self,
        key: T,
        value: (T, T),
    ) -> Result<(), ManndError> {
        let key_ref: &str = &key.into();
        let section = self
            .sections
            .get_mut(key_ref)
            .ok_or_else(|| ManndError::SectionNotFound(format!("{} not found...", key_ref)))?;

        section.insert(value.0.into(), value.1.into());

        Ok(())
    }

    #[instrument(err)]
    fn parse_field<'a>(input: &'a str) -> Result<String, ManndError> {
        let mut res = String::new();

        if let (Some(start), Some(end)) = (input.find('"'), input.rfind('"')) {
            let mut current = &input[start + 1..end];
            let mut var_found = false;
            while let Some(start_idx) = current.find("${") {
                var_found = true;
                let var_start = &current[start_idx + 2..];
                res.push_str(&current[..start_idx]);

                if let Some(end_idx) = var_start.find("}") {
                    let var_name = &var_start[..end_idx];
                    if var_name != "HOME" {
                        return Err(ManndError::InvalidPropertyFormat(
                            "Non-HOME variable used".to_string(),
                        ));
                    }
                    let tmp = HOME.get().unwrap().clone();
                    let home_str = tmp.as_os_str().to_str().ok_or_else(|| {
                        ManndError::OperationFailed("Converting HOME path to &str".to_string())
                    })?;
                    res.push_str(home_str);
                    current = &var_start[end_idx + 1..];
                } else {
                    return Err(ManndError::InvalidPropertyFormat(format!("{:?}", input)));
                }
            }
            if !var_found {
                res.push_str(&input[start + 1..end]);
            } else {
                res.push_str(&current);
            }
        } else {
            res.push_str(&input);
        }
        Ok(res)
    }

    #[instrument(err, skip(self))]
    pub fn get<T: ToString + Debug>(&self, section: T, field: T) -> Result<String, ManndError> {
        if let Some(found_section) = self.sections.get(&section.to_string()) {
            if let Some(found_field) = found_section.get(&field.to_string()) {
                Ok(found_field.to_string())
            } else {
                Err(ManndError::PropertyNotFound(field.to_string()))
            }
        } else {
            Err(ManndError::SectionNotFound(section.to_string()))
        }
    }

    /// Returns an IniConfig with only the provided sections and fields
    /// TODO: Performance
    #[instrument(err, skip(self))]
    pub fn get_partial(&self, filter: BTreeMap<String, Vec<String>>) -> Result<Self, ManndError> {
        let mut sections: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        for section in filter.keys() {
            let section_fields = self
                .sections
                .get(section)
                .ok_or_else(|| ManndError::SectionNotFound(format!("{section}")))?;

            let mut put_fields: BTreeMap<String, String> = BTreeMap::new();

            for filter_field in filter.get(section).unwrap() {
                let field_val = section_fields
                    .get(filter_field)
                    .ok_or_else(|| ManndError::SectionNotFound(format!("{section}")))?;
                put_fields.insert(filter_field.clone(), field_val.clone());
            }
            sections.insert(section.clone(), put_fields);
        }

        Ok(IniConfig {
            file_path: self.file_path.clone(),
            sections,
        })
    }

    /// Atomic overwrite
    #[instrument(err, skip(self))]
    pub fn overwrite(&self) -> Result<(), ManndError> {
        let dir = Path::new(&self.file_path)
            .parent()
            .unwrap_or_else(|| ".".as_ref());

        let mut temp = NamedTempFile::new_in(dir)?;
        self.write_to(&mut temp)?;
        temp.as_file().sync_all()?;
        temp.persist(self.file_path.clone())
            .map_err(|_| ManndError::OperationFailed("Persist Failed".to_string()))?;

        File::open(dir)?.sync_all()?;

        Ok(())
    }

    // writes a file to a provided location or a temporary location if none provided
    #[instrument(err, skip(self))]
    pub fn write_file(&self, file_path: Option<PathBuf>) -> Result<(), ManndError> {
        let file = match file_path {
            Some(path) => &File::open(path)?,
            None => &File::open(NamedTempFile::new()?.path())?,
        };

        let mut writer = BufWriter::new(file);
        self.write_to(&mut writer)?;

        Ok(())
    }

    fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        for (section, keys) in &self.sections {
            if !section.is_empty() {
                writeln!(writer, "[{}]", section)?;
            }

            for (key, value) in keys {
                writeln!(writer, "{}={}", key, value)?;
            }
            writeln!(writer)?;
        }

        writer.flush()?;
        Ok(())
    }
}
